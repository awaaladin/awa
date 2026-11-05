//! AWA's own 4-level page tables, replacing the bootloader's.
//!
//! Layout (see docs/spec/01 §4):
//!   direct map (HHDM)        same offset as Limine, RW + NX, global
//!   0xffff_c000_0000_0000    kernel heap, RW + NX
//!   0xffff_c100_0000_0000    kernel stacks, each with an unmapped guard page below
//!   0xffff_ffff_8000_0000    kernel image, per section: text R+X, rodata R+NX, data RW+NX
//! No mapping is ever writable and executable at once (W^X).

use super::{frames, phys_to_virt, PAGE_SIZE};
use crate::boot::{MemKind, MemoryMap};
use crate::sync::SpinLock;

const PRESENT: u64 = 1 << 0;
const WRITABLE: u64 = 1 << 1;
const WRITE_THROUGH: u64 = 1 << 3; // with PAT as programmed: index 1 = write-combining
const HUGE: u64 = 1 << 7;
const GLOBAL: u64 = 1 << 8;
const NO_EXECUTE: u64 = 1 << 63;
const ADDR_MASK: u64 = 0x000f_ffff_ffff_f000;
const HUGE_SIZE: u64 = 2 * 1024 * 1024;

pub const HEAP_BASE: u64 = 0xffff_c000_0000_0000;
const STACKS_BASE: u64 = 0xffff_c100_0000_0000;
const STACK_SLOT: u64 = 256 * 1024; // room for a stack plus its guard page

/// What a mapping allows.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Access {
    ReadExecute,
    ReadOnly,
    ReadWrite,
    /// Read-write, write-combining, never executable (framebuffer).
    ReadWriteCombining,
}

impl Access {
    fn bits(self) -> u64 {
        match self {
            Access::ReadExecute => PRESENT | GLOBAL,
            Access::ReadOnly => PRESENT | GLOBAL | NO_EXECUTE,
            Access::ReadWrite => PRESENT | WRITABLE | GLOBAL | NO_EXECUTE,
            Access::ReadWriteCombining => PRESENT | WRITABLE | GLOBAL | NO_EXECUTE | WRITE_THROUGH,
        }
    }
}

struct Space {
    pml4: u64,
    next_stack: u64,
}

static KERNEL_SPACE: SpinLock<Option<Space>> = SpinLock::new(None);

/// Runs `f` with the kernel address space locked (all page-table edits go through here).
fn with_space<R>(f: impl FnOnce(&mut Space) -> R) -> R {
    let mut g = KERNEL_SPACE.lock();
    f(g.as_mut().expect("kernel address space not built"))
}

fn table(phys: u64) -> &'static mut [u64; 512] {
    // SAFETY: `phys` is a page-table frame owned by the kernel address space
    // and reachable through the direct map.
    unsafe { &mut *(phys_to_virt(phys) as *mut [u64; 512]) }
}

/// Returns the next-level table for `entry`, creating it if needed.
fn next_table(entry: &mut u64) -> Result<u64, &'static str> {
    if *entry & PRESENT == 0 {
        let frame = frames::alloc_zeroed().ok_or("out of memory for page tables")?;
        // Intermediate levels allow everything; the leaf entry decides.
        *entry = frame | PRESENT | WRITABLE;
    } else if *entry & HUGE != 0 {
        return Err("address already covered by a huge page");
    }
    Ok(*entry & ADDR_MASK)
}

fn indices(virt: u64) -> [usize; 4] {
    [(virt >> 39) as usize & 511, (virt >> 30) as usize & 511, (virt >> 21) as usize & 511, (virt >> 12) as usize & 511]
}

fn map_4k(pml4: u64, virt: u64, phys: u64, access: Access) -> Result<(), &'static str> {
    let [i4, i3, i2, i1] = indices(virt);
    let pdpt = next_table(&mut table(pml4)[i4])?;
    let pd = next_table(&mut table(pdpt)[i3])?;
    let pt = next_table(&mut table(pd)[i2])?;
    table(pt)[i1] = phys | access.bits();
    Ok(())
}

fn map_2m(pml4: u64, virt: u64, phys: u64, access: Access) -> Result<(), &'static str> {
    let [i4, i3, i2, _] = indices(virt);
    let pdpt = next_table(&mut table(pml4)[i4])?;
    let pd = next_table(&mut table(pdpt)[i3])?;
    let e = &mut table(pd)[i2];
    if *e & PRESENT != 0 && *e & HUGE == 0 {
        return Err("2 MiB slot already split into 4 KiB pages");
    }
    *e = phys | access.bits() | HUGE;
    Ok(())
}

/// Maps [phys, phys+len) at virt, using 2 MiB pages where alignment allows.
fn map_range(pml4: u64, virt: u64, phys: u64, len: u64, access: Access) -> Result<(), &'static str> {
    let mut off = 0;
    while off < len {
        let (v, p) = (virt + off, phys + off);
        if v % HUGE_SIZE == 0 && p % HUGE_SIZE == 0 && len - off >= HUGE_SIZE {
            map_2m(pml4, v, p, access)?;
            off += HUGE_SIZE;
        } else {
            map_4k(pml4, v, p, access)?;
            off += PAGE_SIZE;
        }
    }
    Ok(())
}

