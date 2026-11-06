//! Boot self-tests. Each prints `AWA TEST <name> PASS|FAIL <detail>` on the
//! serial port; `tools/awa.py test` fails the boot test if any line says FAIL
//! or the summary is missing. They run on every boot (they are fast), so real
//! hardware is checked too.

use crate::arch::{self, apic, probe};
use crate::mem::{self, frames, paging};
use crate::serial_println;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::sync::atomic::Ordering;

const PF_PRESENT: u64 = 1 << 0;
const PF_WRITE: u64 = 1 << 1;
const PF_FETCH: u64 = 1 << 4;

/// Memory protection and allocators: W^X, NX, guard pages, frames, heap.
pub fn memory(r: &mut Report) {
    // W^X: kernel code must not be writable, even by the kernel.
    let text = paging::kernel_text_start();
    let got = probe::write_same(text);
    let ok = matches!(got, probe::Outcome::Faulted { code, .. } if code & (PF_PRESENT | PF_WRITE) == PF_PRESENT | PF_WRITE);
    r.check("wx-text-not-writable", ok, format_args!("write to kernel code {:#x} -> {:?}", text, got));

    let rodata = paging::kernel_rodata_start();
    let got = probe::write_same(rodata);
    let ok = matches!(got, probe::Outcome::Faulted { code, .. } if code & PF_WRITE != 0);
    r.check("rodata-not-writable", ok, format_args!("write to read-only data {:#x} -> {:?}", rodata, got));

    // NX: data on the heap must never run as code.
    let ret = Box::new([0xC3u8; 64]); // 0xC3 = `ret`, so a wrongly allowed call just returns
    let addr = ret.as_ptr() as u64;
    // SAFETY: the target holds `ret` bytes, as `probe::exec` requires.
    let got = unsafe { probe::exec(addr) };
    let ok = matches!(got, probe::Outcome::Faulted { code, .. } if code & PF_FETCH != 0);
    r.check("nx-heap-not-executable", ok, format_args!("call into heap {:#x} -> {:?}", addr, got));
    drop(ret);

    // NX on the direct map (all of RAM).
    match frames::alloc() {
        Some(f) => {
            let v = mem::phys_to_virt(f);
            // SAFETY: `f` is a frame we own, mapped read-write in the direct map.
            unsafe { core::ptr::write_bytes(v as *mut u8, 0xC3, 16) };
            // SAFETY: the target holds `ret` bytes.
            let got = unsafe { probe::exec(v) };
            frames::free(f);
            let ok = matches!(got, probe::Outcome::Faulted { code, .. } if code & PF_FETCH != 0);
            r.check("nx-directmap-not-executable", ok, format_args!("call into RAM {:#x} -> {:?}", v, got));
        }
        None => r.check("nx-directmap-not-executable", false, format_args!("no frame to test with")),
    }

    // Guard page: the page right below the kernel stack is never mapped.
    let guard = crate::KERNEL_STACK_BOTTOM.load(Ordering::Relaxed) - 8;
    let got = probe::read(guard);
    let ok = matches!(got, probe::Outcome::Faulted { code, .. } if code & PF_PRESENT == 0);
    r.check("stack-guard-page", ok, format_args!("read below the stack {:#x} -> {:?}", guard, got));

    // Frame allocator: many frames, all distinct and aligned, all returned.
    let before = frames::stats();
    let mut taken: Vec<u64> = Vec::with_capacity(4096);
    while taken.len() < 4096 {
        match frames::alloc() {
            Some(f) => taken.push(f),
            None => break,
        }
    }
    let count = taken.len();
    let aligned = taken.iter().all(|f| f % mem::PAGE_SIZE == 0 && *f >= 1 << 20);
    taken.sort_unstable();
    taken.dedup();
    let distinct = taken.len() == count;
    for f in taken {
        frames::free(f);
    }
    let after = frames::stats();
    r.check(
        "frames-stress",
        count == 4096 && aligned && distinct && after.free == before.free,
        format_args!("{} frames: aligned {}, distinct {}, free {} -> {}", count, aligned, distinct, before.free, after.free),
    );

    // Heap: mixed sizes, patterns verified, everything returned.
    let start = mem::heap::usage().used;
    let mut blocks: Vec<Vec<u8>> = Vec::new();
    for i in 0..300usize {
        let len = 16 + (i * 97) % 4000;
        blocks.push(alloc::vec![(i % 251) as u8; len]);
    }
    let intact = blocks.iter().enumerate().all(|(i, b)| b.iter().all(|&x| x == (i % 251) as u8));
    blocks.retain(|b| b.len() % 2 == 0); // free about half, fragmenting the heap
    let big: Vec<u64> = alloc::vec![0xA5A5_A5A5_A5A5_A5A5; 128 * 1024]; // 1 MiB
    let big_ok = big.iter().all(|&x| x == 0xA5A5_A5A5_A5A5_A5A5);
    drop(big);
    drop(blocks);
    let end = mem::heap::usage().used;
    r.check("heap-stress", intact && big_ok && end == start, format_args!("300 mixed blocks + 1 MiB; used bytes {} -> {}", start, end));
}

