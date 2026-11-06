//! The doorway between user programs (ring 3) and the kernel.
//!
//! - `syscall` entry: switches to the thread's kernel stack, saves every user
//!   register in a `SyscallFrame`, calls the Rust dispatcher, restores every
//!   register (so no kernel value leaks out) and returns with `sysretq`.
//! - `enter_user`: first entry of a thread into ring 3 (via `iretq`).
//! - User memory access: `copy_from_user`/`copy_to_user` check the range is
//!   below USER_TOP and recover from page faults, so a bad pointer from a
//!   program becomes an error code, never a kernel crash.
//!
//! The kernel does not use the GS base for itself (programs can change it);
//! `swapgs` is used only for a few instructions at syscall entry to reach
//! this CPU's `Scratch` block.

use super::cpu::{read_msr, write_msr, MSR_EFER};
use awa_abi::USER_TOP;
use core::arch::{asm, global_asm};
use core::sync::atomic::{AtomicBool, Ordering};

const MSR_STAR: u32 = 0xC000_0081;
const MSR_LSTAR: u32 = 0xC000_0082;
const MSR_FMASK: u32 = 0xC000_0084;
const MSR_GS_BASE: u32 = 0xC000_0101;
const MSR_KERNEL_GS_BASE: u32 = 0xC000_0102;
const EFER_SCE: u64 = 1;

pub const USER_CS: u64 = 0x23;
pub const USER_SS: u64 = 0x1b;

/// Per-CPU block reached through `swapgs` on every entry from user mode. It
/// sits in the CPU's entry area, which stays mapped while user code runs.
/// The assembly below uses these offsets.
#[repr(C)]
pub struct Scratch {
    /// 0: temporary home of the user stack pointer.
    pub user_rsp: u64,
    /// 8: top of the current thread's kernel stack.
    pub kernel_rsp: u64,
    /// 16: page tables to load on entry (current process, kernel mode).
    pub kernel_cr3: u64,
    /// 24: page tables to load on exit (current process, user mode).
    pub user_cr3: u64,
    /// 32, 40: register spill slots for the entry/exit code.
    pub tmp: [u64; 2],
    /// 48: top of this CPU's entry stack (TSS rsp0).
    pub entry_stack_top: u64,
}

/// Registers saved at syscall entry, lowest address first.
#[repr(C)]
#[derive(Debug)]
pub struct SyscallFrame {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub rbp: u64,
    pub rbx: u64,
    pub r9: u64,
    pub r8: u64,
    pub r10: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    /// Syscall number on entry, return value on exit.
    pub rax: u64,
    /// User return address (set by `syscall`).
    pub rcx: u64,
    /// User RFLAGS (set by `syscall`).
    pub r11: u64,
    pub user_rsp: u64,
}

global_asm!(
    // Entry and exit code lives in `.entry_text`: mapped while user code runs.
    ".section .entry_text, \"ax\"",
    ".global awa_syscall_entry",
    "awa_syscall_entry:",
    "  swapgs",
    "  mov qword ptr gs:[0], rsp",
    "  mov rsp, qword ptr gs:[16]", // kernel-mode page tables
    "  mov cr3, rsp",
    "  mov rsp, qword ptr gs:[8]",
    "  push qword ptr gs:[0]",
    "  swapgs",
    "  push r11",
    "  push rcx",
    "  push rax",
    "  push rdi",
    "  push rsi",
    "  push rdx",
    "  push r10",
    "  push r8",
    "  push r9",
    "  push rbx",
    "  push rbp",
    "  push r12",
    "  push r13",
    "  push r14",
    "  push r15",
    "  mov rdi, rsp",
    "  call {dispatch}",
    "  cli",
    "  pop r15",
    "  pop r14",
    "  pop r13",
    "  pop r12",
    "  pop rbp",
    "  pop rbx",
    "  pop r9",
    "  pop r8",
    "  pop r10",
    "  pop rdx",
    "  pop rsi",
    "  pop rdi",
    "  pop rax",
    "  pop rcx",
    "  pop r11",
    // Last value on the kernel stack is the user rsp. Park it in scratch,
    // switch to the user-mode page tables, then load it: the kernel stack is
    // not touched after the switch (it is not mapped any more).
    "  swapgs",
    "  pop qword ptr gs:[0]",
    "  mov rsp, qword ptr gs:[24]",
    "  mov cr3, rsp",
    "  mov rsp, qword ptr gs:[0]",
    "  swapgs",
    "  sysretq",
    // fn(rip, rsp, a0, a1, a2) -> ! : first entry into ring 3. The iret
    // frame is built on the entry stack so the user-mode page tables can be
    // loaded before the iret.
    ".global awa_enter_user",
    "awa_enter_user:",
    "  cli",
    "  swapgs",
    "  mov rax, qword ptr gs:[48]",
    "  mov qword ptr [rax - 8], {ss}",
    "  mov qword ptr [rax - 16], rsi",
    "  mov qword ptr [rax - 24], 0x202",
    "  mov qword ptr [rax - 32], {cs}",
    "  mov qword ptr [rax - 40], rdi",
    "  lea rsp, [rax - 40]",
    "  mov rax, qword ptr gs:[24]",
    "  mov cr3, rax",
    "  swapgs",
    "  mov rdi, rdx",
    "  mov rsi, rcx",
    "  mov rdx, r8",
    "  xor eax, eax",
    "  xor ebx, ebx",
    "  xor ecx, ecx",
    "  xor ebp, ebp",
    "  xor r8d, r8d",
    "  xor r9d, r9d",
    "  xor r10d, r10d",
    "  xor r11d, r11d",
    "  xor r12d, r12d",
    "  xor r13d, r13d",
    "  xor r14d, r14d",
    "  xor r15d, r15d",
    "  iretq",
    ".text",
    // fn(dst, src, len) -> 0 ok / 1 fault. Only the `rep movsb` may fault.
    ".global awa_uaccess_copy",
    ".global awa_uaccess_insn",
    ".global awa_uaccess_fault",
    "awa_uaccess_copy:",
    "  mov rcx, rdx",
    "awa_uaccess_insn:",
    "  rep movsb",
    "  xor eax, eax",
    "  ret",
    "awa_uaccess_fault:",
    "  mov eax, 1",
    "  ret",
    dispatch = sym crate::syscall::dispatch,
    ss = const USER_SS,
    cs = const USER_CS,
);

