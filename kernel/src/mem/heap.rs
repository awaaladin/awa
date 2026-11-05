//! Kernel heap: a fixed region of fresh frames at HEAP_BASE, managed by
//! `linked_list_allocator`. Mapped read-write and never executable.
//!
//! The lock disables interrupts while held: the timer interrupt can allocate
//! (run-queue lists grow), and must never spin on a lock its own CPU holds.
//!
//! TODO(awa): grow on demand, and charge allocations to memory accounts (Phase 5).

use super::paging::{self, Access, HEAP_BASE};
use super::PAGE_SIZE;
use crate::sync::SpinLock;
use core::alloc::{GlobalAlloc, Layout};
use core::ptr::{self, NonNull};
use linked_list_allocator::Heap;

pub const HEAP_SIZE: u64 = 8 * 1024 * 1024;

struct KernelHeap(SpinLock<Heap>);

#[global_allocator]
static HEAP: KernelHeap = KernelHeap(SpinLock::new(Heap::empty()));

// SAFETY: allocate/deallocate forward to linked_list_allocator's Heap, which
// returns blocks that fit `layout` and are not handed out twice; the lock
// serialises access across CPUs and against interrupt handlers.
unsafe impl GlobalAlloc for KernelHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.0.lock().allocate_first_fit(layout).map_or(ptr::null_mut(), |p| p.as_ptr())
    }

    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        if let Some(nn) = NonNull::new(p) {
            // SAFETY: GlobalAlloc's contract: `p` came from `alloc` with this layout.
            unsafe { self.0.lock().deallocate(nn, layout) };
        }
    }
}

pub fn init() -> Result<(), &'static str> {
    paging::map_new(HEAP_BASE, HEAP_SIZE / PAGE_SIZE, Access::ReadWrite)?;
    // SAFETY: [HEAP_BASE, HEAP_BASE + HEAP_SIZE) was just mapped read-write,
    // is used by nothing else, and init runs once before any allocation.
    unsafe { HEAP.0.lock().init(HEAP_BASE as *mut u8, HEAP_SIZE as usize) };
    Ok(())
}

pub struct Usage {
    pub used: usize,
}

pub fn usage() -> Usage {
    Usage { used: HEAP.0.lock().used() }
}
