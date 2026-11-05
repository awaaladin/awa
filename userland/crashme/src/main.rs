//! crashme: deliberately reads kernel memory. The CPU refuses (page fault),
//! the kernel ends this process and reports it; nothing else is affected.

#![no_std]
#![no_main]

use awa::Startup;

#[unsafe(no_mangle)]
fn awa_main(_start: Startup) -> i64 {
    let kernel = 0xffff_ffff_8000_1000 as *const u64;
    // SAFETY: deliberately invalid: this read must fault.
    let v = unsafe { core::ptr::read_volatile(kernel) };
    v as i64 // never reached
}
