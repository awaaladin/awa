//! x86_64 primitives. Inline assembly is only allowed under `arch/`.

pub mod cpu;
pub mod gdt;
pub mod idt;
pub mod port;
pub mod probe;

use core::arch::asm;

/// Loads AWA's GDT/TSS and IDT. Interrupts stay disabled.
pub fn init() {
    gdt::init();
    idt::init();
}

/// Stops this CPU permanently.
pub fn halt_forever() -> ! {
    loop {
        // SAFETY: `cli` and `hlt` touch no memory and no Rust-visible state.
        // Masking interrupts first means only an NMI can wake the CPU, and the
        // loop puts it straight back to sleep.
        unsafe { asm!("cli", "hlt", options(nomem, nostack)) };
    }
}

/// Raises a breakpoint exception (used by the self-test).
pub fn breakpoint() {
    // SAFETY: `int3` traps to the breakpoint handler, which returns normally.
    unsafe { asm!("int3", options(nomem, nostack)) };
}

/// CPUID leaf/subleaf -> (eax, ebx, ecx, edx).
pub fn cpuid(leaf: u32, sub: u32) -> (u32, u32, u32, u32) {
    // CPUID exists on every x86_64 CPU; the intrinsic is safe to call.
    let r = core::arch::x86_64::__cpuid_count(leaf, sub);
    (r.eax, r.ebx, r.ecx, r.edx)
}
