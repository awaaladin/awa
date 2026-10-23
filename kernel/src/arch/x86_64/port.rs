//! Legacy x86 I/O-port access.

use core::arch::asm;

/// Writes one byte to an I/O port.
///
/// # Safety
/// The caller must know which device decodes `port` and that writing `value`
/// to it cannot break memory safety (for example by starting DMA).
pub unsafe fn outb(port: u16, value: u8) {
    // SAFETY: the caller upholds the device contract above; `out` itself
    // touches no memory and no stack.
    unsafe { asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags)) };
}

/// Reads one byte from an I/O port.
///
/// # Safety
/// Same contract as [`outb`]: reading `port` must have no side effects that
/// could break memory safety.
pub unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    // SAFETY: the caller upholds the device contract above; `in` itself
    // touches no memory and no stack.
    unsafe { asm!("in al, dx", out("al") value, in("dx") port, options(nomem, nostack, preserves_flags)) };
    value
}
