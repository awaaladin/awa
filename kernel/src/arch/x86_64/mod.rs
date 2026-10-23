//! x86_64 primitives. Inline assembly is only allowed under `arch/`.

pub mod port;

use core::arch::asm;

/// Stops this CPU permanently.
pub fn halt_forever() -> ! {
    loop {
        // SAFETY: `cli` and `hlt` touch no memory and no Rust-visible state.
        // Masking interrupts first means only an NMI can wake the CPU, and the
        // loop puts it straight back to sleep.
        unsafe { asm!("cli", "hlt", options(nomem, nostack)) };
    }
}
