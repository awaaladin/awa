# 08 · Compatibility layer plan

Status: draft · Not implemented

## 1. Strategy (owner decision, 2026-10-22)

AWA builds its **own native APIs first**. Software from other platforms comes in stages:

1. Rust programs, by porting Rust's `std` to AWA
2. C/C++ programs through a POSIX libc, recompiled for AWA
3. Python and the ML/CV stack, recompiled for AWA
4. *Later, optional:* a **from-scratch Linux-application compatibility layer**, so
   prebuilt Linux binaries (pip wheels, Blender releases) run unmodified

Until stage 4 exists, the brief's goal "existing software runs unmodified" is **not
met**: every program must be rebuilt for AWA. This is an accepted consequence of the
owner's choice.

## 2. Stages and exit criteria

| Stage | What | Exit criterion |
|-------|------|----------------|
| C1 | **Rust `std` for `x86_64-unknown-awa`** on libawa (threads, time, fs via VFS protocol, net via sockets protocol) | Rust's own `std` test suite passes the agreed subset in QEMU |
| C2 | **libc** (proposal A6: relibc with an AWA platform backend), pthreads, signals (emulated through an exception/notification channel), `mmap` (VMOs, with the class chosen via `madvise` extensions), sockets, `poll`/`epoll`-like API | `libc-test` subset + a POSIX conformance subset; `busybox`-class tools build and run |
| C3 | **C/C++ toolchain**: clang/LLVM target `x86_64-unknown-awa`, compiler-rt, libc++/libc++abi, libunwind (all Apache-2.0 with LLVM exception) | Hello-world C and C++ with exceptions and threads; a CMake project cross-builds |
| C4 | **CPython** | CPython test suite subset passes; `pip install` of pure-Python packages |
| C5 | **NumPy, OpenCV (CPU), image/IO libs** (libpng, zlib, OpenEXR, OpenImageIO) | NumPy tests subset; an OpenCV pipeline benchmark runs |
| C6 | **PyTorch CPU** | Train a small CNN end to end; ATen tests subset |
| C7 | **3D formats & tools**: glTF (tinygltf/cgltf), OpenUSD, OpenEXR; Blender-class apps after Vulkan exists ([06 §7](06-gpu.md)) | Load and render a USD and glTF scene in a sample viewer |
| C8 | **Linux-app compatibility layer** (optional) | Prebuilt CPython and a static busybox from a Linux distro run unmodified |

## 3. How existing programs benefit automatically

Even recompiled-but-unmodified programs gain from AWA without code changes:

- File mappings are Streamable, so large models opened with `mmap` load lazily and share
  pages across processes.
- Focused windows get frame-deadline scheduling through the window server.
- Per-app quotas, tracing and the profiler work for everything.
- `madvise(MADV_FREE)`-style hints are mapped to the Recomputable class.

Native libawa APIs (memory classes, budget levels, reservations, gangs) unlock the rest.

## 4. Linux-app layer design notes (stage C8, not scheduled)

- A **user-space personality service**: Linux system calls from a marked process trap to
  a per-process handler (via a dedicated syscall-entry mode for that process) and are
  translated into AWA capability operations. It is written from public documentation
  (man pages, the ELF and x86-64 System V ABI specs). **No Linux code.**
- Linux binaries run in a namespace built for them (`/proc`, `/dev` emulated), always
  inside a box by default.
- Precedent: WSL1, FreeBSD's Linuxulator, and Fuchsia's Starnix show this is feasible
  and very large. Estimate: comparable in effort to the POSIX libc stage.

## 5. Risks

- PyTorch, OpenCV and Blender ports require porting their build systems and
  dependencies, and each is large. They depend on C1–C3 being solid.
- GPU-accelerated ML depends on Vulkan compute or ROCm, which depends on the A5 decision.
