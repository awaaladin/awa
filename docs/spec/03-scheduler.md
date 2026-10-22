# 03 · Scheduler

Status: draft · Not implemented

## 1. Classes

Classes are strictly ordered. A runnable thread in a higher class always preempts a lower
class on the same CPU, subject to the bandwidth limits below.

| Class | For | Policy |
|-------|-----|--------|
| **Frame-deadline** | Viewport, playback, compositor, audio, input | Earliest-deadline-first within admitted reservations (runtime *R* per period *P*), each enforced by a constant-bandwidth server |
| **Interactive** | Shell, editors, UI threads not holding a reservation, most app threads | Fair share with latency bias (EEVDF-style virtual deadlines), short slices (1–4 ms) |
| **Batch** | Renders, training, simulation, compiles | Fair share, long slices (20–100 ms) for cache warmth, gang scheduling, yields to everything above |
| **Idle** | Background maintenance (scrubbing, page zeroing, compaction) | Runs only when a CPU has nothing else |

A thread's default class is Interactive. Unmodified programs run there.

## 2. Frame-deadline reservations

- A thread obtains a reservation by presenting a `SchedReservation` capability. It
  requests e.g. *R* = 4 ms every *P* = 16.67 ms. The kernel **admits** it only if the
  CPU's total frame-deadline utilization stays at or below the admission cap (default
  70 %, configurable). This leaves room for the other classes.
- A thread that overruns its *R* in a period is demoted to Interactive until the next
  period. Overruns are counted in trace data. This prevents runaway frame-deadline
  threads from starving the system.
- The window server is granted the right to give reservations to the **focused app's**
  render and UI threads. Unmodified apps therefore get frame-deadline treatment through
  their window without code changes. libawa apps can request reservations explicitly:
  `awa::sched::reserve(period, runtime)`.
- Deadlines can be aligned to display vblank: the window server publishes vblank times
  through a shared page.

## 3. Interactive class

- EEVDF-style fair scheduling: virtual deadlines, with latency-sensitive (short-slice)
  threads favored.
- Waking from user input gives a one-off latency boost, bounded and charged against
  fair share.

## 4. Batch class

- **Long slices** reduce context-switch and cache-pollution costs.
- **Gang scheduling.** Threads in a `Gang` object (for example a renderer's worker pool)
  are co-scheduled: when the gang gets a time window, all its threads start together on
  separate CPUs. This avoids lock-holder or barrier stalls where one worker waits for a
  descheduled peer. If not enough CPUs are free, the gang runs with fewer threads only
  if it set `allow_partial`; otherwise it waits for the next window.
- **Yield to higher classes.** Batch threads on a CPU are preempted within one timer
  tick when a frame-deadline or interactive thread becomes runnable there.
  Frame-deadline wakeups send an IPI immediately.
- **SMT isolation (optional, on by default when there are at least 4 cores).** A batch
  thread is not placed on the hyperthread sibling of a CPU running a frame-deadline
  thread, because the sibling competes for the same execution units. On 2-core/4-thread
  hardware this is **off by default**, since it would halve batch throughput. It can be
  toggled at runtime.

## 5. Placement

- Topology comes from CPUID leaf 0xB/0x4 (core, SMT and cache sharing) and ACPI tables.
- A thread's wake-up CPU is chosen to prefer its last CPU (cache-warm), then a CPU
  sharing its L2, then one sharing its L3.
- Batch gangs are placed so that the gang's threads share an L3 where possible.
  Frame-deadline threads are spread so they don't contend with each other.
- Load balancing runs periodically per CPU. Pulls are rate-limited to avoid ping-pong.

## 6. Priority inversion

- Synchronous IPC calls **donate** the caller's scheduling class and deadline to the
  server thread for the duration of the call. A frame-deadline viewport calling the
  GPU service is therefore served at frame-deadline priority.
- Kernel mutexes (where blocking is used) use priority inheritance.

## 7. Timers and ticks

- APIC timer in TSC-deadline mode (supported by Haswell) for one-shot programming.
  The kernel is **tickless** when a CPU runs one thread or is idle.
- Time base: invariant TSC, calibrated against HPET or the ACPI PM timer at boot.
- Idle: `mwait` with C-state hints where supported, otherwise `hlt`.

## 8. Accounting

- CPU time per thread is attributed to its process, account and box, split by class.
  Donated time is charged to the caller. These numbers feed the profiler ([10](10-tracing.md)).

## 9. Open items

- TODO(awa): gang windows vs. strict co-scheduling; evaluate in Phase 3 with a synthetic
  barrier-heavy workload.
- TODO(awa): energy-aware placement (later; not a goal for desktops on power).
