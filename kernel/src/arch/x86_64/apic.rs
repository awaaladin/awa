//! Local APIC and its timer, calibrated against the legacy PIT.
//!
//! The 8259 PICs are remapped away from the exception vectors and masked.
//! The local APIC runs in x2APIC mode when the CPU supports it (MSR access),
//! otherwise in xAPIC mode through uncached MMIO.
//!
//! TODO(awa): TSC-deadline one-shot mode for a tickless scheduler (Phase 3);
//! I/O APIC routing for device interrupts (needs ACPI MADT, Phase 3/7).

use super::cpu::{read_msr, write_msr};
use super::idt::{self, InterruptStackFrame};
use super::port::{inb, outb};
use crate::mem;
use core::arch::asm;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

pub const TIMER_VECTOR: u8 = 0x20;
pub const SPURIOUS_VECTOR: u8 = 0xff;
pub const TICK_HZ: u64 = 100;

const MSR_APIC_BASE: u32 = 0x1b;
const APIC_ENABLE: u64 = 1 << 11;
const X2APIC_ENABLE: u64 = 1 << 10;

// Register offsets (xAPIC MMIO); x2APIC MSR = 0x800 + offset / 16.
const REG_EOI: u32 = 0xb0;
const REG_SVR: u32 = 0xf0;
const REG_LVT_TIMER: u32 = 0x320;
const REG_TIMER_INITIAL: u32 = 0x380;
const REG_TIMER_CURRENT: u32 = 0x390;
const REG_TIMER_DIVIDE: u32 = 0x3e0;
const LVT_MASKED: u32 = 1 << 16;
const LVT_PERIODIC: u32 = 1 << 17;

static X2APIC: AtomicBool = AtomicBool::new(false);
static MMIO_BASE: AtomicU64 = AtomicU64::new(0);

/// Timer interrupts since boot.
pub static TICKS: AtomicU64 = AtomicU64::new(0);
/// TSC ticks per second, measured at boot.
pub static TSC_HZ: AtomicU64 = AtomicU64::new(0);

fn read(reg: u32) -> u32 {
    if X2APIC.load(Ordering::Relaxed) {
        read_msr(0x800 + reg / 16) as u32
    } else {
        let addr = MMIO_BASE.load(Ordering::Relaxed) + u64::from(reg);
        // SAFETY: `addr` is a local APIC register inside the uncached MMIO
        // page mapped by `init`; APIC registers are 32-bit aligned.
        unsafe { core::ptr::read_volatile(addr as *const u32) }
    }
}

fn write(reg: u32, value: u32) {
    if X2APIC.load(Ordering::Relaxed) {
        // SAFETY: x2APIC register MSRs only affect this CPU's local APIC, and
        // every register written here is documented as writable.
        unsafe { write_msr(0x800 + reg / 16, u64::from(value)) };
    } else {
        let addr = MMIO_BASE.load(Ordering::Relaxed) + u64::from(reg);
        // SAFETY: as in `read`.
        unsafe { core::ptr::write_volatile(addr as *mut u32, value) };
    }
}

/// Remaps the two 8259 PICs to vectors 0x20-0x2f and masks every line, so a
/// stray legacy interrupt can never look like a CPU exception.
fn disable_pic() {
    // SAFETY: the standard 8259 initialisation sequence on its fixed ports
    // (0x20/0x21 master, 0xa0/0xa1 slave); it only reprograms the PICs.
    unsafe {
        outb(0x20, 0x11);
        outb(0xa0, 0x11);
        outb(0x21, 0x20);
        outb(0xa1, 0x28);
        outb(0x21, 0x04);
        outb(0xa1, 0x02);
        outb(0x21, 0x01);
        outb(0xa1, 0x01);
        outb(0x21, 0xff);
        outb(0xa1, 0xff);
    }
}

fn rdtsc() -> u64 {
    let (lo, hi): (u32, u32);
    // SAFETY: rdtsc only reads the time-stamp counter.
    unsafe { asm!("rdtsc", out("eax") lo, out("edx") hi, options(nomem, nostack, preserves_flags)) };
    (u64::from(hi) << 32) | u64::from(lo)
}

pub fn tsc() -> u64 {
    rdtsc()
}

