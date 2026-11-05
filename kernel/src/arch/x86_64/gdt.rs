//! Global descriptor table and task state segment, one pair per CPU.
//!
//! Long mode barely uses segmentation, but the CPU still needs code/data
//! descriptors, and the TSS provides the interrupt stack table (IST): known
//! good stacks for double faults, NMIs and machine checks, so those still work
//! when the current stack is broken. A TSS is marked busy once loaded, so
//! every CPU needs its own TSS and therefore its own GDT.

use crate::sync::InitCell;
use alloc::boxed::Box;
use core::arch::asm;
use core::mem::size_of;

pub const KERNEL_CODE: u16 = 0x08;
pub const KERNEL_DATA: u16 = 0x10;
const TSS_SELECTOR: u16 = 0x28;

/// IST slots (1-based in the IDT, 0-based in `Tss::ist`).
pub const IST_DOUBLE_FAULT: u8 = 1;
pub const IST_NMI: u8 = 2;
pub const IST_MACHINE_CHECK: u8 = 3;

const BOOT_IST_STACK_SIZE: usize = 16 * 1024;

#[repr(C, align(16))]
struct Stack([u8; BOOT_IST_STACK_SIZE]);

// Boot-time IST stacks for the boot CPU, used until AWA's own page tables
// exist; then `set_ist` moves it to stacks with unmapped guard pages.
static BOOT_IST_STACKS: InitCell<[Stack; 3]> = InitCell::new([const { Stack([0; BOOT_IST_STACK_SIZE]) }; 3]);

#[repr(C, packed(4))]
pub struct Tss {
    _reserved0: u32,
    rsp: [u64; 3],
    _reserved1: u64,
    ist: [u64; 7],
    _reserved2: u64,
    _reserved3: u16,
    iomap_base: u16,
}

impl Tss {
    const fn empty() -> Tss {
        Tss {
            _reserved0: 0,
            rsp: [0; 3],
            _reserved1: 0,
            ist: [0; 7],
            _reserved2: 0,
            _reserved3: 0,
            iomap_base: size_of::<Tss>() as u16, // no I/O permission bitmap
        }
    }
}

type Gdt = [u64; 7];

/// null, kernel code, kernel data, user data, user code, TSS (two slots).
const GDT_TEMPLATE: Gdt = [
    0,
    0x00af_9a00_0000_ffff, // kernel code: present, ring 0, executable, 64-bit
    0x00cf_9200_0000_ffff, // kernel data: present, ring 0, writable
    0x00cf_f200_0000_ffff, // user data: ring 3 (used from Phase 4)
    0x00af_fa00_0000_ffff, // user code: ring 3, 64-bit (used from Phase 4)
    0,
    0,
];

static BOOT_TSS: InitCell<Tss> = InitCell::new(Tss::empty());
static BOOT_GDT: InitCell<Gdt> = InitCell::new(GDT_TEMPLATE);

#[repr(C, packed)]
struct DescriptorPointer {
    limit: u16,
    base: u64,
}

/// Writes the TSS descriptor into `gdt`, then loads the GDT, reloads every
/// segment register and loads the task register.
///
/// # Safety
/// `gdt` and `tss` must live forever, be used by this CPU only, and `tss`
/// must not be loaded on any other CPU.
unsafe fn load(gdt: *mut Gdt, tss: *const Tss) {
    let base = tss as u64;
    let limit = (size_of::<Tss>() - 1) as u64;
    // SAFETY: the caller guarantees exclusive, 'static access to `gdt`.
    let g = unsafe { &mut *gdt };
    // 64-bit available TSS descriptor (type 0x9, present).
    g[5] = (limit & 0xffff) | ((base & 0x00ff_ffff) << 16) | (0x89 << 40) | (((limit >> 16) & 0xf) << 48) | (((base >> 24) & 0xff) << 56);
    g[6] = base >> 32;

    let ptr = DescriptorPointer { limit: (size_of::<Gdt>() - 1) as u16, base: gdt as u64 };
    // SAFETY: `ptr` describes a valid, 'static GDT whose kernel code and data
    // descriptors are at the selectors used below. The far return reloads CS;
    // the data segments are reloaded with KERNEL_DATA; the TSS selector points
    // at this CPU's own, not-yet-busy TSS descriptor.
    unsafe {
        asm!(
            "lgdt [{ptr}]",
            "push {code}",
            "lea {tmp}, [rip + 2f]",
            "push {tmp}",
            "retfq",
            "2:",
            "mov ds, {data:x}",
            "mov es, {data:x}",
            "mov ss, {data:x}",
            "xor {tmp:e}, {tmp:e}",
            "mov fs, {tmp:x}",
            "mov gs, {tmp:x}",
            "ltr {tss:x}",
            ptr = in(reg) &ptr,
            code = in(reg) u64::from(KERNEL_CODE),
            data = in(reg) KERNEL_DATA,
            tss = in(reg) TSS_SELECTOR,
            // `out`, not `lateout`: tmp is written before `data` and `tss` are read.
            tmp = out(reg) _,
            options(preserves_flags),
        );
    }
}

/// Boot CPU: loads the static GDT and TSS with the boot-time IST stacks.
pub fn init() {
    // SAFETY: called once on the boot CPU before interrupts are enabled and
    // before any other CPU runs, so nothing else references these statics.
    unsafe {
        let stacks = BOOT_IST_STACKS.get_mut();
        let tss = BOOT_TSS.get_mut();
        for (i, stack) in stacks.iter_mut().enumerate() {
            // Stacks grow down: the IST entry is the top of each stack.
            tss.ist[i] = stack.0.as_mut_ptr().add(BOOT_IST_STACK_SIZE) as u64;
        }
        load(BOOT_GDT.as_ptr(), BOOT_TSS.as_ptr());
    }
}

/// Boot CPU: points IST slot `ist` (1-based, as used in the IDT) at a new stack top.
pub fn set_ist(ist: u8, stack_top: u64) {
    assert!((1..=7).contains(&ist));
    // SAFETY: boot CPU, interrupts disabled, before other CPUs start; the CPU
    // only reads the IST entry when delivering an exception that uses it.
    unsafe { BOOT_TSS.get_mut().ist[usize::from(ist - 1)] = stack_top };
}

/// Secondary CPU: builds and loads its own GDT and TSS. `ist_tops` are the
/// tops of its double-fault, NMI and machine-check stacks.
pub fn init_secondary(ist_tops: [u64; 3]) {
    let mut tss = Box::new(Tss::empty());
    for (i, top) in ist_tops.into_iter().enumerate() {
        tss.ist[i] = top;
    }
    let tss: &'static mut Tss = Box::leak(tss);
    let gdt: &'static mut Gdt = Box::leak(Box::new(GDT_TEMPLATE));
    // SAFETY: both were just leaked, so they live forever and belong to this CPU alone.
    unsafe { load(gdt, tss) };
}
