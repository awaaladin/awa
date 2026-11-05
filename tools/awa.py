#!/usr/bin/env python3
"""AWA developer tool: build, make the disk image, run and test in QEMU.

    python tools/awa.py build            build kernel + image tool
    python tools/awa.py image            build, then create the boot disk image
    python tools/awa.py run [--bios]     boot the image in QEMU with a window (UEFI by default)
    python tools/awa.py test             automated boot test, UEFI and BIOS

Only the Python standard library is used. Paths can be overridden with
environment variables:

    AWA_BUILD_DIR   build output, toolchain downloads   (default: C:/awa-build if present, else ./build)
    AWA_QEMU_DIR    folder containing qemu-system-x86_64 (default: $AWA_BUILD_DIR/qemu, else PATH)
    AWA_LIMINE_DIR  unpacked Limine binary release       (default: $AWA_BUILD_DIR/limine/limine-binary)
"""

import argparse
import hashlib
import json
import os
import shutil
import socket
import struct
import subprocess
import sys
import time
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BOOT_MESSAGE = "AWA kernel booted"
PANIC_MARKER = "KERNEL PANIC"

# Must match kernel/src/console.rs.
BACKGROUND = (0x10, 0x18, 0x20)
FOREGROUND = (0xE8, 0xF0, 0xFF)
BANNER_X, BANNER_Y, BANNER_SCALE = 32, 32, 2
GLYPH_HEIGHT = 16
# Generous width for the banner box: 32 px per character at scale 2.
BANNER_BOX_W = len(BOOT_MESSAGE) * 16 * BANNER_SCALE
BANNER_BOX_H = GLYPH_HEIGHT * BANNER_SCALE

GOLDEN = ROOT / "tests" / "golden" / "banner.txt"


def build_dir() -> Path:
    env = os.environ.get("AWA_BUILD_DIR")
    if env:
        return Path(env)
    if Path("C:/awa-build").is_dir():
        return Path("C:/awa-build")
    return ROOT / "build"


def limine_dir() -> Path:
    return Path(os.environ.get("AWA_LIMINE_DIR", build_dir() / "limine" / "limine-binary"))


def exe(name: str) -> str:
    return name + (".exe" if os.name == "nt" else "")


def qemu_binary() -> str:
    candidates = []
    if os.environ.get("AWA_QEMU_DIR"):
        candidates.append(Path(os.environ["AWA_QEMU_DIR"]) / exe("qemu-system-x86_64"))
    candidates.append(build_dir() / "qemu" / exe("qemu-system-x86_64"))
    for c in candidates:
        if c.is_file():
            return str(c)
    found = shutil.which("qemu-system-x86_64")
    if found:
        return found
    sys.exit("awa: qemu-system-x86_64 not found (set AWA_QEMU_DIR)")


def firmware_files(qemu: str) -> tuple[Path, Path]:
    """Returns (OVMF code, OVMF vars template)."""
    qdir = Path(qemu).parent
    pairs = [
        (qdir / "share" / "edk2-x86_64-code.fd", qdir / "share" / "edk2-i386-vars.fd"),
        (Path("/usr/share/OVMF/OVMF_CODE_4M.fd"), Path("/usr/share/OVMF/OVMF_VARS_4M.fd")),
        (Path("/usr/share/OVMF/OVMF_CODE.fd"), Path("/usr/share/OVMF/OVMF_VARS.fd")),
    ]
    for code, vars_ in pairs:
        if code.is_file() and vars_.is_file():
            return code, vars_
    sys.exit("awa: OVMF/edk2 UEFI firmware not found")


def run(cmd: list, **kw) -> None:
    print("+", " ".join(str(c) for c in cmd), flush=True)
    subprocess.run([str(c) for c in cmd], check=True, **kw)


# ---------------------------------------------------------------- build

def cargo_env() -> dict:
    env = dict(os.environ)
    env.setdefault("CARGO_TARGET_DIR", str(build_dir() / "target"))
    if os.name == "nt" and (build_dir() / "rustup").is_dir():
        env.setdefault("RUSTUP_HOME", str(build_dir() / "rustup"))
    return env


