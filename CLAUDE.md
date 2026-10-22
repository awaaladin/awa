# AWA — standing rules for every session

Read `docs/SPEC.md`, `docs/ROADMAP.md` and the end of `docs/DESIGN_LOG.md` before
starting work. The spec in `/docs` is the source of truth; if code and spec disagree,
fix one of them in the same commit and record why in the design log.

## Locked decisions
- Kernel in Rust (nightly, `no_std`). Assembly only where unavoidable.
- x86_64 first; every milestone verified in QEMU + OVMF (UEFI) by an automated script
  that checks serial output. Builds and QEMU run in WSL2 (Ubuntu).
- Small core kernel; drivers and services are isolated user-space processes.
- Capability-based security, no ambient authority, W^X, ASLR, verified boot, signed updates.
- Bootloader: Limine first; AWA's own UEFI loader later.
- No code from Linux, BSD or any other kernel. Permissive (MIT/Apache/BSD/zlib)
  dependencies only. AWA is dual-licensed MIT OR Apache-2.0.
- Single owner per machine. Own native APIs first; a from-scratch Linux-application
  compatibility layer is a later, optional phase.
- Minimum hardware is the dev laptop (Haswell i7-4510U, 2C/4T, 8 GB, Intel HD 4400).
  AWA must be light enough to be useful there and scale up for ML/CGI machines.

## How to work
1. Be honest. Say when something is infeasible, premature or has a better alternative.
   Mark stubs `// STUB:` and unfinished work `// TODO(awa):`. Never present stubs as working.
2. Do not start a phase without the owner's go-ahead. Later phases (storage onward) need
   explicit approval.
3. Never say "done" without running the QEMU boot test and showing the output.
4. Small commits, update `CHANGELOG.md`, record major decisions in `docs/DESIGN_LOG.md`.
5. Every `unsafe` block has a `// SAFETY:` comment stating its invariant, and lives in a
   module listed in `kernel/UNSAFE.md`.
6. Keep `/docs` current.
7. Never claim AWA is faster or lighter than anything without benchmark numbers.
8. Push to `origin` (github.com/awaaladin/awa) after each reviewed step.
