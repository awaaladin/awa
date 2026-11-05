//! Interrupt descriptor table and CPU exception handlers.
//!
//! Every exception prints what happened and a frame-pointer backtrace to the
//! serial port, then stops the CPU. Two exceptions return normally:
//! breakpoints (`int3`, used by the self-test) and page faults raised inside a
//! fault probe (see `probe.rs`).

use super::gdt::{IST_DOUBLE_FAULT, IST_MACHINE_CHECK, IST_NMI, KERNEL_CODE};
use super::{halt_forever, probe};
use crate::serial_println;
use crate::sync::InitCell;
use core::arch::asm;
use core::sync::atomic::{AtomicU64, Ordering};

/// The frame the CPU pushes on an interrupt or exception.
#[repr(C)]
#[derive(Debug)]
pub struct InterruptStackFrame {
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Entry {
    offset_low: u16,
    selector: u16,
    ist: u8,
    attributes: u8,
    offset_mid: u16,
    offset_high: u32,
    _reserved: u32,
}

impl Entry {
    const MISSING: Entry = Entry { offset_low: 0, selector: 0, ist: 0, attributes: 0, offset_mid: 0, offset_high: 0, _reserved: 0 };

    fn new(handler: u64, ist: u8) -> Entry {
        Entry {
            offset_low: handler as u16,
            selector: KERNEL_CODE,
            ist,
            attributes: 0x8e, // present, ring 0, 64-bit interrupt gate (IF cleared on entry)
            offset_mid: (handler >> 16) as u16,
            offset_high: (handler >> 32) as u32,
            _reserved: 0,
        }
    }
}

static IDT: InitCell<[Entry; 256]> = InitCell::new([Entry::MISSING; 256]);

/// Number of `int3` breakpoints handled (the self-test checks it moves).
pub static BREAKPOINTS: AtomicU64 = AtomicU64::new(0);

type Handler = extern "x86-interrupt" fn(InterruptStackFrame);
type HandlerWithCode = extern "x86-interrupt" fn(InterruptStackFrame, u64);
type Diverging = extern "x86-interrupt" fn(InterruptStackFrame, u64) -> !;

/// Installs a handler for `vector`. Used by later subsystems (timer, APIC).
pub fn set_handler(vector: u8, handler: Handler) {
    // SAFETY: single boot CPU; called with interrupts disabled, so the CPU is
    // not reading this entry while it is rewritten.
    unsafe { IDT.get_mut()[vector as usize] = Entry::new(handler as usize as u64, 0) };
}

pub fn init() {
    // SAFETY: single boot CPU, interrupts disabled, nothing else uses the IDT yet.
    let idt = unsafe { IDT.get_mut() };
    let plain = |h: Handler| Entry::new(h as usize as u64, 0);
    let coded = |h: HandlerWithCode| Entry::new(h as usize as u64, 0);
    idt[0] = plain(divide_error);
    idt[1] = plain(debug);
    idt[2] = Entry::new(nmi as Handler as usize as u64, IST_NMI);
    idt[3] = plain(breakpoint);
    idt[4] = plain(overflow);
    idt[5] = plain(bound_range);
    idt[6] = plain(invalid_opcode);
    idt[7] = plain(device_not_available);
    idt[8] = Entry::new(double_fault as Diverging as usize as u64, IST_DOUBLE_FAULT);
    idt[10] = coded(invalid_tss);
    idt[11] = coded(segment_not_present);
    idt[12] = coded(stack_segment);
    idt[13] = coded(general_protection);
    idt[14] = coded(page_fault);
    idt[16] = plain(x87_floating_point);
    idt[17] = coded(alignment_check);
    idt[18] = Entry::new(machine_check as Handler as usize as u64, IST_MACHINE_CHECK);
    idt[19] = plain(simd_floating_point);
    idt[20] = plain(virtualization);
    idt[21] = coded(control_protection);
    idt[0xff] = plain(spurious);

    load();
}

/// Loads the shared IDT on this CPU (every CPU uses the same table).
pub fn load() {
    #[repr(C, packed)]
    struct Pointer {
        limit: u16,
        base: u64,
    }
    let ptr = Pointer { limit: (core::mem::size_of::<[Entry; 256]>() - 1) as u16, base: IDT.as_ptr() as u64 };
    // SAFETY: `ptr` describes the 'static IDT, fully initialised above with
    // handlers that use the kernel code selector loaded by gdt::init().
    unsafe { asm!("lidt [{}]", in(reg) &ptr, options(readonly, nostack, preserves_flags)) };
}

// ---------------------------------------------------------------- reporting

fn report(name: &str, frame: &InterruptStackFrame, code: Option<u64>) -> ! {
    serial_println!("AWA: KERNEL PANIC: CPU exception: {}", name);
    if let Some(code) = code {
        serial_println!("  error code {:#x}", code);
    }
    serial_println!("  rip {:#018x}  rsp {:#018x}  rflags {:#x}  cs {:#x}", frame.rip, frame.rsp, frame.rflags, frame.cs);
    backtrace();
    halt_forever()
}

/// Walks the frame-pointer chain (the kernel is built with frame pointers).
/// Only follows aligned pointers inside the kernel half, so a corrupt chain
/// cannot cause a second fault.
pub fn backtrace() {
    let mut rbp: u64;
    // SAFETY: reads the rbp register; no memory access.
    unsafe { asm!("mov {}, rbp", out(reg) rbp, options(nomem, nostack, preserves_flags)) };
    serial_println!("  backtrace:");
    for depth in 0..16 {
        if rbp < 0xffff_8000_0000_0000 || rbp % 8 != 0 {
            break;
        }
        // SAFETY: rbp is an aligned kernel-half address taken from the frame
        // chain; kernel stacks are always mapped while the kernel runs.
        let (next, ret) = unsafe { (*(rbp as *const u64), *((rbp + 8) as *const u64)) };
        if ret == 0 {
            break;
        }
        serial_println!("    #{:<2} {:#018x}", depth, ret);
        if next <= rbp {
            break; // the chain must move up the stack
        }
        rbp = next;
    }
}

fn cr2() -> u64 {
    let v: u64;
    // SAFETY: reading CR2 has no side effects.
    unsafe { asm!("mov {}, cr2", out(reg) v, options(nomem, nostack, preserves_flags)) };
    v
}

// ---------------------------------------------------------------- handlers

extern "x86-interrupt" fn breakpoint(_frame: InterruptStackFrame) {
    BREAKPOINTS.fetch_add(1, Ordering::Relaxed);
}

extern "x86-interrupt" fn page_fault(mut frame: InterruptStackFrame, code: u64) {
    let addr = cr2();
    if let Some(resume) = probe::recover(frame.rip, addr, code) {
        // SAFETY: with the x86-interrupt ABI `frame` is the CPU-pushed frame on
        // the stack; changing rip makes `iretq` resume at the probe's recovery
        // label, which is valid code in this kernel.
        unsafe { core::ptr::write_volatile(&mut frame.rip, resume) };
        return;
    }
    serial_println!("AWA: page fault at {:#018x} ({})", addr, describe_pf(code));
    report("page fault", &frame, Some(code));
}

fn describe_pf(code: u64) -> &'static str {
    match (code & 1 != 0, code & 2 != 0, code & 16 != 0) {
        (_, _, true) => "instruction fetch",
        (false, false, _) => "read of unmapped page",
        (false, true, _) => "write to unmapped page",
        (true, false, _) => "read, protection violation",
        (true, true, _) => "write to read-only page",
    }
}

