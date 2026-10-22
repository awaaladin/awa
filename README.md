# AWA

AWA is a from-scratch operating system and kernel written in Rust. It boots from a
USB flash drive or external SSD, and is designed to be a fast, light and secure
developer environment for machine learning, computer vision, animation and CGI.

AWA is **not** based on Linux, BSD or any other kernel, and contains no code from them.
It uses permissively licensed Rust crates and public specifications (UEFI, ACPI,
xHCI, AHCI, NVMe, virtio, Vulkan, POSIX).

> **Status: Phase 0 — design specification, awaiting review.**
> There is no kernel code yet. Nothing described in `/docs` is implemented.

## What makes AWA different

1. **Memory classes.** Every allocation is *Required*, *Recomputable* (the kernel may
   drop it under pressure and tell the app to rebuild it) or *Streamable* (backed by
   disk, loaded page by page).
2. **Per-app and per-sandbox RAM limits** you can change at runtime, with a plain
   explanation of what a lower limit costs.
3. **Shared read-only memory**, so ten tools using the same model cost one copy.
4. **Scheduler classes**: frame-deadline, interactive and batch, so a background render
   never makes the viewport stutter.
5. **GPU as a first-class resource**, with unified CPU/GPU memory accounting.
6. **Always-on tracing** of RAM, CPU, GPU and disk per app, exposed as a profiler.
7. **Native sandboxes** (hardware-isolated microVMs) with a download broker.

None of these is a claim of speed. AWA will only claim to be faster or lighter than
something when the benchmark suite in [`bench/`](bench/) shows numbers.

## Repository layout

| Path        | Contents |
|-------------|----------|
| `docs/`     | Specification, security model, design log, roadmap |
| `kernel/`   | The AWA kernel |
| `boot/`     | Bootloader configuration, later AWA's own UEFI loader |
| `services/` | User-space drivers and system services |
| `libawa/`   | Native user-space library: memory classes, scheduling, capabilities |
| `userland/` | Shell, `ram`/`gpu`/`box`/`get` tools, Settings app |
| `tools/`    | Image builder, QEMU scripts, test runner |
| `bench/`    | Benchmark suite |
| `tests/`    | Integration and boot tests |

## Start reading

- [docs/SPEC.md](docs/SPEC.md) — the design specification (start here)
- [docs/ROADMAP.md](docs/ROADMAP.md) — phases and exit criteria
- [docs/SECURITY.md](docs/SECURITY.md) — threat model
- [docs/DESIGN_LOG.md](docs/DESIGN_LOG.md) — why each major decision was made
- [docs/HARDWARE.md](docs/HARDWARE.md) — target hardware and performance budgets

## License

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT)
at your option.
