# AWA Design Log

Each entry records a decision, why it was made, and what was rejected. Entries are never
deleted; a reversed decision gets a new entry that references the old one.

Format: **D-NNN · date · status** (proposed / accepted / superseded)

---

### D-001 · 2026-10-22 · accepted (owner brief)
**Rust, `no_std`, nightly, for kernel and services.** Memory safety for most code;
`unsafe` confined to registered modules. Nightly is needed for features such as custom
targets and `-Z` flags. The toolchain is pinned to a dated nightly to keep builds
reproducible.

### D-002 · 2026-10-22 · accepted (owner brief)
**Small core kernel with user-space drivers.** A driver bug or malicious device should
cost one restartable process, not the machine.

### D-003 · 2026-10-22 · accepted (owner answer)
**Development host: WSL2 Ubuntu on Windows 10.** Bash scripts, QEMU, OVMF and Limine
tooling work cleanly there. Consequence: no KVM acceleration (Windows 10 doesn't
provide nested virtualization to WSL2), so QEMU runs in TCG mode.

### D-004 · 2026-10-22 · accepted (owner answer)
**Minimum target is the owner's laptop** (Haswell i7-4510U, 2C/4T, 8 GB, HD 4400,
mSATA AHCI). AWA must be light enough for it while scaling to ML/CGI hardware the owner
may not test. Consequences: Intel VT-x before AMD-V; AHCI and xHCI are the first
real-hardware storage drivers; Intel Gen7.5 is the first real GPU after virtio-gpu.

### D-005 · 2026-10-22 · accepted (owner answer)
**Single owner per machine.** Security relies on capabilities and boxes, not user
accounts. Rejected: multi-user accounts (more surface, little value for a personal dev OS).

### D-006 · 2026-10-22 · accepted (owner answer)
**License MIT OR Apache-2.0; permissive dependencies only.** GPL code (including Linux)
cannot be used.

### D-007 · 2026-10-22 · accepted (owner answer)
**Native APIs first; from-scratch Linux-app layer later and optional.** The owner does
not want AWA to be Linux. Consequence: until stage C8, "existing software runs
unmodified" is not met; apps must be recompiled.

### D-008 · 2026-10-22 · proposed (A10)
**Syscall ABI only through the vDSO; raw syscalls from elsewhere rejected.** Lets
syscall numbering evolve and blocks injected syscall gadgets. Rejected: a stable numbered
syscall ABI (freezes mistakes forever).

### D-009 · 2026-10-22 · proposed
**Limine as the first bootloader.** Supports UEFI and BIOS, gives a framebuffer, SMP
start-up, modules, KASLR and config hash enrollment, and has a maintained Rust crate.
Rejected for now: the `bootloader` crate (less flexible for modules and SMP),
writing our own loader (scheduled later; see D-021).

### D-010 · 2026-10-22 · proposed
**Handles + rights + revocable forwarders** for capabilities, rather than a global
capability-derivation tree. Bounded kernel memory and simpler code; revocation is still
possible wherever authority is handed out through a forwarder.

### D-011 · 2026-10-22 · proposed (A4)
**Tradeoff reporting: kernel-measured for all apps, semantic only when declared.**
The kernel can measure refaults, working set, discards and compression, but cannot know
what "batch size" or "preview quality" means inside an app. Inventing those would be
dishonest.

### D-012 · 2026-10-22 · proposed (A1)
**Reclaim order: recomputable → clean streamable → compress → swap.** Clean
streamable pages are free to drop, since they are already on disk. The brief's order
(recomputable → compress → page out) did not mention them.

### D-013 · 2026-10-22 · proposed (A2)
**Content-addressed sharing instead of blind scanning dedup.** Blind cross-domain
dedup enables documented side-channel attacks (copy-on-write timing). Content hashes from
the system image and VFS give the same benefit for models and asset libraries.

### D-014 · 2026-10-22 · proposed
**Recomputable memory uses an explicit lock/unlock chunk protocol.** Discarding memory
an app is reading would be unsafe. A lock protocol (as in known purgeable-memory designs)
makes discards safe and visible.

### D-015 · 2026-10-22 · proposed
**Frame-deadline class = EDF with admission control and CBS budget enforcement.**
Gives guaranteed time to the viewport while preventing a misbehaving frame thread from
starving everything (it is demoted when it overruns).

### D-016 · 2026-10-22 · proposed (A3)
**No in-kernel GPU driver.** Fault isolation is worth more than an unmeasured IPC cost
paid once per submission batch. Revisit only with benchmark data.

### D-017 · 2026-10-22 · proposed (A5)
**Port Mesa's MIT user-space Vulkan drivers** onto an AWA GPU backend. A from-scratch
Vulkan implementation per GPU family is unrealistic. The owner must accept this use of
existing (non-kernel) code, or accept no Vulkan for a long time.

### D-018 · 2026-10-22 · proposed (A7)
**Light boxes and VM boxes.** VMs for everything would waste memory on 8 GB machines.
Untrusted downloads always use VM boxes.

### D-019 · 2026-10-22 · proposed (A6)
**relibc (MIT, Rust) as the basis for AWA's libc.** It is designed to have multiple
platform backends. Writing a POSIX libc from zero would add a year with no
differentiating value. Rejected: musl (tightly bound to Linux syscalls), newlib (mixed
licenses).

### D-020 · 2026-10-22 · proposed (A9)
**Hypervisor tested on real hardware.** QEMU TCG cannot emulate VT-x and Windows 10
WSL2 has no nested KVM.

### D-021 · 2026-10-22 · proposed (A8)
**Own UEFI loader before claiming full verified A/B boot.** Limine's hash-pinned config
covers integrity but not slot selection with boot counting.

### D-022 · 2026-10-22 · proposed
**Immutable A/B system + encrypted data partition** on the boot drive. The owner asked
for the simplest working option for users: flash one image, set a passphrase, done.
Swap is off by default on USB flash media to protect the drive.

### D-023 · 2026-10-22 · proposed
**Kernel page-table isolation on by default** on Meltdown-affected CPUs, including the
target laptop. The syscall/IPC cost is accepted and partly recovered with PCID; the
IPC budget in HARDWARE.md assumes KPTI on.
