//! Fault probes: touch an address on purpose and learn whether the CPU
//! refused, without crashing the kernel. Used by the boot self-tests to prove
//! that W^X and no-execute are really enforced.
//!
//! Each probe is a tiny assembly routine. When the page-fault handler sees a
//! fault inside a probe (or, for `exec`, at the probed address while a probe
//! is armed), it records the fault and resumes at `awa_probe_fault_ret`,
//! which returns 1 instead of 0.

use core::arch::global_asm;
use core::sync::atomic::{AtomicU64, Ordering};

global_asm!(
    ".global awa_probe_read",
    ".global awa_probe_read_insn",
    ".global awa_probe_write",
    ".global awa_probe_write_insn",
    ".global awa_probe_exec",
    ".global awa_probe_fault_ret",
    // u32 awa_probe_read(addr: rdi)
    "awa_probe_read:",
    "  xor eax, eax",
    "awa_probe_read_insn:",
    "  mov dl, byte ptr [rdi]",
    "  ret",
    // u32 awa_probe_write(addr: rdi): writes back the byte already there, so a
    // successful write changes nothing (it must still fault on read-only pages).
    "awa_probe_write:",
    "  xor eax, eax",
    "  mov dl, byte ptr [rdi]",
    "awa_probe_write_insn:",
    "  mov byte ptr [rdi], dl",
    "  ret",
    // u32 awa_probe_exec(addr: rdi): calls addr, which must hold a `ret`.
    "awa_probe_exec:",
    "  xor eax, eax",
    "  call rdi",
    "  ret",
    "awa_probe_fault_ret:",
    "  mov eax, 1",
    "  ret",
);

unsafe extern "C" {
    fn awa_probe_read(addr: u64) -> u32;
    fn awa_probe_write(addr: u64) -> u32;
    fn awa_probe_exec(addr: u64) -> u32;
    static awa_probe_read_insn: u8;
    static awa_probe_write_insn: u8;
    static awa_probe_fault_ret: u8;
}

/// Non-zero while an exec probe is running: the address being executed.
static EXEC_TARGET: AtomicU64 = AtomicU64::new(0);
/// Last fault caught by a probe: (CR2, page-fault error code).
static LAST_ADDR: AtomicU64 = AtomicU64::new(0);
static LAST_CODE: AtomicU64 = AtomicU64::new(0);

/// What happened when a probe touched memory.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Allowed,
    /// The CPU raised a page fault: faulting address and error code.
    Faulted { addr: u64, code: u64 },
}

impl core::fmt::Debug for Outcome {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Outcome::Allowed => write!(f, "allowed"),
            Outcome::Faulted { addr, code } => write!(f, "page fault at {:#x}, error code {:#x}", addr, code),
        }
    }
}

fn outcome(faulted: u32) -> Outcome {
    if faulted == 0 {
        Outcome::Allowed
    } else {
        Outcome::Faulted { addr: LAST_ADDR.load(Ordering::Relaxed), code: LAST_CODE.load(Ordering::Relaxed) }
    }
}

pub fn read(addr: u64) -> Outcome {
    // SAFETY: the probe only reads one byte; a fault is caught by the page
    // fault handler and turned into a return value. Reading device memory
    // could have side effects, so callers only probe RAM or unmapped addresses.
    outcome(unsafe { awa_probe_read(addr) })
}

pub fn write_same(addr: u64) -> Outcome {
    // SAFETY: writes back the byte that is already there, so a write that is
    // (wrongly) allowed changes no memory contents.
    outcome(unsafe { awa_probe_write(addr) })
}

/// # Safety
/// `addr` must hold a `ret` (0xC3) byte, so that if the CPU wrongly allows
/// execution the probe simply returns.
pub unsafe fn exec(addr: u64) -> Outcome {
    EXEC_TARGET.store(addr, Ordering::SeqCst);
    // SAFETY: per the contract above, executing `addr` at worst runs `ret`.
    let r = unsafe { awa_probe_exec(addr) };
    EXEC_TARGET.store(0, Ordering::SeqCst);
    outcome(r)
}

/// Called by the page-fault handler. If the fault belongs to a probe, records
/// it and returns the address to resume at.
pub fn recover(rip: u64, cr2: u64, code: u64) -> Option<u64> {
    let read_insn = core::ptr::addr_of!(awa_probe_read_insn) as u64;
    let write_insn = core::ptr::addr_of!(awa_probe_write_insn) as u64;
    let resume = core::ptr::addr_of!(awa_probe_fault_ret) as u64;
    let exec_target = EXEC_TARGET.load(Ordering::SeqCst);
    // For read/write probes the stack is untouched; for exec the `call` pushed
    // a return address that `awa_probe_fault_ret`'s `ret` will consume.
    let hit = rip == read_insn || rip == write_insn || (exec_target != 0 && rip == exec_target);
    if !hit {
        return None;
    }
    LAST_ADDR.store(cr2, Ordering::Relaxed);
    LAST_CODE.store(code, Ordering::Relaxed);
    Some(resume)
}