def cargo() -> str:
    found = shutil.which("cargo")
    if found:
        return found
    home = Path(os.environ.get("CARGO_HOME", Path.home() / ".cargo"))
    path = home / "bin" / exe("cargo")
    if path.is_file():
        return str(path)
    sys.exit("awa: cargo not found")


# User programs loaded next to the kernel (Limine modules), in boot order.
PROGRAMS = ["init", "ping", "pong", "crashme", "noperm"]

KERNEL_RUSTFLAGS = "-C code-model=kernel -C relocation-model=static -C force-frame-pointers=yes"
USER_RUSTFLAGS = "-C relocation-model=pie -C force-frame-pointers=yes"


def build_programs() -> dict:
    """Builds the user programs (position-independent, ordinary code model)."""
    env = cargo_env()
    env["CARGO_TARGET_DIR"] = str(build_dir() / "target-user")
    env["RUSTFLAGS"] = USER_RUSTFLAGS
    run([cargo(), "build", "--release", "--target", "x86_64-unknown-none"], cwd=ROOT / "userland", env=env)
    out = Path(env["CARGO_TARGET_DIR"]) / "x86_64-unknown-none" / "release"
    return {name: out / name for name in PROGRAMS}


def cmd_build(_args=None) -> tuple[Path, Path]:
    env = cargo_env()
    kenv = dict(env, RUSTFLAGS=KERNEL_RUSTFLAGS)
    run([cargo(), "build", "--release", "-p", "awa-kernel", "--target", "x86_64-unknown-none"], cwd=ROOT, env=kenv)
    run([cargo(), "build", "--release", "-p", "awa-mkimage"], cwd=ROOT, env=env)
    target = Path(env["CARGO_TARGET_DIR"])
    kernel = target / "x86_64-unknown-none" / "release" / "awa-kernel"
    mkimage = target / "release" / exe("awa-mkimage")
    print(f"awa: kernel {kernel} ({kernel.stat().st_size} bytes)")
    return kernel, mkimage


def verify_limine() -> Path:
    ldir = limine_dir()
    lock = ROOT / "tools" / "limine.sha256"
    expected = {}
    for line in lock.read_text().splitlines():
        if line.strip() and not line.startswith("#"):
            digest, name = line.split(maxsplit=1)
            expected[name.strip().lstrip("*")] = digest
    for name, digest in expected.items():
        path = ldir / name
        if not path.is_file():
            sys.exit(f"awa: missing Limine file {path}")
        actual = hashlib.sha256(path.read_bytes()).hexdigest()
        if actual != digest:
            sys.exit(f"awa: checksum mismatch for {path}\n  expected {digest}\n  actual   {actual}")
    return ldir


def cmd_image(_args=None) -> Path:
    kernel, mkimage = cmd_build()
    programs = build_programs()
    ldir = verify_limine()
    out = build_dir() / "awa.img"
    run([
        mkimage, out, 64,
        f"boot/awa-kernel={kernel}",
        *[f"boot/{name}={path}" for name, path in programs.items()],
        f"boot/limine/limine.conf={ROOT / 'boot' / 'limine.conf'}",
        f"boot/limine/limine-bios.sys={ldir / 'limine-bios.sys'}",
        f"EFI/BOOT/BOOTX64.EFI={ldir / 'BOOTX64.EFI'}",
    ])
    tool = ldir / "limine-tool-windows-x86" / "limine.exe" if os.name == "nt" else shutil.which("limine")
    if not tool:
        sys.exit("awa: Limine host tool not found (needed for BIOS boot)")
    run([tool, "bios-install", out])
    return out


# ---------------------------------------------------------------- USB (UEFI, file copy only)

