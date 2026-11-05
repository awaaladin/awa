//! Always-on kernel tracing (docs/spec/10-tracing.md), first version.
//!
//! Each CPU has a ring of fixed-size 32-byte events, written only by that
//! CPU with interrupts disabled, so no lock is needed. The ring overwrites
//! itself: the last `RING_EVENTS` events are always available.
//!
//! TODO(awa): TraceSession capability and a reader service (Phase 4+).

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

pub const RING_EVENTS: usize = 2048;

pub const KIND_SWITCH: u32 = 1;
pub const KIND_MARK: u32 = 2;

pub static ENABLED: AtomicBool = AtomicBool::new(true);

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Event {
    pub tsc: u64,
    pub kind: u32,
    pub cpu: u32,
    pub a: u64,
    pub b: u64,
}

pub struct Ring {
    /// Heap storage (64 KiB is too big to build on a kernel stack).
    events: UnsafeCell<alloc::boxed::Box<[Event]>>,
    head: AtomicU64,
}

// SAFETY: only the owning CPU writes `events`, with interrupts disabled;
// readers on other CPUs may see a partly written event, which is acceptable
// for diagnostics and never unsound for plain integers.
unsafe impl Sync for Ring {}

impl Ring {
    pub fn new() -> Self {
        let zero = Event { tsc: 0, kind: 0, cpu: 0, a: 0, b: 0 };
        Self { events: UnsafeCell::new(alloc::vec![zero; RING_EVENTS].into_boxed_slice()), head: AtomicU64::new(0) }
    }

    /// Total events ever written to this ring.
    pub fn written(&self) -> u64 {
        self.head.load(Ordering::Relaxed)
    }

    fn push(&self, e: Event) {
        let h = self.head.load(Ordering::Relaxed);
        // SAFETY: single writer (this CPU, interrupts off), index in bounds.
        unsafe { (*self.events.get())[(h as usize) % RING_EVENTS] = e };
        self.head.store(h + 1, Ordering::Release);
    }
}

/// Records an event on this CPU. Cheap: one branch, one 32-byte store.
#[inline]
pub fn emit(kind: u32, a: u64, b: u64, tsc: u64) {
    if !ENABLED.load(Ordering::Relaxed) {
        return;
    }
    let irq = crate::arch::interrupts_enabled();
    crate::arch::disable_interrupts();
    let c = crate::sched::this_cpu();
    c.trace.push(Event { tsc, kind, cpu: c.index as u32, a, b });
    if irq {
        crate::arch::enable_interrupts();
    }
}

pub fn switch(_cpu: usize, prev: u64, next: u64, tsc: u64) {
    emit(KIND_SWITCH, prev, next, tsc);
}
