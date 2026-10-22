# AWA Design Specification

Version 0.1 (draft for review) · 2026-10-22 · Status: **awaiting owner approval**

This is the top-level design document. Each subsystem has its own chapter in
[`spec/`](spec/). Nothing here is implemented yet.

## 1. Goals

AWA is an operating system for people who build ML, computer vision, animation and CGI
software. It boots from removable media (USB flash drive or external SSD) and must:

- **Stay responsive under heavy load.** A background render or training job must not
  make the viewport, playback or UI stutter.
- **Use memory deliberately.** It should know which memory is precious, which can be
  rebuilt and which is just a disk cache, and act on that under pressure.
- **Be light.** It must be useful on an 8 GB, 2-core laptop (see [HARDWARE.md](HARDWARE.md))
  and scale up to workstation hardware.
- **Be secure by construction.** It uses capabilities and no ambient authority, isolated
  drivers, sandboxes for untrusted downloads, and verified boot.
- **Be observable.** Every byte of RAM, GPU time and disk I/O is attributable to an app.

## 2. Non-goals (for now)

- Binary compatibility with Linux applications. This is planned as a later,
  optional, from-scratch layer ([spec/08](spec/08-compatibility.md)), not a foundation.
- Multiple user accounts on one machine. AWA has one owner per machine.
- Architectures other than x86_64. The design avoids x86-only assumptions in its
  interfaces so aarch64 stays possible.
- Server, real-time-industrial or phone use cases.

## 3. Chapters

| # | Chapter | Covers |
|---|---------|--------|
| 01 | [Kernel architecture](spec/01-kernel-architecture.md) | What is in the kernel and what is not; kernel objects; address-space layout |
| 02 | [Memory](spec/02-memory.md) | Memory classes API, accounting, quotas, pressure, compression, dedup |
| 03 | [Scheduler](spec/03-scheduler.md) | Frame-deadline / interactive / batch classes, gangs, placement |
| 04 | [Capabilities](spec/04-capabilities.md) | Handles, rights, transfer, revocation, the powerbox |
| 05 | [Syscalls and IPC](spec/05-syscalls-ipc.md) | Syscall ABI, vDSO, channels, message format, IDL |
| 06 | [GPU](spec/06-gpu.md) | GPU service, unified accounting, preemption, zero-copy |
| 07 | [Sandboxes](spec/07-sandboxes.md) | Hypervisor, microVM boxes, download broker |
| 08 | [Compatibility](spec/08-compatibility.md) | Rust std, libc, C/C++, Python, ML stack, Linux-app layer |
| 09 | [Boot, update, storage](spec/09-boot-update-storage.md) | Boot chain, disk layout, A/B updates, encryption |
| 10 | [Tracing and profiler](spec/10-tracing.md) | Always-on attribution, trace buffers, profiler |
| 11 | [User interface](spec/11-user-interface.md) | `ram`, `gpu`, `box`, `get` commands; Settings app |

Cross-cutting documents:

- [SECURITY.md](SECURITY.md): threat model, overall and per component
- [HARDWARE.md](HARDWARE.md): target hardware, CPU feature facts, performance budgets
- [BENCHMARKS.md](BENCHMARKS.md): success metrics and how they are measured
- [ROADMAP.md](ROADMAP.md): phases and exit criteria
- [DESIGN_LOG.md](DESIGN_LOG.md): decisions and why they were made
- [DEVELOPMENT.md](DEVELOPMENT.md): toolchain, workflow, `unsafe` policy

## 4. Architecture at a glance

```
 ┌───────────────────────────── user space (ring 3) ─────────────────────────────┐
 │  apps (native libawa / POSIX)   shell, ram/gpu/box/get   Settings   profiler   │
 │  ─────────────────────────────────────────────────────────────────────────────│
 │  system services:  init · service manager · window server · VFS · net stack   │
 │                    memory-policy · download broker · VMM (one per box)        │
 │  drivers:          PCI/ACPI · AHCI · NVMe · xHCI/USB storage · virtio-* ·     │
 │                    GPU (virtio-gpu → Intel → AMD → NVIDIA) · e1000e · input   │
 └──────────────────────────────────── syscalls (vDSO) ──────────────────────────┘
 ┌───────────────────────────── AWA kernel (ring 0) ─────────────────────────────┐
 │  capabilities · IPC · threads + scheduler classes · address spaces + VMOs     │
 │  memory classes, accounting, compression · interrupts → events · IOMMU        │
 │  timers · always-on trace buffers · VMX core (VM entry/exit, EPT)             │
 └───────────────────────────────────────────────────────────────────────────────┘
          Limine (UEFI) → kernel + boot bundle (init, early drivers)
```

