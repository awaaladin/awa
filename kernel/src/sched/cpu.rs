//! Per-CPU state, reached through the GS segment base.

use super::{admit_into, release_from, Reservation, RunQueue, Thread};
use crate::arch;
use crate::sync::SpinLock;
use crate::trace;
use alloc::boxed::Box;
use core::ptr;
use core::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

pub const MAX_CPUS: usize = 64;

#[repr(C)]
pub struct Cpu {
    /// Must stay the first field: `gs:[0]` reads it to find this block.
    self_ptr: u64,
    pub index: usize,
    pub apic_id: u32,
    pub rq: SpinLock<RunQueue>,
    current: AtomicPtr<Thread>,
    prev: AtomicPtr<Thread>,
    idle: AtomicPtr<Thread>,
    pub trace: trace::Ring,
    /// This CPU's TSS (for rsp0) and syscall scratch block.
    tss: AtomicPtr<arch::gdt::Tss>,
    scratch: AtomicPtr<arch::syscall::Scratch>,
}

static CPUS: [AtomicPtr<Cpu>; MAX_CPUS] = [const { AtomicPtr::new(ptr::null_mut()) }; MAX_CPUS];
static COUNT: AtomicUsize = AtomicUsize::new(0);

/// Sets up CPU `index` on the CPU itself: its TSS pointer and the syscall
/// entry. (CPUs find their block by local APIC ID, never through GS, which
/// user programs can change.)
pub fn register_this(index: usize, apic_id: u32, tss: *mut arch::gdt::Tss, scratch: *mut arch::syscall::Scratch) -> &'static Cpu {
    let c = prepare(index, apic_id);
    if arch::cpu::has_rdtscp() {
        arch::cpu::set_cpu_index(index as u32);
        if index == 0 {
            FAST_INDEX.store(true, Ordering::Release);
        }
    }
    c.tss.store(tss, Ordering::Release);
    c.scratch.store(scratch, Ordering::Release);
    arch::syscall::init_cpu(scratch);
    c
}

/// Creates the per-CPU block for a CPU that is not running yet (so other
/// CPUs can already enqueue work for it).
pub fn prepare(index: usize, apic_id: u32) -> &'static Cpu {
    assert!(index < MAX_CPUS, "too many CPUs");
    let existing = CPUS[index].load(Ordering::Acquire);
    if !existing.is_null() {
        // SAFETY: entries are only ever set to leaked, never-freed Cpu blocks.
        return unsafe { &*existing };
    }
    let c: &'static mut Cpu = Box::leak(Box::new(Cpu {
        self_ptr: 0,
        index,
        apic_id,
        rq: SpinLock::new(RunQueue::new()),
        current: AtomicPtr::new(ptr::null_mut()),
        prev: AtomicPtr::new(ptr::null_mut()),
        idle: AtomicPtr::new(ptr::null_mut()),
        trace: trace::Ring::new(),
        tss: AtomicPtr::new(ptr::null_mut()),
        scratch: AtomicPtr::new(ptr::null_mut()),
    }));
    c.self_ptr = c as *const Cpu as u64;
    CPUS[index].store(c, Ordering::Release);
    COUNT.fetch_max(index + 1, Ordering::AcqRel);
    c
}

/// Set once when the boot CPU registers: whether RDTSCP gives the CPU index.
static FAST_INDEX: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

pub fn this_cpu() -> &'static Cpu {
    if FAST_INDEX.load(Ordering::Relaxed) {
        let i = arch::cpu::cpu_index_fast() as usize;
        if i < MAX_CPUS {
            let p = CPUS[i].load(Ordering::Acquire);
            // SAFETY: entries are only ever set to leaked, never-freed Cpu blocks.
            if let Some(c) = unsafe { p.as_ref() } {
                return c;
            }
        }
    }
    let id = arch::apic::id();
    for i in 0..count() {
        let p = CPUS[i].load(Ordering::Acquire);
        // SAFETY: entries are only ever set to leaked, never-freed Cpu blocks.
        if let Some(c) = unsafe { p.as_ref() } {
            if c.apic_id == id {
                return c;
            }
        }
    }
    panic!("this CPU (APIC ID {id}) is not registered")
}

pub fn get(index: usize) -> &'static Cpu {
    let p = CPUS[index].load(Ordering::Acquire);
    assert!(!p.is_null(), "CPU {index} not registered");
    // SAFETY: see `prepare`.
    unsafe { &*p }
}

pub fn count() -> usize {
    COUNT.load(Ordering::Acquire)
}

/// The CPU with the fewest ready threads (spreading new work).
pub fn least_loaded() -> usize {
    (0..count())
        .filter(|&i| !CPUS[i].load(Ordering::Acquire).is_null())
        .min_by_key(|&i| get(i).rq.lock().load())
        .unwrap_or(0)
}

impl Cpu {
    pub fn current(&self) -> &'static Thread {
        self.try_current().expect("no current thread on this CPU")
    }

    pub fn try_current(&self) -> Option<&'static Thread> {
        // SAFETY: only leaked, never-freed threads are stored.
        unsafe { self.current.load(Ordering::Acquire).as_ref() }
    }

    pub fn set_current(&self, t: &'static Thread) {
        self.current.store(t as *const Thread as *mut Thread, Ordering::Release);
    }

    pub fn set_prev(&self, t: &'static Thread) {
        self.prev.store(t as *const Thread as *mut Thread, Ordering::Release);
    }

    pub fn take_prev(&self) -> Option<&'static Thread> {
        // SAFETY: as in `try_current`.
        unsafe { self.prev.swap(ptr::null_mut(), Ordering::AcqRel).as_ref() }
    }

    /// Prepares this CPU's entry path for a user thread: its kernel stack and
    /// its process's kernel-mode and user-mode page tables.
    pub fn set_user_context(&self, kernel_stack_top: u64, kernel_cr3: u64, user_cr3: u64) {
        let scratch = self.scratch.load(Ordering::Acquire);
        // SAFETY: the scratch block belongs to this CPU (set in `register_this`),
        // lives forever, and is only read by this CPU's entry code.
        unsafe {
            (*scratch).kernel_rsp = kernel_stack_top;
            (*scratch).kernel_cr3 = kernel_cr3;
            (*scratch).user_cr3 = user_cr3;
        }
    }

    pub fn set_idle(&self, t: &'static Thread) {
        self.idle.store(t as *const Thread as *mut Thread, Ordering::Release);
    }

    pub fn idle(&self) -> &'static Thread {
        // SAFETY: as in `try_current`; set before the CPU starts scheduling.
        unsafe { self.idle.load(Ordering::Acquire).as_ref() }.expect("idle thread missing")
    }

    pub fn is_idle(&self, t: &Thread) -> bool {
        ptr::eq(self.idle.load(Ordering::Acquire), t)
    }

    pub fn admit(&self, r: Reservation) -> bool {
        admit_into(&mut self.rq.lock(), r)
    }

    pub fn release(&self, r: Reservation) {
        release_from(&mut self.rq.lock(), r);
    }
}
