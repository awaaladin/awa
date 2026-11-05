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
mod object;
mod process;
mod rng;
mod sched;
mod selftest;
mod serial;
mod sync;
mod syscall;
mod trace;

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

    // The boot CPU becomes CPU 0 of the scheduler; this code continues as the
    // interactive thread "kmain".
    let me = sched::cpu::register_this(0, apic::id(), gdt::boot_tss());
    let boot_thread = sched::adopt_current("kmain", sched::Class::Interactive);
    match sched::new_idle(0) {
        Ok(idle) => me.set_idle(idle),
        Err(e) => panic!("idle thread: {e}"),
    }
    sched::start_cpu(boot_thread);
    arch::enable_interrupts();
    serial_println!("AWA: random numbers seeded from {}", rng::init(features.rdrand));

    // The timer is measured before other CPUs start, so an emulator that is
    // short of host CPU time does not distort the result.
    let mut tests = selftest::Report::new();
    selftest::timer(&mut tests);

    let expected = start_other_cpus();

    selftest::cpu(&mut tests);
    selftest::memory(&mut tests);
    selftest::random(&mut tests);
    selftest::scheduler(&mut tests, expected);
    run_init(&mut tests);
    let summary = tests.finish();

    match boot::take_framebuffer() {
        Ok(fb) => {
            serial_println!("AWA: framebuffer {}x{}, pitch {}", fb.width(), fb.height(), fb.pitch());
            let mut console = console::Console::new(fb);
            console.clear();
            console.draw_banner(BOOT_MESSAGE);
            let mut lines = summary.lines;
            lines.push(alloc::format!("self-test: {} passed, {} failed", summary.passed, summary.failed));
            console.draw_lines(&lines);
        }
        Err(reason) => serial_println!("AWA: no framebuffer output: {}", reason),
    }

    serial_println!("{}", BOOT_MESSAGE);
    // The boot work is done; this thread ends and the CPUs keep scheduling.
    sched::exit()
}

// ---------------------------------------------------------------- user space

/// Starts init (the first user program) with every other boot file, waits for
/// it to finish its self-tests, and adds its results to the report.
fn run_init(tests: &mut selftest::Report) {
    if let Err(e) = process::init_vdso() {
        tests.check("init-started", false, format_args!("vDSO: {}", e));
        return;
    }
    let modules = boot::modules();
    let Some(init) = modules.iter().find(|m| m.name == "init") else {
        tests.check("init-started", false, format_args!("no 'init' module in the boot files"));
        return;
    };
    let files: alloc::vec::Vec<(&str, &[u8])> = modules.iter().filter(|m| m.name != "init").map(|m| (m.name, m.bytes)).collect();
    let p = match process::start_init(init.bytes, &files) {
        Ok(p) => p,
        Err(e) => {
            tests.check("init-started", false, format_args!("{}", e));
            return;
        }
    };
    serial_println!("AWA: started init (process {}) with {} boot files", p.id, files.len());
    let deadline = apic::tsc() + apic::TSC_HZ.load(Ordering::Relaxed) * 120;
    while p.status().is_none() && apic::tsc() < deadline {
        sched::sleep_ns(5_000_000);
    }
    for (name, ok, detail) in syscall::USER_RESULTS.lock().drain(..) {
        tests.check(&name, ok, format_args!("{}", detail));
    }
    match p.status() {
        Some(0) => tests.check("init-exit", true, format_args!("init finished and exited with code 0")),
        Some(s) => tests.check("init-exit", false, format_args!("init ended with status {:#x}", s)),
        None => tests.check("init-exit", false, format_args!("init did not finish within 120 s")),
    }
}

// ---------------------------------------------------------------- other CPUs

/// CPUs that finished bringing themselves up (the boot CPU included).
pub static ONLINE: AtomicU64 = AtomicU64::new(1);

/// Scheduler index of the CPU with local APIC ID `lapic_id`: the boot CPU is
/// 0, the others follow in the order Limine lists them.
fn cpu_index(cpus: &boot::Cpus, lapic_id: u32) -> usize {
    if lapic_id == cpus.bsp_lapic_id {
        return 0;
    }
    1 + cpus.list.iter().filter(|c| c.lapic_id != cpus.bsp_lapic_id).position(|c| c.lapic_id == lapic_id).expect("CPU not in Limine's list")
}

/// Starts every other CPU and waits (up to 2 s) until they report in.
/// Returns how many CPUs the machine has.
fn start_other_cpus() -> usize {
    let Some(cpus) = boot::cpus() else {
        serial_println!("AWA: bootloader did not report other CPUs; running on one");
        return 1;
    };
    let total = cpus.list.len();
    for info in cpus.list.iter().filter(|c| c.lapic_id != cpus.bsp_lapic_id) {
        // Create the per-CPU block first, so work can be queued for it at once.
        sched::cpu::prepare(cpu_index(&cpus, info.lapic_id), info.lapic_id);
        info.start(ap_entry);
    }
    let deadline = apic::tsc() + apic::TSC_HZ.load(Ordering::Relaxed) * 2;
    while (ONLINE.load(Ordering::Acquire) as usize) < total && apic::tsc() < deadline {
        sched::sleep_ns(1_000_000);
    }
    serial_println!("AWA: {} of {} CPUs online", ONLINE.load(Ordering::Acquire), total);
    total
}

/// First code a secondary CPU runs, on Limine's stack and page tables.
extern "C" fn ap_entry(_info: &'static boot::MpInfo) -> ! {
    cpu::enable_protections();
    cpu::program_pat();
    // SAFETY: the kernel's page tables map this code, its data and the
    // direct map (which holds Limine's stack for this CPU).
    unsafe { cpu::write_cr3(mem::paging::kernel_pml4()) };
    let stack = match mem::paging::new_stack(KERNEL_STACK_PAGES) {
        Ok(s) => s,
        Err(e) => panic!("CPU stack: {e}"),
    };
    // SAFETY: a fresh, aligned, guarded stack used by nothing else.
    unsafe { cpu::switch_stack(stack.top, ap_main) }
}

extern "C" fn ap_main() -> ! {
    let mut ist = [0u64; 3];
    for top in ist.iter_mut() {
        *top = match mem::paging::new_stack(IST_STACK_PAGES) {
            Ok(s) => s.top,
            Err(e) => panic!("CPU IST stack: {e}"),
        };
    }
    let tss = gdt::init_secondary(ist);
    arch::idt::load();
    apic::init_secondary();
    let cpus = boot::cpus().expect("started by Limine, so the CPU list exists");
    let index = cpu_index(&cpus, apic::id());
    let me = sched::cpu::register_this(index, apic::id(), tss);
    // This CPU's start-up code becomes its idle thread.
    let idle = sched::adopt_current("idle", sched::Class::Idle);
    me.set_idle(idle);
    sched::start_cpu(idle);
    ONLINE.fetch_add(1, Ordering::AcqRel);
    loop {
        arch::enable_interrupts();
        apic::wait_for_interrupt();
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    crate::serial::_print_force(format_args!("AWA: KERNEL PANIC: {}\n", info));
    arch::idt::backtrace();
    arch::halt_forever()
}