The kernel holds only what must be privileged or on the hot path: scheduling, address
spaces, the memory manager (including memory classes and compression, because they
live in the page-fault path), IPC, capabilities, interrupt routing, IOMMU programming,
tracing buffers and the VM-entry core of the hypervisor. Everything else, including
every device driver and the GPU driver, runs as a user-space process.

## 5. Decisions that need your approval

These items change or interpret the original brief. Each is argued in the chapter and
the design log. Approve, reject or amend each one.

| # | Proposal | Where |
|---|----------|-------|
| A1 | Under memory pressure, evict **clean streamable pages before compressing** anonymous memory. They are already on disk, so dropping them costs nothing to write. The brief said: recomputable, then compress, then page out. | [02 §6](spec/02-memory.md), D-012 |
| A2 | **No blind page-scanning dedup across sandboxes.** Sharing is explicit and content-addressed (same file hash ⇒ same pages). Blind scanning enables known cross-VM side-channel attacks. | [02 §8](spec/02-memory.md), D-013 |
| A3 | **No GPU exception in the kernel.** The GPU driver runs in user space, and the kernel only provides DMA/IOMMU and accounting. Revisit only if benchmarks show a need. | [06](spec/06-gpu.md), D-016 |
| A4 | "What a lower limit costs" is **measured by the kernel for every app**, such as refault rate and recomputation count. **Semantic costs** such as batch size or preview quality appear only for apps that declare them through libawa. | [02 §5](spec/02-memory.md), D-011 |
| A5 | Realistic GPU acceleration (Vulkan) means **porting Mesa's MIT-licensed user-space drivers** onto AWA's GPU service. This is not kernel code, but it is existing code. The alternative, a from-scratch Vulkan driver, is a multi-year effort per GPU family. | [06 §7](spec/06-gpu.md), D-017 |
| A6 | Base the POSIX libc on **relibc** (MIT, written in Rust, designed to be ported to new kernels) with an AWA backend, rather than writing a libc from zero. | [08](spec/08-compatibility.md), D-019 |
| A7 | **Two box tiers**: *light boxes* (capability-confined process groups, nearly free) and *VM boxes* (hardware-isolated microVMs). Downloads go to VM boxes by default. On 8 GB of RAM, a VM for everything is too heavy. | [07](spec/07-sandboxes.md), D-018 |
| A8 | A **fully verified A/B boot chain needs AWA's own UEFI loader**. Until then, the chain is Secure Boot → Limine (with an enrolled, hash-pinned config) → kernel verifies the system image. This is weaker, as documented. | [09](spec/09-boot-update-storage.md), D-021 |
| A9 | The **hypervisor cannot be tested automatically** on your machine. QEMU's software emulator has no VT-x, and Windows 10 WSL2 does not provide nested KVM. VM-box milestones are tested on the real laptop, or on a Linux host with nested KVM. | [07 §9](spec/07-sandboxes.md), D-020 |
| A10 | The **syscall ABI is private**; programs call the kernel only through the AWA vDSO. Syscall numbers can then change without breaking binaries. | [05 §2](spec/05-syscalls-ipc.md), D-008 |

## 6. Honest constraints

- **This is a very large project.** Phases 1–5 are a kernel with processes and memory
  classes. Each later phase (storage, networking, graphics, POSIX, hypervisor, GPU) is
  comparable in size to the whole kernel. Running PyTorch or Blender is years of work,
  not months.
- **Your laptop's CPU lacks SMAP, CET and PKU.** They arrived in later Intel generations.
  The design treats them as optional hardening and must not depend on them. The CPU is
  also affected by Meltdown, MDS and L1TF, so kernel page-table isolation and buffer
  clearing are required ([HARDWARE.md](HARDWARE.md)).
- **The Intel HD 4400 cannot run serious ML or CGI workloads.** It is a bring-up target.
  ML/CGI performance targets assume stronger hardware, and you have said you may not
  test those on this machine.
- **The Wi-Fi chip (Intel 7260) needs a closed firmware blob and a large driver.** Ethernet
  (Intel I218-V) and USB Ethernet come first.
- **QEMU in WSL2 on Windows 10 runs without hardware acceleration (TCG).** Boot tests will
  be slower than on real hardware, and timing-sensitive tests must allow for that.

## 7. Glossary

- **Capability / handle**: an unforgeable, per-process reference to a kernel object
  together with a set of rights. The only way to act on anything.
- **VMO (virtual memory object)**: a kernel object representing a range of memory pages,
  which can be mapped into address spaces. It carries a memory class.
- **Memory account**: a node in a tree (system → box → process) that RAM is charged to,
  and that holds a quota.
- **Box**: a sandbox, either a light box (process group) or a VM box (microVM).
- **Service**: a user-space process providing an IPC protocol, such as a driver, the VFS
  or the window server.
- **Boot bundle**: a read-only archive loaded by the bootloader next to the kernel. It
  contains init and the drivers needed to reach the system image.
