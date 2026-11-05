//! Threads and the scheduler (docs/spec/03-scheduler.md).
//!
//! Classes, strictly ordered: frame-deadline > interactive > batch > idle.
//! - Frame-deadline: earliest deadline first among threads with budget left.
//!   Each has a reservation (runtime R every period P), enforced like a
//!   constant-bandwidth server: a thread that uses up R before its period ends
//!   is demoted to interactive until the next period, so it cannot starve
//!   others. Admission control (sum of R/P per CPU <= 70 %) refuses
//!   reservations that would not fit.
//! - Interactive: fair (lowest virtual runtime first), 4 ms slices.
//! - Batch: fair, 50 ms slices; members of a gang go before other batch work.
//! - Idle: one per CPU, runs only when nothing else can.
//!
//! Each CPU has its own run queue. Lock order: run queue, then a thread's
//! `sched` lock. Time is measured in TSC ticks.

pub mod cpu;

use crate::arch::{apic, context};
use crate::mem::paging;
use crate::sync::SpinLock;
use crate::trace;
use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicU64, Ordering};
pub use cpu::{this_cpu, Cpu};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Class {
    FrameDeadline,
    Interactive,
    Batch,
    Idle,
}

impl Class {
    fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum State {
    Ready,
    Running,
    Sleeping,
    /// Waiting for an event (a message, a process exit); in no run queue.
    Blocked,
    Dead,
}

#[derive(Clone, Copy)]
pub struct Reservation {
    pub runtime: u64,
    pub period: u64,
}

pub struct Params {
    pub class: Class,
    pub state: State,
    /// CPU whose run queue owns this thread.
    pub cpu: usize,
    pub pinned: Option<usize>,
    pub gang: Option<u32>,
    vruntime: u64,
    slice_start: u64,
    last_start: u64,
    wake_at: u64,
    // frame-deadline
    reservation: Option<Reservation>,
    period_start: u64,
    budget: i64,
    demoted: bool,
    /// Set when a frame-deadline job is released; used to measure wakeup latency.
    released_at: u64,
    /// True from the moment a CPU picks this thread until its context has
    /// been saved on the way out (see `finish_switch`).
    on_cpu: bool,
    /// A wake-up arrived before the thread managed to block.
    wake_pending: bool,
}

/// Counters kept for every thread (always on).
#[derive(Default)]
pub struct Stats {
    pub runtime: [AtomicU64; 4],
    pub switches_in: AtomicU64,
    pub jobs: AtomicU64,
    pub misses: AtomicU64,
    pub overruns: AtomicU64,
    pub max_wake_latency: AtomicU64,
    pub first_run: AtomicU64,
}

pub struct Thread {
    pub id: u64,
    pub name: &'static str,
    /// Saved stack pointer. Written only by the CPU switching away from this
    /// thread, read only by the CPU switching to it; never both at once,
    /// because a thread is in no run queue until its switch-away completed.
    rsp: UnsafeCell<u64>,
    entry: Option<fn(u64)>,
    arg: u64,
    /// Top of this thread's own kernel stack (0 for adopted boot threads).
    pub kstack_top: u64,
    /// The process a user thread belongs to (None for kernel threads).
    pub process: Option<Arc<crate::process::Process>>,
    pub sched: SpinLock<Params>,
    pub stats: Stats,
}

// SAFETY: `rsp` follows the hand-off protocol documented on the field; every
// other mutable field is behind `sched`'s lock or atomic.
unsafe impl Sync for Thread {}

const INTERACTIVE_SLICE_NS: u64 = 4_000_000;
const BATCH_SLICE_NS: u64 = 50_000_000;
const FD_ADMISSION_PERCENT: u64 = 70;
const STACK_PAGES: u64 = 8; // 32 KiB

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

pub fn ns_to_tsc(ns: u64) -> u64 {
    (apic::TSC_HZ.load(Ordering::Relaxed) as u128 * ns as u128 / 1_000_000_000) as u64
}

pub fn tsc_to_ns(t: u64) -> u64 {
    (t as u128 * 1_000_000_000 / apic::TSC_HZ.load(Ordering::Relaxed).max(1) as u128) as u64
}

fn new_thread(name: &'static str, class: Class, entry: Option<fn(u64)>, arg: u64) -> &'static Thread {
    new_thread_full(name, class, entry, arg, 0, None)
}

fn new_thread_full(name: &'static str, class: Class, entry: Option<fn(u64)>, arg: u64, kstack_top: u64, process: Option<Arc<crate::process::Process>>) -> &'static Thread {
    Box::leak(Box::new(Thread {
        id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
        name,
        rsp: UnsafeCell::new(0),
        entry,
        arg,
        kstack_top,
        process,
        sched: SpinLock::new(Params {
            class,
            state: State::Ready,
            cpu: 0,
            pinned: None,
            gang: None,
            vruntime: 0,
            slice_start: 0,
            last_start: 0,
            wake_at: 0,
            reservation: None,
            period_start: 0,
            budget: 0,
            demoted: false,
            released_at: 0,
            on_cpu: false,
            wake_pending: false,
        }),
        stats: Stats::default(),
    }))
}

/// Represents the code already running on a CPU (boot path) as a thread.
pub fn adopt_current(name: &'static str, class: Class) -> &'static Thread {
    let t = new_thread(name, class, None, 0);
    {
        let mut p = t.sched.lock();
        p.state = State::Running;
        p.on_cpu = true;
    }
    t
}

/// Creates the idle thread for a CPU.
pub fn new_idle(cpu: usize) -> Result<&'static Thread, &'static str> {
    let t = new_thread("idle", Class::Idle, Some(idle_loop), 0);
    t.sched.lock().cpu = cpu;
    let stack = paging::new_stack(STACK_PAGES)?;
    // SAFETY: `stack.top` is the aligned top of a fresh stack owned by `t`.
    unsafe { *t.rsp.get() = context::prepare_stack(stack.top, t as *const Thread as u64) };
    Ok(t)
}

fn idle_loop(_: u64) {
    loop {
        crate::arch::enable_interrupts();
        apic::wait_for_interrupt();
    }
}

#[derive(Default)]
pub struct SpawnOptions {
    pub reservation: Option<Reservation>,
    pub pinned: Option<usize>,
    pub gang: Option<u32>,
}

/// Starts a kernel thread running `entry(arg)`.
pub fn spawn(name: &'static str, class: Class, entry: fn(u64), arg: u64, opts: SpawnOptions) -> Result<&'static Thread, &'static str> {
    let t = new_thread(name, class, Some(entry), arg);
    let stack = paging::new_stack(STACK_PAGES)?;
    // SAFETY: `stack.top` is the aligned top of a fresh stack owned by `t`.
    unsafe { *t.rsp.get() = context::prepare_stack(stack.top, t as *const Thread as u64) };

    let target = opts.pinned.unwrap_or_else(cpu::least_loaded);
    if let Some(r) = opts.reservation {
        if class != Class::FrameDeadline {
            return Err("only frame-deadline threads take reservations");
        }
        // Admission takes the run-queue lock, so it happens before the thread lock.
        if !cpu::get(target).admit(r) {
            return Err("reservation refused: CPU frame-deadline capacity (70 %) exceeded");
        }
    }
    let now = apic::tsc();
    {
        let mut p = t.sched.lock();
        p.pinned = opts.pinned;
        p.gang = opts.gang;
        p.cpu = target;
        if let Some(r) = opts.reservation {
            p.reservation = Some(r);
            p.period_start = now;
            p.budget = r.runtime as i64;
            p.released_at = now;
        }
    }
    enqueue(t, target);
    Ok(t)
}

/// Puts a ready thread on `cpu`'s run queue and makes that CPU reschedule if needed.
fn enqueue(t: &'static Thread, target: usize) {
    let c = cpu::get(target);
    {
        let mut rq = c.rq.lock();
        {
            let mut p = t.sched.lock();
            p.state = State::Ready;
            p.cpu = target;
            // A thread joining a queue starts at the queue's minimum vruntime,
            // so it neither starves nor gets a backlog of credit.
            p.vruntime = p.vruntime.max(rq.min_vruntime);
        }
        rq.ready.push(t);
        rq.need_resched = true;
    }
    if target != this_cpu().index {
        apic::send_reschedule(c.apic_id);
    }
}

// ---------------------------------------------------------------- picking

/// Lower is better: (class rank, tiebreak).
fn rank(p: &Params) -> (u8, u64) {
    match p.class {
        Class::FrameDeadline if !p.demoted && p.budget > 0 => (0, p.period_start + p.reservation.map_or(0, |r| r.period)),
        Class::FrameDeadline | Class::Interactive => (1, p.vruntime),
        Class::Batch if p.gang.is_some() => (2, p.vruntime),
        Class::Batch => (3, p.vruntime),
        Class::Idle => (9, 0),
    }
}

fn replenish(p: &mut Params, now: u64) {
    if let Some(r) = p.reservation {
        if now >= p.period_start + r.period {
            let periods = (now - p.period_start) / r.period;
            p.period_start += periods * r.period;
            p.budget = r.runtime as i64;
            p.demoted = false;
            p.released_at = p.period_start;
        }
    }
}

/// Charges the running thread for the time since it last started.
fn account(t: &Thread, p: &mut Params, now: u64) {
    let delta = now.saturating_sub(p.last_start);
    p.last_start = now;
    if p.class == Class::Idle {
        t.stats.runtime[Class::Idle.index()].fetch_add(delta, Ordering::Relaxed);
        return;
    }
    let effective = if p.class == Class::FrameDeadline && (p.demoted || p.budget <= 0) { Class::Interactive } else { p.class };
    t.stats.runtime[effective.index()].fetch_add(delta, Ordering::Relaxed);
    p.vruntime += delta;
    if p.class == Class::FrameDeadline && !p.demoted {
        p.budget -= delta as i64;
        if p.budget <= 0 {
            p.demoted = true;
            t.stats.overruns.fetch_add(1, Ordering::Relaxed);
        }
    }
}

// ---------------------------------------------------------------- switching

/// Picks the next thread on this CPU and switches to it. `block` (if any)
/// marks the current thread as no longer runnable, atomically with the switch.
fn schedule_with(block: Option<(State, u64)>) {
    let irq = crate::arch::interrupts_enabled();
    crate::arch::disable_interrupts();
    let c = this_cpu();
    let prev = c.current();
    let next;
    {
        let mut rq = c.rq.lock();
        let now = apic::tsc();
        {
            let mut p = prev.sched.lock();
            account(prev, &mut p, now);
            match block {
                // A wake-up raced ahead of the block: do not block.
                Some((State::Blocked, _)) if p.wake_pending => {
                    p.wake_pending = false;
                    p.state = State::Ready;
                }
                Some((state, wake_at)) => {
                    p.state = state;
                    p.wake_at = wake_at;
                    if state == State::Sleeping {
                        rq.sleeping.push(prev);
                    }
                }
                None if p.state == State::Running && p.class != Class::Idle => p.state = State::Ready,
                None => {}
            }
        }
        rq.wake_due(now, Some(prev));
        rq.need_resched = false;

        // Best ready thread; the current one competes too if it is still ready.
        // Thread locks are taken one at a time, never nested.
        let (prev_ready, prev_rank) = {
            let p = prev.sched.lock();
            (p.state == State::Ready, rank(&p))
        };
        let best = rq.best();
        let take = match (best, prev_ready) {
            (Some((_, r)), true) => r < prev_rank,
            (Some(_), false) => true,
            (None, _) => false,
        };
        next = if take {
            let (i, _) = best.expect("checked above");
            rq.ready.swap_remove(i)
        } else if prev_ready {
            prev
        } else {
            c.idle()
        };

        if core::ptr::eq(next, prev) {
            let mut p = prev.sched.lock();
            p.state = State::Running;
            p.last_start = now;
            drop(p);
            drop(rq);
            if irq {
                crate::arch::enable_interrupts();
            }
            return;
        }

        let ready_min = rq.ready.iter().map(|t| t.sched.lock().vruntime).min();
        {
            let mut p = next.sched.lock();
            p.state = State::Running;
            p.on_cpu = true;
            p.cpu = c.index;
            p.last_start = now;
            p.slice_start = now;
            if p.released_at != 0 {
                let lat = now.saturating_sub(p.released_at);
                next.stats.max_wake_latency.fetch_max(lat, Ordering::Relaxed);
                p.released_at = 0;
            }
            let floor = ready_min.map_or(p.vruntime, |m| m.min(p.vruntime));
            rq.min_vruntime = rq.min_vruntime.max(floor);
        }
        next.stats.switches_in.fetch_add(1, Ordering::Relaxed);
        let _ = next.stats.first_run.compare_exchange(0, now, Ordering::Relaxed, Ordering::Relaxed);
        trace::switch(c.index, prev.id, next.id, now);
        c.set_current(next);
        c.set_prev(prev);
        // A user thread needs its process's page tables, and its own kernel
        // stack for interrupts and syscalls arriving from ring 3. Kernel
        // threads keep whatever page tables are loaded (the kernel half is the
        // same everywhere).
        if let Some(proc_) = &next.process {
            c.set_kernel_stack(next.kstack_top);
            let pml4 = proc_.pml4();
            if crate::arch::cpu::read_cr3() & !0xfff != pml4 {
                // SAFETY: a process page table maps the whole kernel half
                // (copied from the kernel's), including this code and stack.
                unsafe { crate::arch::cpu::write_cr3(pml4) };
            }
        }
    }
    // SAFETY: `next` is not in any run queue and not running anywhere (we
    // just removed it, or it is this CPU's idle thread), so its saved rsp is
    // stable; `prev.rsp` stays valid because threads are never freed.
    unsafe { context::switch(prev.rsp.get(), *next.rsp.get()) };
    finish_switch();
    if irq {
        crate::arch::enable_interrupts();
    }
}

/// Runs right after a switch, on the new thread: puts the previous thread
/// back on a run queue if it is still runnable. Doing this only now ensures
/// no other CPU can pick it before its context was saved.
fn finish_switch() {
    let c = this_cpu();
    if let Some(prev) = c.take_prev() {
        let (ready, target) = {
            let mut p = prev.sched.lock();
            p.on_cpu = false;
            (p.state == State::Ready && p.class != Class::Idle, p.pinned.unwrap_or(p.cpu))
        };
        if ready {
            let mut rq = cpu::get(target).rq.lock();
            rq.ready.push(prev);
        }
    }
}

/// First code of every new thread (called from the assembly trampoline).
pub extern "C" fn thread_start(t: &'static Thread) -> ! {
    finish_switch();
    crate::arch::enable_interrupts();
    if let Some(entry) = t.entry {
        entry(t.arg);
    }
    exit()
}

pub fn yield_now() {
    schedule_with(None);
}

/// Ends the current thread. TODO(awa): free its stack and control block.
pub fn exit() -> ! {
    // Give back admitted frame-deadline capacity before going away.
    let (res, cpu_index) = {
        let p = current().sched.lock();
        (p.reservation, p.cpu)
    };
    if let Some(r) = res {
        cpu::get(cpu_index).release(r);
    }
    schedule_with(Some((State::Dead, 0)));
    unreachable!("a dead thread was scheduled again")
}

pub fn sleep_until(tsc: u64) {
    if apic::tsc() < tsc {
        schedule_with(Some((State::Sleeping, tsc)));
    }
}

pub fn sleep_ns(ns: u64) {
    sleep_until(apic::tsc() + ns_to_tsc(ns));
}

pub fn current() -> &'static Thread {
    this_cpu().current()
}

/// Frame-deadline threads call this when a frame's work is done: records a
/// deadline miss if the job finished late, then sleeps until the next period.
pub fn wait_next_period() {
    let t = current();
    let (wake, missed) = {
        let p = t.sched.lock();
        let Some(r) = p.reservation else { return };
        let deadline = p.period_start + r.period;
        (deadline, apic::tsc() > deadline)
    };
    t.stats.jobs.fetch_add(1, Ordering::Relaxed);
    if missed {
        t.stats.misses.fetch_add(1, Ordering::Relaxed);
    }
    sleep_until(wake);
}

/// Timer interrupt on this CPU: wake sleepers, refill budgets, preempt if needed.
pub fn tick() {
    let c = this_cpu();
    let Some(cur) = c.try_current() else { return };
    let resched = {
        let mut rq = c.rq.lock();
        let now = apic::tsc();
        rq.wake_due(now, Some(cur));
        for t in rq.ready.iter() {
            replenish(&mut t.sched.lock(), now);
        }
        let (my_rank, expired) = {
            let mut p = cur.sched.lock();
            replenish(&mut p, now);
            if p.class != Class::Idle {
                account(cur, &mut p, now);
            }
            let slice = match p.class {
                Class::Interactive => ns_to_tsc(INTERACTIVE_SLICE_NS),
                Class::Batch => ns_to_tsc(BATCH_SLICE_NS),
                _ => u64::MAX,
            };
            (rank(&p), now.saturating_sub(p.slice_start) >= slice)
        };
        let better_waiting = rq.ready.iter().any(|t| rank(&t.sched.lock()).0 < my_rank.0);
        let same_waiting = rq.ready.iter().any(|t| rank(&t.sched.lock()).0 == my_rank.0);
        rq.need_resched || better_waiting || (expired && same_waiting) || (c.is_idle(cur) && !rq.ready.is_empty())
    };
    if resched {
        schedule_with(None);
    }
}

/// Starts scheduling on this CPU: the caller (already adopted as a thread) keeps running.
pub fn start_cpu(boot_thread: &'static Thread) {
    this_cpu().set_current(boot_thread);
    boot_thread.sched.lock().last_start = apic::tsc();
}

// ---------------------------------------------------------------- run queue

pub struct RunQueue {
    pub ready: alloc::vec::Vec<&'static Thread>,
    sleeping: alloc::vec::Vec<&'static Thread>,
    pub need_resched: bool,
    min_vruntime: u64,
    /// Sum of admitted frame-deadline utilisation, in percent.
    fd_load_percent: u64,
}

impl RunQueue {
    pub const fn new() -> Self {
        Self { ready: alloc::vec::Vec::new(), sleeping: alloc::vec::Vec::new(), need_resched: false, min_vruntime: 0, fd_load_percent: 0 }
    }

    /// Index and rank of the best ready thread.
    fn best(&self) -> Option<(usize, (u8, u64))> {
        let mut best: Option<(usize, (u8, u64))> = None;
        for (i, t) in self.ready.iter().enumerate() {
            let r = rank(&t.sched.lock());
            if best.is_none_or(|(_, b)| r < b) {
                best = Some((i, r));
            }
        }
        best
    }

    /// Moves sleepers whose time has come to the ready list. The thread that
    /// is still current on this CPU (if it is among them) only changes state:
    /// it must not enter the ready list while it is running.
    fn wake_due(&mut self, now: u64, current: Option<&Thread>) {
        let mut i = 0;
        while i < self.sleeping.len() {
            let t = self.sleeping[i];
            let mut p = t.sched.lock();
            if p.wake_at <= now {
                p.state = State::Ready;
                replenish(&mut p, now);
                p.vruntime = p.vruntime.max(self.min_vruntime);
                drop(p);
                self.sleeping.swap_remove(i);
                if !current.is_some_and(|c| core::ptr::eq(c, t)) {
                    self.ready.push(t);
                    self.need_resched = true;
                }
            } else {
                i += 1;
            }
        }
    }

    pub fn load(&self) -> usize {
        self.ready.len()
    }
}

pub fn admit_into(rq: &mut RunQueue, r: Reservation) -> bool {
    let pct = r.runtime * 100 / r.period.max(1);
    if rq.fd_load_percent + pct > FD_ADMISSION_PERCENT {
        return false;
    }
    rq.fd_load_percent += pct;
    true
}

pub fn release_from(rq: &mut RunQueue, r: Reservation) {
    let pct = r.runtime * 100 / r.period.max(1);
    rq.fd_load_percent = rq.fd_load_percent.saturating_sub(pct);
}

// ---------------------------------------------------------------- blocking

/// Threads waiting for something. Wakers call `wake_all` after changing the
/// condition the waiters check.
pub struct WaitQueue(SpinLock<Vec<&'static Thread>>);

impl WaitQueue {
    pub const fn new() -> Self {
        Self(SpinLock::new(Vec::new()))
    }

    pub fn wake_all(&self) {
        let waiters = core::mem::take(&mut *self.0.lock());
        for t in waiters {
            wake(t);
        }
    }
}

/// Makes a blocked thread runnable again. Safe against every race with the
/// thread going to sleep: if it has not blocked yet, the wake-up is remembered.
pub fn wake(t: &'static Thread) {
    let target = {
        let mut p = t.sched.lock();
        match p.state {
            State::Blocked => {
                p.state = State::Ready;
                if p.on_cpu {
                    // Still switching away on its CPU: `finish_switch` will
                    // see it is ready and queue it.
                    return;
                }
                p.pinned.unwrap_or(p.cpu)
            }
            State::Dead => return,
            _ => {
                p.wake_pending = true;
                return;
            }
        }
    };
    enqueue(t, target);
}

/// Blocks the current thread until `ready()` is true.
pub fn wait_event(queue: &WaitQueue, ready: impl Fn() -> bool) {
    loop {
        if ready() {
            return;
        }
        let me = current();
        queue.0.lock().push(me);
        if ready() {
            queue.0.lock().retain(|t| !core::ptr::eq(*t, me));
            return;
        }
        schedule_with(Some((State::Blocked, 0)));
        // Clear any wake-up that arrived after we had already decided not to block.
        me.sched.lock().wake_pending = false;
    }
}

// ---------------------------------------------------------------- user threads

const USER_KSTACK_PAGES: u64 = 16; // 64 KiB kernel stack per user thread

struct UserStart {
    rip: u64,
    rsp: u64,
    args: [u64; 3],
}

fn user_trampoline(arg: u64) {
    // SAFETY: `arg` is the Box<UserStart> leaked by `spawn_user` for this thread.
    let start = unsafe { Box::from_raw(arg as *mut UserStart) };
    let (rip, rsp, a) = (start.rip, start.rsp, start.args);
    drop(start);
    // SAFETY: we were switched to by `schedule_with`, which loaded this
    // process's page tables and pointed rsp0/scratch at this thread's kernel
    // stack; `spawn_user` was given a mapped entry and stack.
    unsafe { crate::arch::syscall::enter_user(rip, rsp, a[0], a[1], a[2]) }
}

/// Starts the main thread of `process` at user address `rip` with stack `rsp`.
pub fn spawn_user(process: Arc<crate::process::Process>, name: &'static str, rip: u64, rsp: u64, args: [u64; 3]) -> Result<&'static Thread, &'static str> {
    let stack = paging::new_stack(USER_KSTACK_PAGES)?;
    let start = Box::into_raw(Box::new(UserStart { rip, rsp, args })) as u64;
    let t = new_thread_full(name, Class::Interactive, Some(user_trampoline), start, stack.top, Some(process));
    // SAFETY: `stack.top` is the aligned top of a fresh stack owned by `t`.
    unsafe { *t.rsp.get() = context::prepare_stack(stack.top, t as *const Thread as u64) };
    let target = cpu::least_loaded();
    t.sched.lock().cpu = target;
    enqueue(t, target);
    Ok(t)
}
