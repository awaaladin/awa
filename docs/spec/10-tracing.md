# 10 · Always-on tracing and the profiler

Status: draft · Not implemented

## 1. Two layers

1. **Counters (always on, essentially free):** per-thread, per-process, per-account and
   per-box totals, updated at events the kernel already handles: context switch, page
   fault, reclaim, IPC send and receive.
2. **Event trace (always on, bounded):** per-CPU lock-free ring buffers of fixed-size
   binary events (32 bytes each: TSC timestamp, event id, thread, 2–3 args). Default
   size is 256 KiB per CPU, overwritten in a loop; the last few seconds are always
   available. Session-based deeper tracing (larger buffers, more event categories) can
   be turned on with a `TraceSession` capability.

## 2. What is attributed

| Resource | Source | Granularity |
|----------|--------|-------------|
| CPU time | scheduler, by class, including donated IPC time | thread → process → box |
| RAM | memory accounting ([02 §4](02-memory.md)) | process / box, by class |
| Faults, refaults, discards, compression | memory manager | process |
| Disk I/O bytes and latency | storage services report per request with the requesting account | process |
| GPU time and memory | GPU service reports per context | process / box |
| Cache misses | PMU: LLC-miss and instruction counters read at context switch (counting mode, not sampling) | thread |
| IPC | message count, bytes, call latency | channel / process |

PMU counters are **not available under QEMU TCG**. They appear only on real hardware or
KVM. The profiler shows "not available" rather than zeros.

## 3. Overhead budget

Target: **≤ 1 % CPU** overhead for always-on counters and the event ring on the
`bench/` CPU workloads, measured against a build with tracing compiled out. If the
target is missed, event categories are moved from always-on to session-only. The
number is published in the benchmark report. Unmeasured until Phase 3.

## 4. Exposure

- The **trace service** holds the `TraceSession` root capability and serves summaries
  over IPC. `ram show`, `gpu show` and the Settings app read from it.
- The **profiler** (`userland/profile`) shows per-app timelines (CPU by class, RAM by
  class, GPU, disk), frame-time histograms for frame-deadline threads, and flame
  graphs from on-demand stack sampling (timer-based, session-only).
- Export format: a documented binary format plus conversion to Chrome/Perfetto JSON for
  use with existing viewers.

## 5. Security

Trace data reveals timing information about other processes. Access requires the
`TraceSession` capability, which apps and boxes do not get by default. A box can be
given a session limited to its own account.