unsafe extern "C" {
    static __kernel_start: u8;
    static __text_start: u8;
    static __text_end: u8;
    static __rodata_start: u8;
    static __rodata_end: u8;
    static __data_start: u8;
    static __kernel_end: u8;
}

fn sym(s: &u8) -> u64 {
    s as *const u8 as u64
}

/// Address of the first byte of kernel code (for the W^X self-test).
pub fn kernel_text_start() -> u64 {
    // SAFETY: taking the address of a linker symbol; its contents are never read here.
    sym(unsafe { &__text_start })
}

/// Address of the first byte of kernel read-only data (for the self-test).
pub fn kernel_rodata_start() -> u64 {
    // SAFETY: as above.
    sym(unsafe { &__rodata_start })
}

pub struct Built {
    pub pml4: u64,
    pub mapped_direct: u64,
}

/// Builds the kernel address space (not yet active).
pub fn build(map: &MemoryMap, hhdm: u64, kernel_phys: u64, kernel_virt: u64) -> Result<Built, &'static str> {
    let pml4 = frames::alloc_zeroed().ok_or("out of memory for the PML4")?;
    let mut mapped_direct = 0;

    // Direct map: every region that holds RAM, firmware tables or the framebuffer.
    for r in map.iter() {
        let access = match r.kind {
            MemKind::Usable | MemKind::BootloaderReclaimable | MemKind::KernelAndModules | MemKind::AcpiReclaimable | MemKind::AcpiNvs => Access::ReadWrite,
            MemKind::Framebuffer => Access::ReadWriteCombining,
            _ => continue,
        };
        let start = r.base / PAGE_SIZE * PAGE_SIZE;
        let end = (r.base + r.length).next_multiple_of(PAGE_SIZE);
        map_range(pml4, hhdm + start, start, end - start, access)?;
        mapped_direct += end - start;
    }

    // Kernel image, section by section.
    // SAFETY: taking the addresses of linker symbols only.
    let (k0, t0, t1, r0, r1, d0, k1) = unsafe {
        (sym(&__kernel_start), sym(&__text_start), sym(&__text_end), sym(&__rodata_start), sym(&__rodata_end), sym(&__data_start), sym(&__kernel_end))
    };
    let phys = |v: u64| v - kernel_virt + kernel_phys;
    let sections = [
        (k0, t0, Access::ReadWrite),                  // Limine requests
        (t0, t1.next_multiple_of(PAGE_SIZE), Access::ReadExecute),
        (r0, r1.next_multiple_of(PAGE_SIZE), Access::ReadOnly),
        (d0, k1, Access::ReadWrite),                  // data + bss
    ];
    for (start, end, access) in sections {
        let mut v = start;
        while v < end {
            map_4k(pml4, v, phys(v), access)?;
            v += PAGE_SIZE;
        }
    }

    *KERNEL_SPACE.lock() = Some(Space { pml4, next_stack: STACKS_BASE });
    Ok(Built { pml4, mapped_direct })
}

/// Maps fresh zeroed frames at [virt, virt + pages*4K) in the kernel space.
pub fn map_new(virt: u64, pages: u64, access: Access) -> Result<(), &'static str> {
    with_space(|s| {
        for i in 0..pages {
            let f = frames::alloc_zeroed().ok_or("out of memory")?;
            map_4k(s.pml4, virt + i * PAGE_SIZE, f, access)?;
        }
        Ok(())
    })
}

/// The kernel's top-level page table (loaded by every CPU).
pub fn kernel_pml4() -> u64 {
    with_space(|s| s.pml4)
}

/// A kernel stack: [bottom, top) is mapped; the page below `bottom` never is.
#[derive(Clone, Copy)]
pub struct Stack {
    pub bottom: u64,
    pub top: u64,
}

pub fn new_stack(pages: u64) -> Result<Stack, &'static str> {
    if pages * PAGE_SIZE + PAGE_SIZE > STACK_SLOT {
        return Err("stack too large for a slot");
    }
    let slot = with_space(|s| {
        let slot = s.next_stack;
        s.next_stack += STACK_SLOT;
        slot
    });
    let bottom = slot + PAGE_SIZE; // the slot's first page stays unmapped: the guard
    map_new(bottom, pages, Access::ReadWrite)?;
    Ok(Stack { bottom, top: bottom + pages * PAGE_SIZE })
}

/// Maps device memory (uncached) at its direct-map address and returns it.
pub fn map_mmio(phys: u64, len: u64, hhdm: u64) -> Result<u64, &'static str> {
    const UNCACHED: u64 = PRESENT | WRITABLE | GLOBAL | NO_EXECUTE | (1 << 4) | WRITE_THROUGH; // PCD|PWT -> PAT index 3 = UC
    let pml4 = kernel_pml4();
    let _edit = KERNEL_SPACE.lock(); // serialise with other page-table edits
    let start = phys / PAGE_SIZE * PAGE_SIZE;
    let end = (phys + len).next_multiple_of(PAGE_SIZE);
    let mut p = start;
    while p < end {
        let [i4, i3, i2, i1] = indices(hhdm + p);
        let pdpt = next_table(&mut table(pml4)[i4])?;
        let pd = next_table(&mut table(pdpt)[i3])?;
        let pt = next_table(&mut table(pd)[i2])?;
        table(pt)[i1] = p | UNCACHED;
        crate::arch::cpu::invlpg(hhdm + p);
        p += PAGE_SIZE;
    }
    Ok(hhdm + phys)
}