pub struct Report {
    passed: u32,
    failed: u32,
    /// Short result lines, also drawn on screen (real laptops have no serial port).
    lines: Vec<alloc::string::String>,
}

pub struct Summary {
    pub passed: u32,
    pub failed: u32,
    pub lines: Vec<alloc::string::String>,
}

impl Report {
    pub fn new() -> Self {
        Self { passed: 0, failed: 0, lines: Vec::new() }
    }

    pub fn check(&mut self, name: &str, ok: bool, detail: core::fmt::Arguments) {
        if ok {
            self.passed += 1;
        } else {
            self.failed += 1;
        }
        serial_println!("AWA TEST {} {} {}", name, if ok { "PASS" } else { "FAIL" }, detail);
        let mut line = alloc::format!("{} {} {}", if ok { "PASS" } else { "FAIL" }, name, detail);
        line.truncate(150);
        self.lines.push(line);
    }

    /// Prints an informational line (serial and screen) that is not a test.
    pub fn note(&mut self, text: core::fmt::Arguments) {
        serial_println!("AWA: {}", text);
        let mut line = alloc::format!("     {}", text);
        line.truncate(150);
        self.lines.push(line);
    }

    pub fn finish(self) -> Summary {
        serial_println!("AWA: self-test {} passed, {} failed", self.passed, self.failed);
        Summary { passed: self.passed, failed: self.failed, lines: self.lines }
    }
}

/// CPU tables: exceptions reach their handlers and the kernel survives them.
pub fn cpu(r: &mut Report) {
    let before = arch::idt::BREAKPOINTS.load(Ordering::Relaxed);
    arch::breakpoint();
    let after = arch::idt::BREAKPOINTS.load(Ordering::Relaxed);
    r.check("idt-breakpoint", after == before + 1, format_args!("int3 handled and returned"));

    // An address nothing maps: the page-fault handler must catch the read.
    const UNMAPPED: u64 = 0xffff_fe00_0000_0000;
    let got = probe::read(UNMAPPED);
    let ok = matches!(got, probe::Outcome::Faulted { addr, code } if addr == UNMAPPED && code & 1 == 0);
    r.check("page-fault-caught", ok, format_args!("read of {:#x} -> {:?}", UNMAPPED, got));
}

/// Timer: interrupts arrive at the configured rate, measured against the TSC.
/// Real hardware must be within 3 %. Under a hypervisor without hardware
/// acceleration (QEMU TCG) emulated time drifts, so 30 % is allowed there
/// and the result says so.
pub fn timer(r: &mut Report) {
    let virtualized = crate::arch::cpu::running_under_hypervisor();
    let tsc_hz = apic::TSC_HZ.load(Ordering::Relaxed);
    let start = apic::TICKS.load(Ordering::Relaxed);
    let deadline = apic::tsc() + tsc_hz; // one second
    while apic::tsc() < deadline {
        apic::wait_for_interrupt();
    }
    let ticks = apic::TICKS.load(Ordering::Relaxed) - start;
    let gap_us = apic::MAX_TICK_GAP.load(Ordering::Relaxed) as u128 * 1_000_000 / tsc_hz.max(1) as u128;
    let expected = apic::TICK_HZ;
    if virtualized {
        // Under a hypervisor the host decides when virtual timer interrupts
        // arrive (QEMU on Windows: host timer granularity ~15.6 ms), so the
        // rate is reported, and only "the timer runs" is judged. (D-036)
        r.note(format_args!("timer under a hypervisor: {} of {} ticks in 1 s, longest gap {} us (host-limited, not judged)", ticks, expected, gap_us));
        r.check("timer-runs", ticks > 0, format_args!("{} timer interrupts in 1 s", ticks));
    } else {
        let ok = ticks * 100 >= expected * 97 && ticks * 100 <= expected * 103;
        r.check("timer-rate", ok, format_args!("{} ticks in 1 s (expected {} +/-3%), longest gap {} us", ticks, expected, gap_us));
    }
}

