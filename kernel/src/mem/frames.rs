//! Physical frame allocator: one bit per 4 KiB frame of RAM.
//!
//! The bitmap itself lives in the first usable region big enough to hold it.
//! Memory below 1 MiB is never handed out (kept for firmware quirks and the
//! SMP start-up trampoline in Phase 3). Bootloader-reclaimable memory is not
//! reclaimed yet.

use super::{phys_to_virt, PAGE_SIZE};
use crate::boot::{MemKind, MemoryMap};
use crate::sync::InitCell;

const LOW_MEMORY_END: u64 = 1 << 20;

struct Bitmap {
    /// 1 = in use, 0 = free.
    words: &'static mut [u64],
    frames: usize,
    /// Frames that were usable at boot (the most that can ever be free).
    usable: usize,
    free: usize,
    /// Next word to look at (next-fit keeps allocation O(1) on average).
    cursor: usize,
}

static FRAMES: InitCell<Option<Bitmap>> = InitCell::new(None);

#[derive(Clone, Copy, Debug)]
pub struct Stats {
    pub total: usize,
    pub free: usize,
}

fn bitmap() -> &'static mut Bitmap {
    // SAFETY: single boot CPU with interrupts disabled whenever frames are
    // allocated (no interrupt handler allocates). TODO(awa): lock in Phase 3.
    unsafe { FRAMES.get_mut() }.as_mut().expect("frame allocator used before init")
}

pub fn init(map: &MemoryMap) -> Result<Stats, &'static str> {
    let top = map
        .iter()
        .filter(|r| r.kind == MemKind::Usable)
        .map(|r| r.base + r.length)
        .max()
        .ok_or("no usable memory")?;
    let frames = (top / PAGE_SIZE) as usize;
    let words = frames.div_ceil(64);
    let bytes = (words * 8) as u64;
    let bytes_pages = bytes.div_ceil(PAGE_SIZE) * PAGE_SIZE;

    let home = map
        .iter()
        .filter(|r| r.kind == MemKind::Usable && r.base >= LOW_MEMORY_END)
        .find(|r| r.length >= bytes_pages)
        .ok_or("no region large enough for the frame bitmap")?;

    // SAFETY: `home` is usable RAM that nothing else owns yet, mapped by the
    // direct map, page aligned, and at least `bytes` long.
    let words_slice = unsafe { core::slice::from_raw_parts_mut(phys_to_virt(home.base) as *mut u64, words) };
    words_slice.fill(u64::MAX);
    let mut bm = Bitmap { words: words_slice, frames, usable: 0, free: 0, cursor: 0 };

    for r in map.iter().filter(|r| r.kind == MemKind::Usable) {
        let start = r.base.max(LOW_MEMORY_END).next_multiple_of(PAGE_SIZE);
        let end = (r.base + r.length) / PAGE_SIZE * PAGE_SIZE;
        let mut f = start;
        while f < end {
            bm.set_free(f);
            f += PAGE_SIZE;
        }
    }
    let mut f = home.base;
    while f < home.base + bytes_pages {
        bm.set_used(f);
        f += PAGE_SIZE;
    }

    bm.usable = bm.free;
    let stats = Stats { total: bm.usable, free: bm.free };
    // SAFETY: single boot CPU, nothing else references FRAMES yet.
    unsafe { *FRAMES.get_mut() = Some(bm) };
    Ok(stats)
}

impl Bitmap {
    fn index(addr: u64) -> (usize, u64) {
        let frame = (addr / PAGE_SIZE) as usize;
        (frame / 64, 1u64 << (frame % 64))
    }

    fn set_free(&mut self, addr: u64) {
        let (w, bit) = Self::index(addr);
        if self.words[w] & bit != 0 {
            self.words[w] &= !bit;
            self.free += 1;
        }
    }

    fn set_used(&mut self, addr: u64) {
        let (w, bit) = Self::index(addr);
        if self.words[w] & bit == 0 {
            self.words[w] |= bit;
            self.free -= 1;
        }
    }
}

/// Allocates one 4 KiB frame; returns its physical address. Contents are undefined.
pub fn alloc() -> Option<u64> {
    let bm = bitmap();
    let n = bm.words.len();
    for i in 0..n {
        let w = (bm.cursor + i) % n;
        let word = bm.words[w];
        if word != u64::MAX {
            let bit = (!word).trailing_zeros() as usize;
            let frame = w * 64 + bit;
            if frame >= bm.frames {
                continue;
            }
            bm.words[w] |= 1 << bit;
            bm.free -= 1;
            bm.cursor = w;
            return Some(frame as u64 * PAGE_SIZE);
        }
    }
    None
}

/// Allocates one frame filled with zeros.
pub fn alloc_zeroed() -> Option<u64> {
    let f = alloc()?;
    // SAFETY: `f` is a frame we just took from the allocator (so nobody else
    // uses it), and the direct map covers all usable RAM with write access.
    unsafe { core::ptr::write_bytes(phys_to_virt(f) as *mut u8, 0, PAGE_SIZE as usize) };
    Some(f)
}

/// Returns a frame to the allocator.
///
/// # Panics
/// On a double free or an address that was never allocatable: both are kernel bugs.
pub fn free(addr: u64) {
    assert!(addr % PAGE_SIZE == 0, "freeing unaligned frame {addr:#x}");
    let bm = bitmap();
    let (w, bit) = Bitmap::index(addr);
    assert!(w < bm.words.len() && bm.words[w] & bit != 0, "double free of frame {addr:#x}");
    bm.words[w] &= !bit;
    bm.free += 1;
}

pub fn stats() -> Stats {
    let bm = bitmap();
    Stats { total: bm.usable, free: bm.free }
}
