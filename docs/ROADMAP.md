# AWA Roadmap

Every phase ends with an **automated QEMU + OVMF boot test** whose serial output is
checked by `tools/` scripts, and is shown to the owner before the phase is called done.
Phases from 6 onward need explicit owner approval before they start.

Status legend: ✅ done (verified) · 🔶 in review · ⬜ not started

## Phase 0 — Design specification 🔶
- Deliverables: `/docs` spec, security model, design log, roadmap; repository layout.
- Exit: owner approves the spec, including proposals A1–A10 in [SPEC.md §5](SPEC.md).

## Phase 1 — Toolchain and scaffold ⬜
- Cargo workspace; pinned nightly (`rust-toolchain.toml`); kernel target
  `x86_64-unknown-none` with a custom linker script (a full custom target JSON only if
  needed); Limine pinned by version and checksum; image builder producing a GPT disk
  image (ESP + Limine + kernel); `tools/qemu-run.sh` (q35, OVMF, serial on stdio,
  `isa-debug-exit`); `tools/test-boot.sh`; `cargo deny`; `DEPENDENCIES.md`;
  `kernel/UNSAFE.md`.
- Exit: `tools/test-boot.sh` boots the kernel in QEMU+OVMF and verifies
  **"AWA kernel booted"** on serial **and** on the framebuffer (QMP `screendump` compared
  with an expected glyph region), then QEMU exits with a success code. Optional: the
  same image boots on the laptop from USB.

## Phase 2 — CPU and memory foundations ⬜
- GDT/TSS (with IST stacks for #DF, NMI, #MC), IDT, exception handlers with serial
  backtraces; local APIC / x2APIC, I/O APIC; TSC calibration and TSC-deadline timer;
  physical frame allocator; own page tables (replacing Limine's), W^X kernel mapping,
  guard pages, SMEP (+SMAP/UMIP when present); kernel heap; kernel CSPRNG.
- Exit: tests in QEMU verify: a deliberate page fault is caught and reported; timer
  interrupts fire at the configured rate; frame allocator and heap stress tests pass;
  writing to kernel text faults (W^X proof); executing from the heap faults (NX proof).

## Phase 3 — Threads, scheduler, SMP ⬜
- Kernel threads, context switch, the class structure (frame-deadline with CBS
  reservations, interactive EEVDF, batch with gangs, idle); AP bring-up through Limine
  SMP; per-CPU run queues; IPIs and TLB shootdown; always-on counters and trace ring v1.
- Exit: on `-smp 4`, all CPUs report in; a test shows a frame-deadline thread meets its
  deadlines (zero misses over 10 s in QEMU, accepting TCG timing noise documented) while
  batch threads saturate all CPUs; gang threads start within a bounded skew; tracing
  overhead measured (number recorded, even under QEMU).

## Phase 4 — User space ⬜
- `syscall`/`sysret`, ring 3, KPTI, the vDSO, handles and rights, revocable forwarders,
  ELF loader (init only in kernel; launcher library for the rest), processes, channels,
  events, ports, `channel_call` fast path, exception channels, boot bundle format, init.
- Exit: init (ring 3) launches two processes which exchange messages and handles over a
  channel; a process without a handle cannot reach an object (negative tests); a
  process fault is reported and does not crash the kernel; a syscall from outside the
  vDSO is rejected; IPC round-trip time recorded; first syscall fuzz run with no kernel
  panics for N minutes.

## Phase 5 — Memory classes and accounting ⬜
- Account tree, quotas and targets, Required / Recomputable / Streamable VMOs, lock
  protocol, pressure ports, reclaim ladder, compressed store, budget levels, tradeoff
  report v1, memory-policy service, minimal serial shell with **`ram show`** and
  **`ram set`**.
- Exit (automated): a test app fills recomputable memory, the test lowers its limit, the
  kernel discards and notifies, and the app rebuilds; Required memory is compressed,
  never lost (checksummed); `ram show` output parsed and checked by the test script;
  degradation microbenchmark (RAM halved) completes without OOM.

## Later phases (each needs owner go-ahead and its own brief)

| Phase | Scope | Exit criteria (outline) |
|-------|-------|-------------------------|
| 6 Storage | PCI service, virtio-blk, AHCI, xHCI + USB mass storage, system image + hash tree, VFS, FAT32, data FS design, pager → Streamable files | Boots from USB on the laptop; verified system image mounted; large file mapped Streamable |
| 7 Drivers + input | PS/2 and USB HID keyboard/mouse, ACPI AML service (shutdown/reboot), RTC, IOMMU (VT-d) domains | Laptop keyboard works; clean power-off; DMA isolation test |
| 8 Networking | virtio-net, Intel I218-V (e1000e), TCP/IP stack (smoltcp or own; MIT/0BSD), DHCP, DNS, TLS (rustls) | Fetch an HTTPS URL in QEMU and on the laptop |
| 9 Graphics & window server | GOP framebuffer compositor, window protocol, frame-deadline integration, fonts, Settings app v1 | Frame-time benchmark with background render |
| 10 Native toolchain & POSIX | Rust `std` port (C1), libc (C2), C/C++ toolchain (C3) | Stages C1–C3 exit criteria |
| 11 Encryption & updates | Data partition encryption, A/B updates, own UEFI loader, signed update bundles | Interrupted update rolls back; tampered image refuses to boot |
| 12 Hypervisor & boxes | VMX core, VMM, virtio models, guest profile, light/VM boxes, `box` commands, download broker, `get` | `get` into a disposable VM box on the laptop; `box reset` works |
| 13 GPU | virtio-gpu, Intel Gen7.5, GPU accounting, `gpu show`, then Vulkan path (A5) | Display via GPU driver; GPU time per app in `gpu show` |
| 14 Python & ML/CV | C4–C6 | PyTorch CPU trains a small CNN |
| 15 3D tools | C7 | USD/glTF scene renders |
| 16 Linux-app layer (optional) | C8 | Unmodified Linux CPython runs in a box |
| 17 More hardware | AMD-V, AMD GPU, NVMe, Wi-Fi, NVIDIA | Per driver |

## Risk register

| Risk | Impact | Mitigation |
|------|--------|------------|
| Scope far exceeds one person + AI | Very high | Strict phase gates; publish nothing as working that isn't tested |
| No hardware-accelerated QEMU on Windows 10 | Slow tests; no VT-x testing | Keep tests small; real-hardware test procedure; optional Linux host with KVM |
| Laptop firmware quirks (USB boot, no DMAR) | Medium | Test boot on hardware early (Phase 1 optional step) |
| GPU stack size | Very high | Decision A5 |
| ML on AWA without CUDA | High | Vulkan compute / ROCm; set expectations |