/// Random numbers: the ChaCha20 core matches RFC 8439, and output looks random.
pub fn random(r: &mut Report) {
    let key: [u32; 8] = core::array::from_fn(|i| u32::from_le_bytes([(4 * i) as u8, (4 * i + 1) as u8, (4 * i + 2) as u8, (4 * i + 3) as u8]));
    let block = crate::rng::chacha20_block(&key, 1, &[0x0900_0000, 0x4a00_0000, 0]);
    let want = [0xe4e7_f110, 0x1559_3bd1, 0x1fdd_0f50, 0xc471_20a3];
    r.check("chacha20-rfc8439", block[..4] == want, format_args!("block function test vector (RFC 8439 2.3.2)"));

    let mut ones = 0u32;
    let mut repeats = 0;
    let mut last = 0u64;
    for _ in 0..1000 {
        let v = crate::rng::next_u64();
        ones += v.count_ones();
        if v == last {
            repeats += 1;
        }
        last = v;
    }
    let mean = ones as f64 / 1000.0;
    r.check("rng-output", (31.0..=33.0).contains(&mean) && repeats == 0, format_args!("1000 values: mean {:.2} set bits of 64, {} repeats", mean, repeats));
}

// ---------------------------------------------------------------- scheduler (Phase 3)

use crate::sched::{self, Class, Reservation, SpawnOptions};
use core::sync::atomic::{AtomicBool, AtomicU64 as Counter};

static DONE: Counter = Counter::new(0);
static STOP: AtomicBool = AtomicBool::new(false);
static PROGRESS: [Counter; 2] = [Counter::new(0), Counter::new(0)];
static CPU_SEEN: Counter = Counter::new(0);

/// Busy work for `ns` nanoseconds with interrupts on, so the thread can be preempted.
fn spin_ns(ns: u64) {
    let end = apic::tsc() + sched::ns_to_tsc(ns);
    while apic::tsc() < end {
        core::hint::spin_loop();
    }
}

/// Sleeps in 1 ms steps until `cond` holds or `timeout_ns` passes.
fn wait_until(timeout_ns: u64, cond: impl Fn() -> bool) -> bool {
    let end = apic::tsc() + sched::ns_to_tsc(timeout_ns);
    while !cond() {
        if apic::tsc() > end {
            return false;
        }
        sched::sleep_ns(1_000_000);
    }
    true
}

fn worker(_: u64) {
    spin_ns(2_000_000);
    CPU_SEEN.fetch_or(1 << sched::this_cpu().index, Ordering::Relaxed);
    DONE.fetch_add(1, Ordering::Release);
}

fn counter_loop(slot: u64) {
    while !STOP.load(Ordering::Relaxed) {
        PROGRESS[slot as usize].fetch_add(1, Ordering::Relaxed);
        spin_ns(50_000);
    }
}

fn batch_hog(_: u64) {
    while !STOP.load(Ordering::Relaxed) {
        spin_ns(100_000);
    }
}

const FD_PERIOD_NS: u64 = 16_666_667; // 60 Hz, a viewport frame
const FD_RUNTIME_NS: u64 = 4_000_000;
const FD_WORK_NS: u64 = 1_000_000;
const FD_FRAMES: u64 = 60;

fn fd_viewport(_: u64) {
    for _ in 0..FD_FRAMES {
        spin_ns(FD_WORK_NS);
        sched::wait_next_period();
    }
    DONE.fetch_add(1, Ordering::Release);
}

fn fd_overrunner(_: u64) {
    for _ in 0..20 {
        spin_ns(4_000_000); // asks for 4 ms each period but only reserved 1 ms
        sched::wait_next_period();
    }
    DONE.fetch_add(1, Ordering::Release);
}