def cmd_usb(args) -> None:
    """Copies AWA onto an existing FAT32 USB drive for UEFI boot.

    Only ordinary files are written (EFI/BOOT/BOOTX64.EFI and boot/...). The
    boot sector and every existing file are left alone, so the drive keeps
    its contents. Legacy BIOS boot is not set up: that needs a write to the
    boot sector and enough free space before the first partition.
    """
    root = Path(args.drive.rstrip("\\/") + "/")
    if not root.is_dir():
        sys.exit(f"awa: drive {args.drive} not found")
    kernel, _ = cmd_build()
    programs = build_programs()
    ldir = verify_limine()
    plan = {
        root / "EFI" / "BOOT" / "BOOTX64.EFI": ldir / "BOOTX64.EFI",
        root / "boot" / "limine" / "limine.conf": ROOT / "boot" / "limine.conf",
        root / "boot" / "awa-kernel": kernel,
        **{root / "boot" / name: path for name, path in programs.items()},
    }
    marker = root / "boot" / "awa-installed.txt"
    for dest, src in plan.items():
        if dest.exists() and not marker.exists() and dest.read_bytes() != src.read_bytes():
            sys.exit(f"awa: {dest} already exists and was not put there by AWA; not overwriting it")
    for dest, src in plan.items():
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(src, dest)
        if hashlib.sha256(dest.read_bytes()).hexdigest() != hashlib.sha256(src.read_bytes()).hexdigest():
            sys.exit(f"awa: verification failed for {dest}")
        print(f"awa: copied and verified {dest}")
    marker.write_text("Files under boot/ and EFI/BOOT/BOOTX64.EFI were installed by AWA (tools/awa.py usb).\n")
    print("awa: done. Boot the drive in UEFI mode (Secure Boot off).")


# ---------------------------------------------------------------- QEMU

def qemu_args(image: Path, firmware: str, workdir: Path, display: bool) -> list:
    qemu = qemu_binary()
    args = [
        qemu, "-machine", "q35", "-m", "256M", "-smp", "4", "-no-reboot",
        "-drive", f"format=raw,file={image}",
        # UEFI runs on QEMU's full-featured CPU (x2APIC, SMEP, SMAP, UMIP, RDRAND);
        # BIOS on the plain default CPU, so both code paths get tested.
        "-cpu", "max" if firmware == "uefi" else "qemu64",
    ]
    if firmware == "uefi":
        code, vars_template = firmware_files(qemu)
        vars_copy = workdir / "uefi-vars.fd"
        shutil.copyfile(vars_template, vars_copy)
        args += [
            "-drive", f"if=pflash,format=raw,readonly=on,file={code}",
            "-drive", f"if=pflash,format=raw,file={vars_copy}",
        ]
    if not display:
        args += ["-display", "none"]
    return args


def cmd_run(args) -> None:
    image = cmd_image()
    workdir = build_dir() / "run"
    workdir.mkdir(parents=True, exist_ok=True)
    firmware = "bios" if args.bios else "uefi"
    run(qemu_args(image, firmware, workdir, display=True) + ["-serial", "stdio"])


class Qmp:
    def __init__(self, port: int):
        deadline = time.time() + 30
        while True:
            try:
                self.sock = socket.create_connection(("127.0.0.1", port), timeout=30)
                break
            except OSError:
                if time.time() > deadline:
                    raise
                time.sleep(0.2)
        self.file = self.sock.makefile("rwb")
        self._read()  # greeting
        self.execute("qmp_capabilities")

    def _read(self) -> dict:
        while True:
            line = self.file.readline()
            if not line:
                raise ConnectionError("QMP connection closed")
            msg = json.loads(line)
            if "event" not in msg:
                return msg

    def execute(self, command: str, **arguments) -> dict:
        req = {"execute": command}
        if arguments:
            req["arguments"] = arguments
        self.file.write(json.dumps(req).encode() + b"\n")
        self.file.flush()
        reply = self._read()
        if "error" in reply:
            raise RuntimeError(f"QMP {command}: {reply['error']}")
        return reply


