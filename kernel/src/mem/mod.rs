//! Memory management: physical frames, page tables, kernel heap.

pub mod frames;
pub mod heap;
pub mod paging;

use core::sync::atomic::{AtomicU64, Ordering};

pub const PAGE_SIZE: u64 = 4096;

static HHDM: AtomicU64 = AtomicU64::new(0);

/// Virtual address of physical address `phys` in the direct map.
pub fn phys_to_virt(phys: u64) -> u64 {
    phys + HHDM.load(Ordering::Relaxed)
}

pub fn hhdm() -> u64 {
    HHDM.load(Ordering::Relaxed)
}

pub fn set_hhdm(offset: u64) {
    HHDM.store(offset, Ordering::Relaxed);
}
