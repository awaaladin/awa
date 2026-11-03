//! Minimal synchronisation for early boot.
//!
//! Phase 2 runs on one CPU. `InitCell` holds data that is written during that
//! single-threaded start-up and only read (or touched by the same CPU) later.
//! TODO(awa): replace with per-CPU data and real locks when SMP arrives (Phase 3).

use core::cell::UnsafeCell;

pub struct InitCell<T>(UnsafeCell<T>);

// SAFETY: until Phase 3 there is exactly one CPU and no preemption of the
// code that uses an `InitCell`; every access goes through `get`/`get_mut`,
// whose callers uphold the single-threaded contract documented there.
unsafe impl<T> Sync for InitCell<T> {}

impl<T> InitCell<T> {
    pub const fn new(v: T) -> Self {
        Self(UnsafeCell::new(v))
    }

    /// Raw pointer, for handing addresses to hardware (GDT, IDT, TSS).
    pub fn as_ptr(&self) -> *mut T {
        self.0.get()
    }

    /// # Safety
    /// No other reference to the contents may be live, and the caller runs on
    /// the boot CPU with nothing else able to touch this cell concurrently
    /// (interrupt handlers included).
    #[allow(clippy::mut_from_ref)]
    pub unsafe fn get_mut(&self) -> &mut T {
        // SAFETY: exclusivity is the caller's contract (see above).
        unsafe { &mut *self.0.get() }
    }
}
