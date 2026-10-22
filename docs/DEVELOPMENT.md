# Development guide

Status: the toolchain is set up in Phase 1. This document describes the plan.

## 1. Host setup (WSL2 Ubuntu on Windows 10)

- Use the **Ubuntu** WSL2 distribution (Kali is currently the default; run
  `wsl -s Ubuntu` or call `wsl -d Ubuntu`).
- Packages (Phase 1 will provide `tools/setup-host.sh`): `qemu-system-x86`, `ovmf`,
  `mtools`, `xorriso`, `gdisk`, `build-essential`, `curl`, `git`.
- Rust via rustup; the nightly version is pinned in `rust-toolchain.toml`.
- Limine is downloaded at a pinned version and its checksum is verified by
  `tools/fetch-limine.sh`. It is not vendored into git.
- The repo lives at `C:\Users\Owner\Documents\awa` (edited from VS Code). Builds from
  WSL access it via `/mnt/c/...`, which is slower. If builds are too slow, move the
  clone into the WSL file system and use VS Code's WSL remote.

## 2. Running

```
tools/qemu-run.sh          # interactive run, serial on the terminal
tools/test-boot.sh         # automated boot test (exit code 0 = pass)
```

QEMU runs without KVM (TCG). Expect a few seconds per boot.

## 3. Real hardware

- Write the image to a USB drive from Windows with Rufus or balenaEtcher ("DD image" mode).
- Serial output on the laptop is not available through a port. Logs go to the
  framebuffer and, from the Storage phase, to the data partition.

## 4. Commit and review rules

- Small commits, imperative subject lines, a `CHANGELOG.md` entry for user-visible
  changes.
- A new major decision gets a `docs/DESIGN_LOG.md` entry in the same commit.
- Push to `origin` (github.com/awaaladin/awa) after each reviewed step.

## 5. `unsafe` policy

- Allowed only in modules listed in `kernel/UNSAFE.md` (or the service's equivalent).
- Every `unsafe` block has a `// SAFETY:` comment stating the invariant that makes it
  sound, and who upholds it.
- `#![deny(unsafe_op_in_unsafe_fn)]`, `clippy::undocumented_unsafe_blocks = deny`.
- Prefer one small audited abstraction (e.g. `Mmio<T>`, `PhysFrame`, `UserPtr<T>`) used
  by safe code over scattered `unsafe`.

## 6. Stubs

Unfinished code is marked `// STUB:` (does nothing real) or `// TODO(awa):` (incomplete).
`tools/stubs.sh` (Phase 1) lists them; the list goes into each phase review.