/// Phase 3 exit criteria: CPUs online, threads run and preempt, frame-deadline
/// threads keep their deadlines under full batch load, overruns are demoted,
/// admission control holds, gangs start together, tracing is cheap.
pub fn scheduler(r: &mut Report, expected_cpus: usize) {
    let online = crate::ONLINE.load(Ordering::Acquire) as usize;
    r.check("cpus-online", online == expected_cpus, format_args!("{} of {} CPUs running the scheduler", online, expected_cpus));
    let ncpu = sched::cpu::count().max(1);
    let virtualized = arch::cpu::running_under_hypervisor();

    // Threads run, on more than one CPU when there is more than one.
    DONE.store(0, Ordering::Relaxed);
    CPU_SEEN.store(0, Ordering::Relaxed);
    let mut spawned = 0;
    for _ in 0..8 {
        if sched::spawn("worker", Class::Interactive, worker, 0, SpawnOptions::default()).is_ok() {
            spawned += 1;
        }
    }
    let finished = wait_until(3_000_000_000, || DONE.load(Ordering::Acquire) == spawned);
    let cpus_used = CPU_SEEN.load(Ordering::Relaxed).count_ones() as usize;
    r.check(
        "threads-run",
        spawned == 8 && finished && (ncpu == 1 || cpus_used > 1),
        format_args!("{} of 8 threads finished, on {} CPU(s)", DONE.load(Ordering::Relaxed), cpus_used),
    );

    // Preemption: two CPU-bound threads pinned to one CPU both make progress.
    let target = ncpu - 1;
    STOP.store(false, Ordering::Relaxed);
    for slot in 0..2u64 {
        let _ = sched::spawn("counter", Class::Interactive, counter_loop, slot, SpawnOptions { pinned: Some(target), ..Default::default() });
    }
    sched::sleep_ns(300_000_000);
    let (a, b) = (PROGRESS[0].load(Ordering::Relaxed), PROGRESS[1].load(Ordering::Relaxed));
    STOP.store(true, Ordering::Relaxed);
    r.check("preemption", a > 0 && b > 0, format_args!("two spinning threads on CPU {}: {} and {} steps in 0.3 s", target, a, b));
    sched::sleep_ns(20_000_000);

    // Admission control.
    let c = sched::cpu::get(target);
    let half = Reservation { runtime: sched::ns_to_tsc(5_000_000), period: sched::ns_to_tsc(10_000_000) };
    let third = Reservation { runtime: sched::ns_to_tsc(3_000_000), period: sched::ns_to_tsc(10_000_000) };
    let first = c.admit(half);
    let second = c.admit(third);
    if first {
        c.release(half);
    }
    if second {
        c.release(third);
    }
    r.check("fd-admission", first && !second, format_args!("50 % admitted: {}, then +30 % refused: {} (cap 70 %)", first, !second));

    // Frame-deadline under load: a 60 Hz viewport thread vs batch hogs on every CPU.
    STOP.store(false, Ordering::Relaxed);
    for _ in 0..ncpu * 2 {
        let _ = sched::spawn("batch-hog", Class::Batch, batch_hog, 0, SpawnOptions::default());
    }
    DONE.store(0, Ordering::Relaxed);
    let res = Reservation { runtime: sched::ns_to_tsc(FD_RUNTIME_NS), period: sched::ns_to_tsc(FD_PERIOD_NS) };
    match sched::spawn("viewport", Class::FrameDeadline, fd_viewport, 0, SpawnOptions { reservation: Some(res), pinned: Some(target), ..Default::default() }) {
        Ok(t) => {
            let ok = wait_until(5_000_000_000, || DONE.load(Ordering::Acquire) == 1);
            let misses = t.stats.misses.load(Ordering::Relaxed);
            let jobs = t.stats.jobs.load(Ordering::Relaxed);
            let lat_us = sched::tsc_to_ns(t.stats.max_wake_latency.load(Ordering::Relaxed)) / 1000;
            // Real hardware: zero misses. Under a hypervisor timer delivery is
            // host-limited (see timer test), so misses are reported, not judged. (D-036)
            let allowed = if virtualized { FD_FRAMES } else { 0 };
            r.check(
                "fd-deadlines-under-load",
                ok && jobs == FD_FRAMES && misses <= allowed,
                format_args!(
                    "{} frames at 60 Hz with {} batch hogs: {} missed (allowed {}), worst wake-up latency {} us{}",
                    jobs,
                    ncpu * 2,
                    misses,
                    allowed,
                    lat_us,
                    if virtualized { " (hypervisor)" } else { "" }
                ),
            );
        }
        Err(e) => r.check("fd-deadlines-under-load", false, format_args!("spawn failed: {}", e)),
    }

    // Overrun: a frame-deadline thread that exceeds its budget is demoted, and
    // an interactive thread on the same CPU keeps running.
    DONE.store(0, Ordering::Relaxed);
    PROGRESS[0].store(0, Ordering::Relaxed);
    let _ = sched::spawn("counter", Class::Interactive, counter_loop, 0, SpawnOptions { pinned: Some(target), ..Default::default() });
    let res = Reservation { runtime: sched::ns_to_tsc(1_000_000), period: sched::ns_to_tsc(10_000_000) };
    match sched::spawn("overrunner", Class::FrameDeadline, fd_overrunner, 0, SpawnOptions { reservation: Some(res), pinned: Some(target), ..Default::default() }) {
        Ok(t) => {
            let ok = wait_until(5_000_000_000, || DONE.load(Ordering::Acquire) == 1);
            let overruns = t.stats.overruns.load(Ordering::Relaxed);
            let others = PROGRESS[0].load(Ordering::Relaxed);
            r.check("fd-overrun-demoted", ok && overruns > 0 && others > 0, format_args!("{} overruns demoted; interactive neighbour made {} steps", overruns, others));
        }
        Err(e) => r.check("fd-overrun-demoted", false, format_args!("spawn failed: {}", e)),
    }
    STOP.store(true, Ordering::Relaxed);
    sched::sleep_ns(50_000_000);

    // Gang: members of a gang start close together. (Gang members are
    // preferred over other batch work; strict co-scheduled windows are TODO.)
    STOP.store(false, Ordering::Relaxed);
    let mut members = alloc::vec::Vec::new();
    for i in 0..ncpu {
        if let Ok(t) = sched::spawn("gang", Class::Batch, batch_hog, 0, SpawnOptions { gang: Some(1), pinned: Some(i), ..Default::default() }) {
            members.push(t);
        }
    }
    let all_started = wait_until(2_000_000_000, || members.iter().all(|t| t.stats.first_run.load(Ordering::Relaxed) != 0));
    STOP.store(true, Ordering::Relaxed);
    let firsts: alloc::vec::Vec<u64> = members.iter().map(|t| t.stats.first_run.load(Ordering::Relaxed)).collect();
    let skew_us = match (firsts.iter().min(), firsts.iter().max()) {
        (Some(a), Some(b)) => sched::tsc_to_ns(b - a) / 1000,
        _ => u64::MAX,
    };
    let bound_us = if virtualized { 50_000 } else { 5_000 };
    r.check("gang-start-skew", all_started && skew_us <= bound_us, format_args!("{} gang threads started within {} us (bound {} us)", members.len(), skew_us, bound_us));
    sched::sleep_ns(50_000_000);

    // Tracing cost: emitting one event.
    // Measure the path the scheduler uses: CPU already known, interrupts off.
    let n = 20_000u64;
    arch::disable_interrupts();
    let me = sched::this_cpu();
    let t0 = apic::tsc();
    for i in 0..n {
        crate::trace::emit_on(me, crate::trace::KIND_MARK, i, 0, t0);
    }
    let per_event_ns = sched::tsc_to_ns(apic::tsc() - t0) / n;
    arch::enable_interrupts();
    let events: u64 = (0..sched::cpu::count()).map(|i| sched::cpu::get(i).trace.written()).sum::<u64>();
    r.check("trace-cost", per_event_ns < 1_000, format_args!("{} ns per event; {} events recorded so far across CPUs", per_event_ns, events));
}

/// Kernel page-table isolation: while a program runs, its page tables must
/// not map the kernel, except the entry code.
pub fn isolation(r: &mut Report, p: &crate::process::Process, entry_start: u64) {
    if !paging::isolation() {
        r.check("kpti-kernel-hidden", true, format_args!("not needed: this CPU reports it is not affected by Meltdown"));
        return;
    }
    let user = p.user_pml4();
    let visible = |va: u64| paging::translate_in(user, va).is_some();
    let hidden = [paging::kernel_rodata_start(), paging::HEAP_BASE, mem::phys_to_virt(0x10_0000), paging::kernel_text_start() + 0x10_000];
    let leaked: alloc::vec::Vec<u64> = hidden.iter().copied().filter(|&va| visible(va)).collect();
    let entry_ok = visible(entry_start) && visible(p.vdso_base);
    r.check(
        "kpti-kernel-hidden",
        leaked.is_empty() && entry_ok,
        format_args!("in init's user page tables: kernel data, heap, RAM and code {}; entry code and vDSO {}", if leaked.is_empty() { "unmapped" } else { "STILL MAPPED" }, if entry_ok { "mapped" } else { "MISSING" }),
    );
}
