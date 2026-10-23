# AWA

AWA is an operating system and kernel written from scratch in Rust. It boots from a
USB flash drive or external SSD and is built as a developer environment for machine
learning, computer vision, animation and CGI.

AWA is not based on Linux, BSD or any other kernel.

> **Status: early development.** AWA does not yet do anything useful for end users.
> This README documents only what works today.

## Planned features

- **Memory classes:** apps mark memory as required, rebuildable or disk-backed, so AWA
  can make smart choices when RAM runs low.
- **Per-app and per-sandbox RAM limits** you can change at any time, with a plain
  explanation of what a lower limit costs.
- **Smooth viewports:** scheduling built so a background render never makes the
  interface stutter.
- **Sandboxes** for anything you download.
- **A built-in profiler** that shows RAM, CPU, GPU and disk use per app.

## What works today

- Boots on x86_64 PCs through the Limine bootloader, in both **UEFI** and
  **legacy BIOS** mode, from one disk image.
- Prints `AWA kernel booted` on the serial port and on screen.

## Building

You need:

- **Rust** via [rustup](https://rustup.rs). The exact nightly version is pinned in
  `rust-toolchain.toml` and installed automatically. On Windows, use the
  `x86_64-pc-windows-gnu` host so no Visual Studio is needed.
- **Python 3.10+** (standard library only).
- **QEMU** (`qemu-system-x86_64`) with its bundled UEFI firmware, to run AWA in a
  virtual machine.
- The **Limine 12.9.4** binary release (`limine-binary.tar.gz` from the Limine GitHub
  releases), unpacked into `<build dir>/limine/limine-binary`. Its files are checked
  against `tools/limine.sha256`.

The build directory defaults to `C:/awa-build` if it exists, otherwise `./build`.
Override it with `AWA_BUILD_DIR`. QEMU is looked for in `<build dir>/qemu`, then
`AWA_QEMU_DIR`, then `PATH`.

```
python tools/awa.py build        # compile the kernel and the image tool
python tools/awa.py image        # create <build dir>/awa.img (64 MiB, BIOS + UEFI)
python tools/awa.py run          # boot it in a QEMU window (UEFI)
python tools/awa.py run --bios   # boot it with legacy BIOS
python tools/awa.py test         # automated boot test (UEFI and BIOS)
```

`test` boots the image headless, waits for `AWA kernel booted` on serial, takes a
screenshot and compares the on-screen text with `tests/golden/banner.txt`. It exits
with code 0 only if both firmware types pass.

## Running on real hardware

Write `awa.img` to a USB drive with a raw image writer (for example Rufus in "DD
image" mode). **This erases the drive.** Then boot the PC from it (on most laptops,
press F12 at power-on and pick the USB drive). Today you will see the boot message
and nothing else; remove the drive and restart to return to your normal system.

## License

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT)
at your option.
