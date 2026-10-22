# Target hardware and performance budgets

## 1. Development and minimum target machine

Read from the owner's PC on 2026-10-22:

| Part | Model | AWA driver / notes |
|------|-------|--------------------|
| Machine | Toshiba Portégé Z30-A | UEFI; Secure Boot support to be confirmed |
| CPU | Intel Core i7-4510U (Haswell, 2 cores / 4 threads, 2.0 GHz) | x86_64, VT-x, VT-d (CPU supports it; firmware DMAR table to be confirmed) |
| RAM | 8 GB | Sets the "light" budget |
| GPU | Intel HD Graphics 4400 (Gen7.5) | Framebuffer first, then Intel Gen7.5 driver (public PRMs) |
| Storage | SK hynix SC210 mSATA 256 GB | **AHCI** (SATA), not NVMe |
| USB | Intel 8 Series xHCI (USB 3.0) + EHCI | xHCI + USB mass storage needed to boot from USB |
| Ethernet | Intel I218-V | e1000e-family, documented: first real-NIC target |
| Wi-Fi | Intel Dual Band Wireless-AC 7260 | Needs closed firmware blob + large driver: **late** |
| Bluetooth | Intel (combo with 7260) | Not planned |

Windows reports "virtualization firmware enabled: False". Check the BIOS setting before
the hypervisor phase.

### CPU features that matter (Haswell)

| Feature | Present | Consequence |
|---------|---------|-------------|
| NX, SMEP | yes | Used |
| SMAP, UMIP, PKU, CET | **no** | Optional hardening only; must not be relied on |
| PCID, INVPCID | yes | Makes kernel page-table isolation cheaper |
| x2APIC, TSC-deadline, invariant TSC | yes | Tickless scheduler, precise timers |
| RDRAND | yes | Mixed into CSPRNG, never trusted alone |
| RDSEED | no | — |
| AES-NI, AVX2, BMI2 | yes | Disk encryption; fast compression and hashing |
| VT-x with EPT, unrestricted guest | yes | Hypervisor |
| Meltdown, MDS, L1TF, Spectre v1/v2 affected | **yes** | Mitigations required, on by default |
| TSX | disabled by microcode on most parts | Not used |

## 2. Upper target (ML / CGI)

AWA must also scale to machines the owner may not test personally: 8–64 cores, 32–256
GB RAM, NVMe, discrete AMD/Intel GPUs (later NVIDIA). Algorithms must not assume small
core counts (per-CPU structures, no global locks on hot paths).

## 3. Performance budgets

These are **targets, not measurements**. Each will be measured on the laptop (real
hardware, not QEMU) once the relevant phase exists, and reported in `bench/results/`.

| Budget | Target on the laptop | Measured in |
|--------|----------------------|-------------|
| Kernel image size (release) | ≤ 4 MiB | Phase 1 |
| RAM in use at serial shell, after boot | ≤ 64 MiB | Phase 5 |
| RAM in use at idle desktop (window server + Settings) | ≤ 350 MiB | Graphics phase |
| Boot: firmware handoff → shell, USB 3 SSD | ≤ 5 s | Storage phase |
| IPC `channel_call` round trip, 64-byte message, KPTI on | ≤ 3 µs | Phase 4 |
| Frame-deadline wakeup latency p99, with batch load on all CPUs | ≤ 200 µs | Phase 3 |
| Viewport frame time p99 at 60 Hz, with a background batch render | ≤ 1.2 × 16.7 ms | Graphics phase |
| Always-on tracing overhead | ≤ 1 % | Phase 3 |
| VM box idle overhead | ≤ 64 MiB, start ≤ 1 s | Hypervisor phase |

If a target is missed, the result is reported as missed, with the measured number.
