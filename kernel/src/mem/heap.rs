//! Kernel heap: a fixed region of fresh frames at HEAP_BASE, managed by
//! `linked_list_allocator`. Mapped read-write and never executable.
//!
//! TODO(awa): grow on demand, and charge allocations to memory accounts (Phase 5).

use super::paging::{self, Access, HEAP_BASE};
use super::PAGE_SIZE;
use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::ptr::{self, NonNull};
use core::sync::atomic::{AtomicBool, Ordering};
use linked_list_allocator::Heap;

pub const HEAP_SIZE: u64 = 4 * 1024 * 1024;

struct KernelHeap {
    locked: AtomicBool,
    heap: UnsafeCell<Heap>,
}

// SAFETY: every access to `heap` happens while `locked` is held (see `with`).
unsafe impl Sync for KernelHeap {}

impl KernelHeap {
    fn with<R>(&self, f: impl FnOnce(&mut Heap) -> R) -> R {
        while self.locked.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
            core::hint::spin_loop();
        }
        // SAFETY: we hold the lock, so this is the only reference to the heap.
        let r = f(unsafe { &mut *self.heap.get() });
        self.locked.store(false, Ordering::Release);
        r
    }
}

#[global_allocator]
static HEAP: KernelHeap = KernelHeap { locked: AtomicBool::new(false), heap: UnsafeCell::new(Heap::empty()) };

// SAFETY: allocate/deallocate forward to linked_list_allocator's Heap, which
// returns blocks that fit `layout` and are not handed out twice; the lock
// serialises access.
unsafe impl GlobalAlloc for KernelHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.with(|h| h.allocate_first_fit(layout).map_or(ptr::null_mut(), |p| p.as_ptr()))
    }

    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        if let Some(nn) = NonNull::new(p) {
            // SAFETY: GlobalAlloc's contract: `p` came from `alloc` with this layout.
            self.with(|h| unsafe { h.deallocate(nn, layout) });
        }
    }
}

pub fn init() -> Result<(), &'static str> {
    paging::map_new(HEAP_BASE, HEAP_SIZE / PAGE_SIZE, Access::ReadWrite)?;
    // SAFETY: [HEAP_BASE, HEAP_BASE + HEAP_SIZE) was just mapped read-write,
    // is used by nothing else, and init runs once before any allocation.
    HEAP.with(|h| unsafe { h.init(HEAP_BASE as *mut u8, HEAP_SIZE as usize) });
    Ok(())
}

pub struct Usage {
    pub used: usize,
    pub free: usize,
}

pub fn usage() -> Usage {
    HEAP.with(|h| Usage { used: h.used(), free: h.free() })
}
