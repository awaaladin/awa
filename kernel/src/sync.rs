//! Synchronisation primitives.
//!
//! `SpinLock` is the general lock: it disables interrupts on this CPU while
//! held (so an interrupt handler can never deadlock against the code it
//! interrupted) and spins across CPUs. `InitCell` remains for data written
//! once during single-threaded boot and never contended afterwards.

use crate::arch;
use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};

pub struct SpinLock<T> {
    locked: AtomicBool,
    value: UnsafeCell<T>,
}

// SAFETY: the value is only reachable through a guard, and at most one guard
// exists at a time (the atomic flag), so sharing the lock across CPUs is
// sound whenever T may be sent between them.
unsafe impl<T: Send> Sync for SpinLock<T> {}
// SAFETY: moving the lock moves the value; same requirement.
unsafe impl<T: Send> Send for SpinLock<T> {}

pub struct Guard<'a, T> {
    lock: &'a SpinLock<T>,
    irq_was_enabled: bool,
}

impl<T> SpinLock<T> {
    pub const fn new(value: T) -> Self {
        Self { locked: AtomicBool::new(false), value: UnsafeCell::new(value) }
    }

    pub fn lock(&self) -> Guard<'_, T> {
        let irq_was_enabled = arch::interrupts_enabled();
        arch::disable_interrupts();
        while self.locked.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
            while self.locked.load(Ordering::Relaxed) {
                core::hint::spin_loop();
            }
        }
        Guard { lock: self, irq_was_enabled }
    }

    /// Takes the lock only if it is free (used on the panic path).
    pub fn try_lock(&self) -> Option<Guard<'_, T>> {
        let irq_was_enabled = arch::interrupts_enabled();
        arch::disable_interrupts();
        if self.locked.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_ok() {
            Some(Guard { lock: self, irq_was_enabled })
        } else {
            if irq_was_enabled {
                arch::enable_interrupts();
            }
            None
        }
    }
}

impl<T> Deref for Guard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        // SAFETY: the guard proves we hold the lock.
        unsafe { &*self.lock.value.get() }
    }
}

impl<T> DerefMut for Guard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: the guard proves we hold the lock, and `&mut self` makes this unique.
        unsafe { &mut *self.lock.value.get() }
    }
}

impl<T> Drop for Guard<'_, T> {
    fn drop(&mut self) {
        self.lock.locked.store(false, Ordering::Release);
        if self.irq_was_enabled {
            arch::enable_interrupts();
        }
    }
}

/// Data written once during single-threaded boot, then only read.
pub struct InitCell<T>(UnsafeCell<T>);

// SAFETY: writes happen only through `get_mut`, whose callers guarantee they
// run before other CPUs start or with exclusive access (see its contract).
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
    /// No other reference to the contents may be live: call only on the boot
    /// CPU before other CPUs run, with interrupts disabled, or where the
    /// caller otherwise has exclusive access.
    #[allow(clippy::mut_from_ref)]
    pub unsafe fn get_mut(&self) -> &mut T {
        // SAFETY: exclusivity is the caller's contract (see above).
        unsafe { &mut *self.0.get() }
    }
}
