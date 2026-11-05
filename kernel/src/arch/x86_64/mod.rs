//! x86_64 primitives. Inline assembly is only allowed under `arch/`.

pub mod apic;
pub mod context;
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

pub fn interrupts_enabled() -> bool {
    let flags: u64;
    // SAFETY: reads RFLAGS through the stack; no other effect.
    unsafe { asm!("pushfq", "pop {}", out(reg) flags, options(nomem, preserves_flags)) };
    flags & (1 << 9) != 0
}

pub fn disable_interrupts() {
    // SAFETY: masking interrupts cannot break memory safety.
    unsafe { asm!("cli", options(nomem, nostack)) };
}

pub fn enable_interrupts() {
    // SAFETY: the IDT has a handler for every vector that can fire.
    unsafe { asm!("sti", options(nomem, nostack)) };
}

/// Points GS at this CPU's per-CPU block (its first field must be a pointer to itself).
pub fn set_cpu_local(ptr: u64) {
    // SAFETY: IA32_GS_BASE only changes what `gs:` addresses refer to; kernel
    // code reads per-CPU data only through `cpu_local`.
    unsafe { cpu::write_msr(0xC000_0101, ptr) };
}

/// The per-CPU block set with `set_cpu_local`.
pub fn cpu_local() -> u64 {
    let p: u64;
    // SAFETY: reads the self pointer at gs:0, which `set_cpu_local` set up
    // before any caller can run on this CPU.
    unsafe { asm!("mov {}, gs:[0]", out(reg) p, options(nostack, preserves_flags, readonly)) };
    p
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
