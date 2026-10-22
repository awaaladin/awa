# 06 · GPU

Status: draft · Not implemented

## 1. Where the GPU driver lives (proposal A3)

The brief allowed a kernel exception for the GPU. **This spec proposes not using it.**

- The GPU driver is a **user-space service**. One process per GPU owns its MMIO, its
  interrupts and its DMA, through the IOMMU where available.
- The kernel provides only what must be privileged: IOMMU domains, pinned DMA regions,
  interrupt delivery, and **accounting hooks**, so GPU memory and GPU time land in the
  same account tree as CPU memory.
- Why: GPU drivers are among the largest and most crash-prone code in any OS. In user
  space, a GPU hang or driver bug restarts the service (the window server re-attaches)
  instead of taking down the machine. The cost is one IPC per **submission batch**, not
  per draw call. Clients build command buffers in shared memory and submit handles to
  them, so this cost is expected to be small. **This is unmeasured** and will be checked
  with a benchmark at the GPU phase. The decision is revisited only with numbers.

## 2. Unified CPU/GPU memory accounting

- GPU buffers are VMOs (system RAM) or **VRAM allocations** (discrete GPUs). Both are
  charged to the creating process's account, under separate domains: `ram`, `vram`.
- On integrated GPUs (your HD 4400, and future iGPUs/APUs), GPU memory *is* system RAM.
  Memory classes apply directly: a recomputable texture cache can be discarded like any
  other recomputable VMO, as long as it is not in use by a pending GPU job. The GPU
  service holds a lock on the chunk while a job references it.
- On discrete GPUs, VRAM is a separate pool with its own per-account quotas. Under VRAM
  pressure, the GPU service evicts recomputable VRAM first, then migrates Required
  buffers to system RAM, following the same ladder as [02 §6](02-memory.md).
- `gpu show` reports per app and box: VRAM and GTT/system bytes by class, GPU busy time
  per engine, and preemption/timeout counts.

## 3. GPU scheduling and preemption

- Every GPU context carries a class mirroring CPU classes: **frame-deadline**
  (compositor, focused viewport), **interactive** and **batch** (renders, training).
- The GPU service schedules submissions per engine: frame-deadline contexts first,
  batch contexts fill the remaining time.
- **Preemption depends on hardware, and the honest answer differs by GPU:**

  | Hardware | Preemption available | What AWA can guarantee |
  |----------|----------------------|------------------------|
  | virtio-gpu (QEMU) | none (host decides) | functional testing only |
  | Intel Gen7.5 (HD 4400) | no usable mid-batch preemption | scheduling **between batches** only. libawa's compute API splits long work into bounded batches; an unmodified app submitting one huge batch **can** still delay the display |
  | Intel Gen9+ and AMD GCN/RDNA | mid-command-buffer / compute-wave preemption | true preemption of batch work by frame-deadline work |
  | NVIDIA (via GSP firmware) | firmware time-slicing | depends on firmware interfaces |

- A **watchdog** resets any GPU context that holds an engine past its limit (default 2 s
  for interactive, configurable for batch) and reports it in the trace, not by freezing.

So "long renders never freeze the display" is fully achievable only on GPUs with
preemption. On the HD 4400, it holds for libawa-aware renderers and on a best-effort
basis for others.

## 4. Zero-copy disk → GPU

- **Integrated GPUs:** a Streamable VMO filled by the storage driver's DMA *is* GPU-visible
  memory. Mapping it into the GPU's address space (GTT/PPGTT) means weights or textures
  go from disk to GPU-accessible pages with no CPU copy. This works on your laptop.
- **Discrete GPUs:** true zero-copy needs **peer-to-peer DMA** (NVMe → GPU BAR). This
  requires resizable BAR, IOMMU/ACS support and a cooperating driver. Planned behind a
  capability (`P2pDma`), with a staged-copy fallback. Availability is a per-machine
  fact, reported by `gpu show`.

## 5. Display

- The window server owns scan-out through the GPU service's display protocol (mode
  setting, planes, vblank events). It runs at frame-deadline class with a reservation.
- Before any GPU driver exists, the window server draws to the **UEFI GOP framebuffer**
  handed over by Limine (software composition, single fixed mode).

## 6. Driver order

1. **virtio-gpu** (2D first, then 3D via virgl/venus only if useful for testing)
2. **Intel Gen7.5 (HD 4400)**: the bring-up target on real hardware; Intel publishes PRMs
3. **AMD** (documented registers; AMD publishes register headers and ISA docs)
4. **NVIDIA** last. It needs the GSP firmware and the published open-kernel-module
   interfaces as reference, and that repository's license must be checked before it is
   used even as documentation.

## 7. Vulkan and compute APIs (proposal A5)

- A GPU service alone gives display and buffers, not Vulkan. Applications (Blender,
  PyTorch backends, OpenCV) need a **Vulkan driver in user space**.
- Writing one per GPU family from scratch is a multi-year effort each. **Proposal:** port
  **Mesa's MIT-licensed user-space Vulkan drivers** (`hasvk` for Haswell, later `anv`
  and `radv`). They would sit on an AWA backend for buffer management and submission
  (`libawa-gpu`) instead of their usual kernel interface.
- This uses existing user-space code (permitted by the brief, MIT-licensed, no kernel
  code), but it is a large dependency. If rejected, the alternative is a minimal AWA
  compute API (buffers + SPIR-V compute dispatch) written from scratch, and no Vulkan
  graphics for a long time.
- CUDA cannot run on AWA (NVIDIA's user-space CUDA stack is closed and Linux/Windows-only).
  ML GPU acceleration on AWA means Vulkan compute or a ROCm port (AMD). Both are
  far-future and listed as risks in the roadmap.

## 8. Open items

- TODO(awa): GPU service ↔ client protocol (contexts, buffers, sync objects, fences)
  drafted at the GPU phase.
- TODO(awa): confirm whether HD 4400 IOMMU (VT-d graphics domain) is usable on your
  laptop's firmware.