extern "x86-interrupt" fn double_fault(frame: InterruptStackFrame, code: u64) -> ! {
    report("double fault", &frame, Some(code))
}

macro_rules! fatal {
    ($name:ident, $text:literal) => {
        extern "x86-interrupt" fn $name(frame: InterruptStackFrame) {
            report($text, &frame, None)
        }
    };
    ($name:ident, $text:literal, code) => {
        extern "x86-interrupt" fn $name(frame: InterruptStackFrame, code: u64) {
            report($text, &frame, Some(code))
        }
    };
}

fatal!(divide_error, "divide error");
fatal!(debug, "debug");
fatal!(nmi, "non-maskable interrupt");
fatal!(overflow, "overflow");
fatal!(bound_range, "bound range exceeded");
fatal!(invalid_opcode, "invalid opcode");
fatal!(device_not_available, "device not available");
fatal!(x87_floating_point, "x87 floating point");
fatal!(machine_check, "machine check");
fatal!(simd_floating_point, "SIMD floating point");
fatal!(virtualization, "virtualization");
fatal!(invalid_tss, "invalid TSS", code);
fatal!(segment_not_present, "segment not present", code);
fatal!(stack_segment, "stack-segment fault", code);
fatal!(general_protection, "general protection fault", code);
fatal!(alignment_check, "alignment check", code);
fatal!(control_protection, "control protection", code);

/// Spurious interrupts need no EOI and no action.
extern "x86-interrupt" fn spurious(_frame: InterruptStackFrame) {}
