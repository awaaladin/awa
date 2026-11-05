//! Kernel thread context switching.
//!
//! A switch saves the callee-saved registers (rbp, rbx, r12-r15) on the old
//! thread's stack, stores its stack pointer, loads the new one and pops the
//! new thread's registers. Everything else is already saved by the Rust
//! calling convention at the call site. Floating point/SIMD state is not
//! switched: the kernel is built without SSE (target x86_64-unknown-none).
//! TODO(awa): save extended state once user threads exist (Phase 4).

use core::arch::global_asm;

global_asm!(
    ".global awa_context_switch",
    // fn(save_rsp: *mut u64 [rdi], new_rsp: u64 [rsi])
    "awa_context_switch:",
    "  push rbp",
    "  push rbx",
    "  push r12",
    "  push r13",
    "  push r14",
    "  push r15",
    "  mov [rdi], rsp",
    "  mov rsp, rsi",
    "  pop r15",
    "  pop r14",
    "  pop r13",
    "  pop r12",
    "  pop rbx",
    "  pop rbp",
    "  ret",
    ".global awa_thread_trampoline",
    // First code a new thread runs: r12 holds its argument.
    "awa_thread_trampoline:",
    "  mov rdi, r12",
    "  call {start}",
    "  ud2",
    start = sym crate::sched::thread_start,
);

unsafe extern "C" {
    fn awa_context_switch(save_rsp: *mut u64, new_rsp: u64);
    fn awa_thread_trampoline();
}

/// Saves the current context into `*save_rsp` and resumes the one at `new_rsp`.
///
/// # Safety
/// `new_rsp` must be a stack pointer produced by an earlier switch away from
/// a thread, or by `prepare_stack`, on a mapped stack that no other CPU is
/// running on; `save_rsp` must stay valid until this thread is resumed.
pub unsafe fn switch(save_rsp: *mut u64, new_rsp: u64) {
    // SAFETY: the caller's contract above.
    unsafe { awa_context_switch(save_rsp, new_rsp) };
}

/// Lays out a fresh stack so that switching to it runs
/// `thread_start(arg)` through the trampoline. Returns the initial rsp.
///
/// # Safety
/// `top` must be the 16-byte-aligned top of a mapped, writable stack of at
/// least 64 bytes that nothing else uses.
pub unsafe fn prepare_stack(top: u64, arg: u64) -> u64 {
    let slots: [u64; 7] = [
        0,                                  // r15
        0,                                  // r14
        0,                                  // r13
        arg,                                // r12 -> rdi in the trampoline
        0,                                  // rbx
        0,                                  // rbp (end of the frame chain)
        awa_thread_trampoline as usize as u64, // return address
    ];
    let rsp = top - 8 * slots.len() as u64;
    // SAFETY: [rsp, top) lies inside the caller's stack (see contract).
    unsafe { core::ptr::copy_nonoverlapping(slots.as_ptr(), rsp as *mut u64, slots.len()) };
    rsp
}
