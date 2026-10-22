# AWA Security Model and Threat Model

Status: draft · Not implemented

## 1. Assets

1. The owner's files and secrets (data partition, credentials inside apps)
2. Integrity of the AWA system (kernel, boot bundle, system image)
3. Isolation between boxes, and between boxes and the host
4. Availability of the interactive experience (no denial of service by one app)

## 2. Adversaries in scope

| Adversary | Example |
|-----------|---------|
| Malicious downloaded file or program | A trojan installer, a crafted image file exploiting a parser |
| Malicious or compromised app | A plugin that tries to read other projects or the network |
| Network attacker | Hostile Wi-Fi, malicious server, MITM on updates |
| Compromised driver or service | A bug in the USB stack exploited by a malicious USB device |
| Malicious guest in a VM box | Attempts to escape the VMM or attack the host via CPU side channels |
| Brief physical access / lost drive | The USB drive is stolen, or modified and returned (evil maid) |

## 3. Out of scope (stated honestly)

- Compromised firmware, SMM or Intel ME; hardware implants
- An attacker with the device while it is **unlocked and running**
- Side channels not mitigable on the hardware (AWA applies known mitigations and
  documents what remains)
- Rubber-hose or coercion attacks against the owner

## 4. System-wide mechanisms

| Mechanism | Implementation |
|-----------|----------------|
| No ambient authority | Capabilities only ([spec/04](spec/04-capabilities.md)) |
| Least privilege for drivers | User-space drivers with narrowed MMIO, IRQ and DMA capabilities, IOMMU-enforced where available |
| W^X | Kernel: per-section page permissions, no RWX. User: loader enforces segment permissions; `vmar_protect` refuses W+X; JITs need a `Jit` capability and use dual mappings |
| ASLR | Kernel base randomized by Limine (relocatable kernel); user: PIE required, random stack, heap, mmap and vDSO bases; handle values tweaked per process |
| Stack protection | Kernel stacks with guard pages; `-Z stack-protector=strong` for C code in the POSIX layer; Rust's bounds checks for Rust code. Shadow stacks (CET) used when present (not on Haswell) |
| SMEP / SMAP / UMIP | SMEP on (Haswell supports it). SMAP and UMIP used when present (**not on Haswell**); `copy_from_user` discipline is the fallback |
| Speculative execution | Kernel page-table isolation on Meltdown-affected CPUs; `VERW` buffer clearing (MDS) on return to user and VM entry; L1D flush on VM entry (L1TF); retpoline or IBRS as the CPU requires; core scheduling for VM boxes running untrusted code |
| Randomness | RDRAND (Haswell; RDSEED absent) mixed with TSC jitter and boot-time entropy into a ChaCha20 CSPRNG; RDRAND is never trusted alone |
| Verified boot | Secure Boot → signed Limine → hash-pinned kernel/bundle → signed system-image root hash → per-block hash checks ([spec/09](spec/09-boot-update-storage.md)) |
| Signed updates | Ed25519 manifests, monotonic versions, anti-rollback |
| Data at rest | AES-256-XTS on the data partition, key from passphrase via Argon2id |
| Memory-safety | Rust for kernel and services; `unsafe` confined, registered, commented |

## 5. Per-component threat models

Each component's own document must have a section in this format, kept up to date.

### Kernel
- **Attack surface:** syscalls (via vDSO), exception handlers, IPC message handling,
  hypervisor exits.
- **Threats:** memory-corruption bugs, confused-deputy use of handles, TOCTOU on user
  memory, resource exhaustion, speculative leaks.
- **Controls:** handles checked for type and rights on every call; user memory copied
  once before validation; all kernel allocations charged to the caller's account;
  fuzzing of the syscall layer (syzkaller-style, own harness) from Phase 4.

### IPC / services
- **Threats:** malformed messages, handle-type confusion, a slow or malicious client
  holding server resources.
- **Controls:** generated decoders validate every message; servers never trust sizes or
  handle types from clients; per-client resource limits; servers run under the client's
  donated scheduling context but charge memory to the client's account.

### Drivers
- **Threats:** a malicious device (BadUSB, crafted descriptors), driver bugs, DMA attacks.
- **Controls:** one process per driver; IOMMU domains per device; descriptor parsing
  in safe Rust; crash → restart without kernel impact. **Without an IOMMU, DMA-capable
  drivers are trusted** and the system says so.

### GPU service
- **Threats:** a malicious command buffer reading other contexts' memory, GPU hangs.
- **Controls:** per-context GPU page tables (PPGTT on Intel); command-buffer validation
  where the hardware lacks per-context isolation; watchdog resets.

### VMM and hypervisor
- **Threats:** guest-to-VMM exploits through virtio device models; VMM-to-kernel
  exploits through hypervisor syscalls; side channels.
- **Controls:** the VMM is an unprivileged process per box; virtio models written in
  safe Rust and fuzzed; kernel exit handling minimal; L1TF/MDS mitigations and core
  scheduling (see [spec/07 §7](spec/07-sandboxes.md)).

### Download broker
- **Threats:** a malicious URL or response exploiting the downloader; tricking the owner
  into exporting a malicious file.
- **Controls:** fetching and parsing happen inside the target box; the host handles only
  the URL string and the final hash; exporting is explicit and shows the file type.

### Update agent
- **Threats:** forged or replayed updates, interrupted updates.
- **Controls:** signature and version checks before writing; A/B slots; boot counting
  (after own loader); offline signing keys.

### Window server
- **Threats:** UI spoofing (a box pretending to be the system), input snooping, screen scraping.
- **Controls:** box borders drawn by the server; input delivered only to the focused
  window; no screen-capture capability by default; a trusted path for passphrase entry.

## 6. Security process

- Every `unsafe` block carries a `// SAFETY:` comment; modules with `unsafe` are listed
  in `kernel/UNSAFE.md` and reviewed when changed.
- Dependencies are pinned, license-checked and reviewed (`cargo deny` in CI from Phase 1).
- Fuzzing targets: syscall layer, IPC decoders, ELF loader, virtio models, file-system
  parsers, USB descriptor parsing.