def free_port() -> int:
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def read_ppm(path: Path) -> tuple[int, int, bytes]:
    data = path.read_bytes()
    parts = []
    pos = 0
    while len(parts) < 4:
        while data[pos:pos + 1].isspace():
            pos += 1
        if data[pos:pos + 1] == b"#":
            pos = data.index(b"\n", pos) + 1
            continue
        end = pos
        while not data[end:end + 1].isspace():
            end += 1
        parts.append(data[pos:end])
        pos = end
    pos += 1
    if parts[0] != b"P6" or parts[3] != b"255":
        raise ValueError("unsupported PPM")
    w, h = int(parts[1]), int(parts[2])
    return w, h, data[pos:pos + w * h * 3]


def write_png(path: Path, w: int, h: int, rgb: bytes) -> None:
    raw = b"".join(b"\x00" + rgb[y * w * 3:(y + 1) * w * 3] for y in range(h))

    def chunk(kind: bytes, body: bytes) -> bytes:
        return struct.pack(">I", len(body)) + kind + body + struct.pack(">I", zlib.crc32(kind + body))

    png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(raw, 6)) + chunk(b"IEND", b"")
    path.write_bytes(png)


def check_framebuffer(w: int, h: int, rgb: bytes, update_golden: bool) -> list:
    """Returns a list of failure messages (empty = pass)."""
    def px(x, y):
        i = (y * w + x) * 3
        return tuple(rgb[i:i + 3])

    failures = []
    if px(2, 2) != BACKGROUND:
        failures.append(f"background at (2,2) is {px(2, 2)}, expected {BACKGROUND}")

    box_w = min(BANNER_BOX_W, w - BANNER_X)
    mask_rows = []
    text_count = 0
    for y in range(BANNER_Y, BANNER_Y + BANNER_BOX_H):
        row = []
        for x in range(BANNER_X, BANNER_X + box_w):
            p = px(x, y)
            if p == BACKGROUND:
                row.append(".")
            else:
                # The font is anti-aliased: text pixels are blends between
                # BACKGROUND and FOREGROUND, so count anything that isn't background.
                text_count += 1
                row.append("#")
        mask_rows.append("".join(row).rstrip("."))
    if text_count < 500:
        failures.append(f"banner area has only {text_count} text pixels (expected the boot message)")

    # Self-test results are drawn below the banner; the far corner stays empty.
    results = sum(1 for y in range(BANNER_Y + BANNER_BOX_H + 8, min(h, 600), 3) for x in range(BANNER_X, min(w, 900), 3) if px(x, y) != BACKGROUND)
    if results < 100:
        failures.append(f"no self-test results drawn below the banner ({results} text pixels)")
    corner = sum(1 for y in range(h - 40, h, 4) for x in range(w - 200, w, 4) if px(x, y) != BACKGROUND)
    if corner:
        failures.append(f"{corner} stray pixels in the bottom-right corner")

    mask = "\n".join(mask_rows) + "\n"
    if update_golden:
        GOLDEN.parent.mkdir(parents=True, exist_ok=True)
        GOLDEN.write_text(mask)
        print(f"awa: wrote golden banner mask {GOLDEN}")
    elif GOLDEN.is_file():
        if GOLDEN.read_text() != mask:
            failures.append(f"banner pixels differ from golden mask {GOLDEN}")
    else:
        failures.append(f"no golden mask at {GOLDEN} (review the screenshot, then run with --update-golden)")
    return failures


def check_selftests(serial_text: str) -> list:
    """Kernel self-tests print 'AWA TEST <name> PASS|FAIL ...' and a summary line."""
    failures = [f"self-test failed: {line.strip()}" for line in serial_text.splitlines() if line.startswith("AWA TEST ") and " FAIL " in f"{line} "]
    summary = [line for line in serial_text.splitlines() if line.startswith("AWA: self-test ")]
    if not summary:
        failures.append("kernel did not print a self-test summary")
    elif not summary[-1].rstrip().endswith(" 0 failed"):
        failures.append(f"self-test summary reports failures: {summary[-1].strip()}")
    return failures


