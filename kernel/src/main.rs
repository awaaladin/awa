//! AWA kernel entry point.
//!
//! Boot order: serial, CPU tables (GDT/TSS/IDT), self-tests, framebuffer.
//! Phase 2 is in progress: own page tables, heap and timer are still to come.

#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(clippy::undocumented_unsafe_blocks)]

mod arch;
mod boot;
mod console;
mod selftest;
mod serial;
mod sync;

use core::panic::PanicInfo;

/// Printed on serial and drawn on the framebuffer once early boot succeeded.
/// `tools/awa.py test` waits for exactly this line.
const BOOT_MESSAGE: &str = "AWA kernel booted";

#[unsafe(no_mangle)]
extern "C" fn kmain() -> ! {
    serial::init();
    serial_println!("AWA: kernel entry");

    if !boot::base_revision_supported() {
        serial_println!(
            "AWA: FATAL: bootloader does not support Limine base revision {}",
            boot::BASE_REVISION
        );
        arch::halt_forever();
    }

    arch::init();
    serial_println!("AWA: CPU tables loaded (GDT, TSS with IST stacks, IDT)");

    let mut tests = selftest::Report::new();
    selftest::cpu(&mut tests);
    tests.finish();

    match boot::take_framebuffer() {
        Ok(fb) => {
            serial_println!(
                "AWA: framebuffer {}x{}, pitch {}",
                fb.width(),
                fb.height(),
                fb.pitch()
            );
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
    arch::halt_forever()
}