/// Busy-waits `ms` milliseconds using PIT channel 2 (fixed 1.193182 MHz).
fn pit_wait_ms(ms: u64) {
    let count = (1_193_182 * ms / 1000).min(0xffff) as u16;
    // SAFETY: port 0x61 bit 0 gates PIT channel 2 and bit 1 the PC speaker
    // (kept off); 0x43/0x42 program channel 2 only, which nothing else uses.
    unsafe {
        let gate = inb(0x61);
        outb(0x61, (gate & !0x02) | 0x01);
        outb(0x43, 0b1011_0000); // channel 2, lobyte/hibyte, mode 0, binary
        outb(0x42, count as u8);
        outb(0x42, (count >> 8) as u8);
        while inb(0x61) & 0x20 == 0 {}
    }
}

pub struct TimerInfo {
    pub x2apic: bool,
    pub apic_ticks_per_tick: u32,
    pub tsc_hz: u64,
}

/// Brings up the local APIC and a periodic timer at TICK_HZ. Interrupts stay disabled.
pub fn init(x2apic_supported: bool) -> Result<TimerInfo, &'static str> {
    disable_pic();

    let base = read_msr(MSR_APIC_BASE);
    if x2apic_supported {
        // SAFETY: enabling the APIC (and x2APIC mode, which CPUID reported) on
        // this CPU only changes how its own APIC registers are accessed.
        unsafe { write_msr(MSR_APIC_BASE, base | APIC_ENABLE | X2APIC_ENABLE) };
        X2APIC.store(true, Ordering::Relaxed);
    } else {
        // SAFETY: as above, without x2APIC.
        unsafe { write_msr(MSR_APIC_BASE, base | APIC_ENABLE) };
        let phys = base & 0x000f_ffff_ffff_f000;
        MMIO_BASE.store(mem::paging::map_mmio(phys, 4096, mem::hhdm())?, Ordering::Relaxed);
    }

    idt::set_handler(TIMER_VECTOR, timer_interrupt);
    idt::set_handler(SPURIOUS_VECTOR, spurious_interrupt);
    write(REG_SVR, 0x100 | u32::from(SPURIOUS_VECTOR)); // software-enable the APIC

    // Calibrate: how far do the APIC timer and the TSC count in 50 ms of PIT time?
    write(REG_TIMER_DIVIDE, 0x3); // divide by 16
    write(REG_LVT_TIMER, LVT_MASKED);
    write(REG_TIMER_INITIAL, u32::MAX);
    let t0 = rdtsc();
    pit_wait_ms(50);
    let t1 = rdtsc();
    let elapsed = u32::MAX - read(REG_TIMER_CURRENT);
    write(REG_TIMER_INITIAL, 0);
    if elapsed == 0 {
        return Err("APIC timer did not count during calibration");
    }
    let tsc_hz = (t1 - t0) * 20;
    TSC_HZ.store(tsc_hz, Ordering::Relaxed);

    let per_tick = (u64::from(elapsed) * 20 / TICK_HZ) as u32;
    write(REG_LVT_TIMER, u32::from(TIMER_VECTOR) | LVT_PERIODIC);
    write(REG_TIMER_INITIAL, per_tick.max(1));

    Ok(TimerInfo { x2apic: X2APIC.load(Ordering::Relaxed), apic_ticks_per_tick: per_tick, tsc_hz })
}

fn eoi() {
    write(REG_EOI, 0);
}

extern "x86-interrupt" fn timer_interrupt(_frame: InterruptStackFrame) {
    TICKS.fetch_add(1, Ordering::Relaxed);
    eoi();
}

/// Spurious APIC interrupts must not be acknowledged.
extern "x86-interrupt" fn spurious_interrupt(_frame: InterruptStackFrame) {}

pub fn enable_interrupts() {
    // SAFETY: the IDT has handlers for every vector that can now fire (the
    // timer and spurious vectors; the PICs are masked).
    unsafe { asm!("sti", options(nomem, nostack)) };
}

/// Sleeps until the next interrupt.
pub fn wait_for_interrupt() {
    // SAFETY: `hlt` with interrupts enabled resumes on the next interrupt.
    unsafe { asm!("hlt", options(nomem, nostack, preserves_flags)) };
}
