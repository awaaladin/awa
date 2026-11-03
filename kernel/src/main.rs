//! AWA kernel entry point.
//!
//! Boot order: serial, CPU tables (GDT/TSS/IDT), physical memory, AWA's own
//! page tables (W^X, NX, guard pages), switch to a guarded kernel stack,
//! heap, self-tests, framebuffer.

#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(clippy::undocumented_unsafe_blocks)]

extern crate alloc;

mod arch;
mod boot;
mod console;
mod mem;
mod rng;
mod selftest;
mod serial;
mod sync;

use arch::{apic, cpu, gdt};
use core::panic::PanicInfo;
use core::sync::atomic::{AtomicU64, Ordering};

/// Printed on serial and drawn on the framebuffer once boot succeeded.
/// `tools/awa.py test` waits for exactly this line.
const BOOT_MESSAGE: &str = "AWA kernel booted";

const KERNEL_STACK_PAGES: u64 = 16; // 64 KiB
const IST_STACK_PAGES: u64 = 4; // 16 KiB

/// Bottom of the guarded kernel stack (the self-test probes just below it).
pub static KERNEL_STACK_BOTTOM: AtomicU64 = AtomicU64::new(0);

#[unsafe(no_mangle)]
extern "C" fn kmain() -> ! {
    serial::init();
    serial_println!("AWA: kernel entry");

    if !boot::base_revision_supported() {
        serial_println!("AWA: FATAL: bootloader does not support Limine base revision {}", boot::BASE_REVISION);
        arch::halt_forever();
    }

    arch::init();
    serial_println!("AWA: CPU tables loaded (GDT, TSS with IST stacks, IDT)");

    if let Err(e) = init_memory() {
        panic!("memory setup failed: {e}");
    }
    let stack = KERNEL_STACK_BOTTOM.load(Ordering::Relaxed) + KERNEL_STACK_PAGES * mem::PAGE_SIZE;
    // SAFETY: `stack` is the top of a freshly mapped 64 KiB stack (16-byte
    // aligned, unused) in the page tables that are now active.
    unsafe { cpu::switch_stack(stack, kmain_on_kernel_stack) }
}

fn init_memory() -> Result<(), &'static str> {
    let map = boot::memory_map().ok_or("bootloader gave no memory map")?;
    let hhdm = boot::hhdm_offset().ok_or("bootloader gave no direct-map offset")?;
    let (kphys, kvirt) = boot::kernel_address().ok_or("bootloader gave no kernel address")?;
    mem::set_hhdm(hhdm);

    let frames = mem::frames::init(&map)?;
    serial_println!("AWA: physical memory: {} MiB usable", frames.total * 4 / 1024);

    let built = mem::paging::build(&map, hhdm, kphys, kvirt)?;
    let p = cpu::enable_protections();
    cpu::program_pat();
    // SAFETY: `built.pml4` maps the kernel image (including this code and its
    // statics) and the direct map (which holds the current Limine stack and
    // the framebuffer) with the same offsets the bootloader used.
    unsafe { cpu::write_cr3(built.pml4) };
    serial_println!(
        "AWA: own page tables active: {} MiB direct-mapped; NX {}, WP {}, SMEP {}, SMAP {}, UMIP {}",
        built.mapped_direct / (1024 * 1024),
        on(p.nx),
        on(p.write_protect),
        on(p.smep),
        on(p.smap),
        on(p.umip)
    );

    let stack = mem::paging::new_stack(KERNEL_STACK_PAGES)?;
    KERNEL_STACK_BOTTOM.store(stack.bottom, Ordering::Relaxed);
    for ist in [gdt::IST_DOUBLE_FAULT, gdt::IST_NMI, gdt::IST_MACHINE_CHECK] {
        gdt::set_ist(ist, mem::paging::new_stack(IST_STACK_PAGES)?.top);
    }
    mem::heap::init()?;
    serial_println!("AWA: kernel stack and IST stacks have guard pages; heap {} KiB", mem::heap::HEAP_SIZE / 1024);
    Ok(())
}

fn on(b: bool) -> &'static str {
    if b { "on" } else { "unavailable" }
}

extern "C" fn kmain_on_kernel_stack() -> ! {
    let features = cpu::features();
    match apic::init(features.x2apic) {
        Ok(t) => serial_println!(
            "AWA: local APIC ({}) timer at {} Hz ({} APIC ticks each); TSC {} MHz{}",
            if t.x2apic { "x2APIC" } else { "xAPIC" },
            apic::TICK_HZ,
            t.apic_ticks_per_tick,
            t.tsc_hz / 1_000_000,
            if features.invariant_tsc { ", invariant" } else { "" }
        ),
        Err(e) => panic!("timer setup failed: {e}"),
    }
    apic::enable_interrupts();
    serial_println!("AWA: random numbers seeded from {}", rng::init(features.rdrand));

    let mut tests = selftest::Report::new();
    selftest::cpu(&mut tests);
    selftest::memory(&mut tests);
    selftest::timer(&mut tests);
    selftest::random(&mut tests);
    tests.finish();

    match boot::take_framebuffer() {
        Ok(fb) => {
            serial_println!("AWA: framebuffer {}x{}, pitch {}", fb.width(), fb.height(), fb.pitch());
            let mut console = console::Console::new(fb);
            console.clear();
            console.draw_banner(BOOT_MESSAGE);
        }
        Err(reason) => serial_println!("AWA: no framebuffer output: {}", reason),
    }

    serial_println!("{}", BOOT_MESSAGE);
    arch::halt_forever()
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial_println!("AWA: KERNEL PANIC: {}", info);
    arch::idt::backtrace();
    arch::halt_forever()
}