def boot_test(image: Path, firmware: str, timeout: float, update_golden: bool) -> bool:
    workdir = build_dir() / "test" / firmware
    workdir.mkdir(parents=True, exist_ok=True)
    serial_log = workdir / "serial.log"
    serial_log.write_bytes(b"")
    port = free_port()
    args = qemu_args(image, firmware, workdir, display=False) + [
        "-serial", f"file:{serial_log.name}",
        "-qmp", f"tcp:127.0.0.1:{port},server,wait=off",
    ]
    print(f"\n=== boot test: {firmware.upper()} ===", flush=True)
    print("+", " ".join(str(a) for a in args), flush=True)
    proc = subprocess.Popen([str(a) for a in args], cwd=workdir)
    start = time.time()
    ok = False
    failures = []
    try:
        while True:
            text = serial_log.read_text(errors="replace")
            if PANIC_MARKER in text:
                failures.append("kernel panicked")
                break
            if BOOT_MESSAGE in text:
                ok = True
                break
            if proc.poll() is not None:
                failures.append(f"QEMU exited early with code {proc.returncode}")
                break
            if time.time() - start > timeout:
                failures.append(f"timed out after {timeout:.0f} s waiting for '{BOOT_MESSAGE}'")
                break
            time.sleep(0.5)
        elapsed = time.time() - start

        if ok:
            qmp = Qmp(port)
            ppm = workdir / "screen.ppm"
            qmp.execute("screendump", filename=str(ppm.resolve()))
            for _ in range(50):
                if ppm.is_file() and ppm.stat().st_size > 0:
                    break
                time.sleep(0.1)
            qmp.execute("quit")
            w, h, rgb = read_ppm(ppm)
            png = workdir / "screen.png"
            write_png(png, w, h, rgb)
            print(f"awa: screenshot {png} ({w}x{h})")
            failures += check_framebuffer(w, h, rgb, update_golden)
    finally:
        if proc.poll() is None:
            try:
                proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                proc.kill()

    serial_text = serial_log.read_text(errors="replace")
    if ok:
        failures += check_selftests(serial_text)
    print("--- serial output ---")
    print(serial_text.rstrip())
    print("---------------------")
    if failures:
        for f in failures:
            print(f"FAIL [{firmware}]: {f}")
        return False
    print(f"PASS [{firmware}]: '{BOOT_MESSAGE}' on serial after {elapsed:.1f} s; framebuffer verified")
    return True


def cmd_test(args) -> None:
    image = cmd_image()
    firmwares = args.firmware or ["uefi", "bios"]
    results = {fw: boot_test(image, fw, args.timeout, args.update_golden) for fw in firmwares}
    print("\n=== summary ===")
    for fw, ok in results.items():
        print(f"{fw:5} {'PASS' if ok else 'FAIL'}")
    sys.exit(0 if all(results.values()) else 1)


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)
    sub.add_parser("build").set_defaults(func=cmd_build)
    sub.add_parser("image").set_defaults(func=cmd_image)
    r = sub.add_parser("run")
    r.add_argument("--bios", action="store_true", help="boot with legacy BIOS instead of UEFI")
    r.set_defaults(func=cmd_run)
    u = sub.add_parser("usb", help="copy AWA onto a FAT32 USB drive for UEFI boot (keeps existing files)")
    u.add_argument("drive", help="drive root, e.g. D:")
    u.set_defaults(func=cmd_usb)
    t = sub.add_parser("test")
    t.add_argument("--firmware", action="append", choices=["uefi", "bios"])
    t.add_argument("--timeout", type=float, default=300.0, help="seconds to wait for boot (QEMU without KVM is slow)")
    t.add_argument("--update-golden", action="store_true", help="record the current banner as the expected one")
    t.set_defaults(func=cmd_test)
    args = p.parse_args()
    try:
        args.func(args)
    except subprocess.CalledProcessError as e:
        sys.exit(f"awa: command failed with exit code {e.returncode}")


if __name__ == "__main__":
    main()