unsafe extern "C" {
    fn awa_syscall_entry();
    fn awa_enter_user(rip: u64, rsp: u64, a0: u64, a1: u64, a2: u64) -> !;
    fn awa_uaccess_copy(dst: u64, src: u64, len: u64) -> u64;
    static awa_uaccess_insn: u8;
    static awa_uaccess_fault: u8;
}

static SMAP: AtomicBool = AtomicBool::new(false);

/// Records whether SMAP is on (user copies must then be bracketed by stac/clac).
pub fn set_smap(on: bool) {
    SMAP.store(on, Ordering::Relaxed);
}

/// Enables `syscall` on this CPU and points it at `awa_syscall_entry`.
/// `scratch` must live forever and belong to this CPU.
pub fn init_cpu(scratch: *mut Scratch) {
    // SAFETY: these MSRs only configure how this CPU enters the kernel on
    // `syscall`. STAR: kernel CS 0x08 (SS 0x10); sysret base 0x10 gives user
    // SS 0x18|3 and CS 0x20|3, matching the GDT. FMASK clears IF, TF, DF and
    // AC on entry. GS base 0 / kernel GS base = scratch for `swapgs`.
    unsafe {
        write_msr(MSR_EFER, read_msr(MSR_EFER) | EFER_SCE);
        write_msr(MSR_STAR, (0x10u64 << 48) | (0x08u64 << 32));
        write_msr(MSR_LSTAR, awa_syscall_entry as usize as u64);
        write_msr(MSR_FMASK, 0x4_0700);
        write_msr(MSR_GS_BASE, 0);
        write_msr(MSR_KERNEL_GS_BASE, scratch as u64);
    }
}

/// Drops the current thread into ring 3 at `rip` with stack `rsp` and the
/// three arguments in rdi, rsi, rdx. All other registers are zeroed.
///
/// # Safety
/// The current address space must map `rip` executable and `rsp` writable
/// for the user, and this CPU's TSS rsp0 and scratch must point at this
/// thread's kernel stack.
pub unsafe fn enter_user(rip: u64, rsp: u64, a0: u64, a1: u64, a2: u64) -> ! {
    // SAFETY: the caller's contract above.
    unsafe { awa_enter_user(rip, rsp, a0, a1, a2) }
}

/// True if [ptr, ptr+len) lies entirely in user space.
pub fn user_range_ok(ptr: u64, len: u64) -> bool {
    ptr.checked_add(len).is_some_and(|end| end <= USER_TOP)
}

fn raw_copy(dst: u64, src: u64, len: u64) -> bool {
    if len == 0 {
        return true;
    }
    let smap = SMAP.load(Ordering::Relaxed);
    // SAFETY: stac/clac (only when SMAP is on, else they would #UD) open the
    // user-access window for exactly this copy. The copy routine either
    // finishes or faults at its one `rep movsb`, which the page-fault handler
    // turns into a return value of 1. Callers validated that the user side is
    // below USER_TOP and the kernel side is a valid buffer of `len` bytes.
    unsafe {
        if smap {
            asm!("stac", options(nomem, nostack));
        }
        let r = awa_uaccess_copy(dst, src, len);
        if smap {
            asm!("clac", options(nomem, nostack));
        }
        r == 0
    }
}

/// Copies `dst.len()` bytes from user address `src`. False on a bad range or fault.
pub fn copy_from_user(dst: &mut [u8], src: u64) -> bool {
    user_range_ok(src, dst.len() as u64) && raw_copy(dst.as_mut_ptr() as u64, src, dst.len() as u64)
}

/// Copies `src` to user address `dst`. False on a bad range or fault.
pub fn copy_to_user(dst: u64, src: &[u8]) -> bool {
    user_range_ok(dst, src.len() as u64) && raw_copy(dst, src.as_ptr() as u64, src.len() as u64)
}

/// Page-fault hook: a fault inside a user copy resumes at the failure exit.
pub fn recover(rip: u64) -> Option<u64> {
    let insn = core::ptr::addr_of!(awa_uaccess_insn) as u64;
    (rip == insn).then(|| core::ptr::addr_of!(awa_uaccess_fault) as u64)
}
