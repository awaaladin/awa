//! Global descriptor table and task state segment.
//!
//! Long mode barely uses segmentation, but the CPU still needs code/data
//! descriptors, and the TSS provides the interrupt stack table (IST): known
//! good stacks for double faults, NMIs and machine checks, so those still work
//! when the current stack is broken.

use crate::sync::InitCell;
use core::arch::asm;
use core::mem::size_of;

pub const KERNEL_CODE: u16 = 0x08;
pub const KERNEL_DATA: u16 = 0x10;
const TSS_SELECTOR: u16 = 0x28;

/// IST slots (1-based in the IDT, 0-based in `Tss::ist`).
pub const IST_DOUBLE_FAULT: u8 = 1;
pub const IST_NMI: u8 = 2;
pub const IST_MACHINE_CHECK: u8 = 3;

const IST_STACK_SIZE: usize = 16 * 1024;

#[repr(C, align(16))]
struct Stack([u8; IST_STACK_SIZE]);

// TODO(awa): give IST stacks unmapped guard pages once the kernel owns its
// page tables (Phase 2 step B maps them with guards).
static IST_STACKS: InitCell<[Stack; 3]> = InitCell::new([const { Stack([0; IST_STACK_SIZE]) }; 3]);

#[repr(C, packed(4))]
struct Tss {
    _reserved0: u32,
    rsp: [u64; 3],
    _reserved1: u64,
    ist: [u64; 7],
    _reserved2: u64,
    _reserved3: u16,
    iomap_base: u16,
}

static TSS: InitCell<Tss> = InitCell::new(Tss {
    _reserved0: 0,
    rsp: [0; 3],
    _reserved1: 0,
    ist: [0; 7],
    _reserved2: 0,
    _reserved3: 0,
    iomap_base: size_of::<Tss>() as u16, // no I/O permission bitmap
});

/// null, kernel code, kernel data, user data, user code, TSS (two slots).
static GDT: InitCell<[u64; 7]> = InitCell::new([
    0,
    0x00af_9a00_0000_ffff, // kernel code: present, ring 0, executable, 64-bit
    0x00cf_9200_0000_ffff, // kernel data: present, ring 0, writable
    0x00cf_f200_0000_ffff, // user data: ring 3 (used from Phase 4)
    0x00af_fa00_0000_ffff, // user code: ring 3, 64-bit (used from Phase 4)
    0,
    0,
]);

#[repr(C, packed)]
struct DescriptorPointer {
    limit: u16,
    base: u64,
}

/// Loads AWA's GDT and TSS and reloads every segment register.
pub fn init() {
    // SAFETY: called once on the boot CPU before interrupts are enabled, so
    // nothing else references the TSS, GDT or IST stacks yet.
    unsafe {
        let stacks = IST_STACKS.get_mut();
        let tss = TSS.get_mut();
        for (i, stack) in stacks.iter_mut().enumerate() {
            // Stacks grow down: the IST entry is the top of each stack.
            tss.ist[i] = stack.0.as_mut_ptr().add(IST_STACK_SIZE) as u64;
        }

        let base = TSS.as_ptr() as u64;
        let limit = (size_of::<Tss>() - 1) as u64;
        let gdt = GDT.get_mut();
        // 64-bit available TSS descriptor (type 0x9, present).
        gdt[5] = (limit & 0xffff)
            | ((base & 0x00ff_ffff) << 16)
            | (0x89 << 40)
            | (((limit >> 16) & 0xf) << 48)
            | (((base >> 24) & 0xff) << 56);
        gdt[6] = base >> 32;

        let ptr = DescriptorPointer { limit: (size_of::<[u64; 7]>() - 1) as u16, base: GDT.as_ptr() as u64 };
        // SAFETY: `ptr` describes a valid, 'static GDT whose kernel code and
        // data descriptors are at the selectors used below. The far return
        // reloads CS; the data segments are reloaded with KERNEL_DATA, and
        // the TSS selector points at a valid, not-yet-busy TSS descriptor.
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
