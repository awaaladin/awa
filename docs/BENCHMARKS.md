# Benchmarks and success metrics

Status: plan only. No benchmark exists yet, so **no performance claim can be made.**

## 1. Rules

- Every claim of the form "AWA is faster or lighter than X" needs a result file in
  `bench/results/` produced by the suite, on named hardware, with the exact versions of
  both systems.
- Comparisons with Linux use **the same machine, the same workload binary or source
  revision, the same input data** and a documented Linux configuration (distribution,
  kernel version, governor, swap and zram settings). AWA results are run several times
  and reported as median and p99.
- QEMU numbers are never used for performance claims, only for regression tracking.

## 2. Success metrics from the brief

| Metric | Method | Earliest possible |
|--------|--------|-------------------|
| **Peak RAM for the same render / training job vs Linux** | Run the same job (initially a C++ path tracer we control; later Blender/PyTorch) on both OSes, sampling resident memory; compare peaks and time to completion | After POSIX phase (C2–C3); real apps after C6/C7 |
| **Large-model load time** | Time from open to first inference-ready state for a 4 GiB and a 13 GiB weight file, cold and warm cache, plain `read` vs Streamable mapping | Storage phase (synthetic loader); PyTorch after C6 |
| **Viewport frame-time consistency while a background render runs** | A frame-deadline renderer (60 Hz target) + a batch path tracer saturating all CPUs; record frame-time histogram, p99, missed deadlines | Graphics phase (CPU-rendered viewport first) |
| **Graceful degradation when available RAM is halved** | Run the job at full RAM, then with the box limit halved; report slowdown, discards, compression ratio, refaults; must complete without OOM kill | Phase 5 (synthetic workload with recomputable cache) |

## 3. Microbenchmarks (kernel health)

IPC round trip, context switch, page-fault latency (anonymous, streamable, discarded),
compression throughput and ratio, syscall entry cost with and without KPTI, scheduler
wakeup latency per class, and tracing overhead.

## 4. Suite structure (to be built in `bench/`)

- Workloads described in a manifest (name, binary, inputs, metrics, repeat count).
- A runner for AWA (results over serial or to the data partition) and one for Linux.
- Results as JSON plus a comparison report generator.
