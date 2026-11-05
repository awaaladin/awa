//! AWA's native user-space library.
//!
//! Every AWA program links this. It provides the program entry (`_start`),
//! calls into the kernel through the vDSO (never raw `syscall`), and small
//! safe wrappers over handles, channels, memory objects and processes.
//!
//! A program defines:
//! ```ignore
//! #[unsafe(no_mangle)]
//! fn awa_main(start: awa::Startup) -> i64 { ... }
//! ```

#![no_std]

pub use awa_abi as abi;
pub use awa_elf as elf;

use abi::{Call, Status, VDSO_SLOT};
use core::fmt::{self, Write};
use core::sync::atomic::{AtomicU64, Ordering};

pub mod loader;

static VDSO: AtomicU64 = AtomicU64::new(0);

/// What the kernel hands a program at start.
#[derive(Clone, Copy)]
pub struct Startup {
    /// Address of this process's vDSO.
    pub vdso: u64,
    /// Handle passed by whoever started the program (0 if none).
    pub bootstrap: u32,
}

unsafe extern "Rust" {
    fn awa_main(start: Startup) -> i64;
}

#[unsafe(no_mangle)]
extern "C" fn _start(vdso: u64, bootstrap: u64, _arg: u64) -> ! {
    VDSO.store(vdso, Ordering::Relaxed);
    // SAFETY: every AWA program defines `awa_main` (see the crate docs).
    let code = unsafe { awa_main(Startup { vdso, bootstrap: bootstrap as u32 }) };
    exit(code as u64)
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    println!("panic: {}", info);
    exit(101)
}

pub type Result<T> = core::result::Result<T, Status>;

/// Calls vDSO slot `call` with six arguments.
pub fn sys(call: Call, a: [u64; 6]) -> i64 {
    let addr = VDSO.load(Ordering::Relaxed) + call as u64 * VDSO_SLOT;
    // SAFETY: the kernel mapped the vDSO at `VDSO` and every slot is a
    // sysv64 function taking six u64 and returning i64 (awa-abi docs).
    let f: extern "sysv64" fn(u64, u64, u64, u64, u64, u64) -> i64 = unsafe { core::mem::transmute(addr as usize) };
    f(a[0], a[1], a[2], a[3], a[4], a[5])
}

fn check(r: i64) -> Result<u64> {
    if r >= 0 { Ok(r as u64) } else { Err(Status::from_code(r).unwrap_or(Status::NotSupported)) }
}

pub fn debug_write(s: &str) {
    sys(Call::DebugWrite, [s.as_ptr() as u64, s.len() as u64, 0, 0, 0, 0]);
}

pub fn exit(code: u64) -> ! {
    sys(Call::ProcessExit, [code, 0, 0, 0, 0, 0]);
    // The kernel never returns from ProcessExit.
    loop {
        core::hint::spin_loop();
    }
}

pub fn clock_ns() -> u64 {
    sys(Call::ClockGet, [0; 6]) as u64
}

pub fn random() -> u64 {
    sys(Call::RandomGet, [0; 6]) as u64
}

pub fn close(handle: u32) -> Result<()> {
    check(sys(Call::HandleClose, [u64::from(handle), 0, 0, 0, 0, 0])).map(|_| ())
}

pub fn duplicate(handle: u32, rights: u32) -> Result<u32> {
    check(sys(Call::HandleDuplicate, [u64::from(handle), u64::from(rights), 0, 0, 0, 0])).map(|v| v as u32)
}

/// Reports a boot self-test result (only init may).
pub fn test_report(name: &str, ok: bool, detail: &str) {
    sys(Call::TestReport, [name.as_ptr() as u64, name.len() as u64, ok as u64, detail.as_ptr() as u64, detail.len() as u64, 0]);
}

pub mod channel {
    use super::*;

    pub fn create() -> Result<(u32, u32)> {
        let mut out = [0u32; 2];
        check(sys(Call::ChannelCreate, [out.as_mut_ptr() as u64, 0, 0, 0, 0, 0]))?;
        Ok((out[0], out[1]))
    }

    /// Sends bytes and moves `handles` (they leave this process).
    pub fn write(h: u32, bytes: &[u8], handles: &[u32]) -> Result<()> {
        check(sys(Call::ChannelWrite, [u64::from(h), bytes.as_ptr() as u64, bytes.len() as u64, handles.as_ptr() as u64, handles.len() as u64, 0])).map(|_| ())
    }

    /// Waits for a message. Returns (bytes received, handles received).
    pub fn read(h: u32, buf: &mut [u8], handles: &mut [u32]) -> Result<(usize, usize)> {
        let r = check(sys(Call::ChannelRead, [u64::from(h), buf.as_mut_ptr() as u64, buf.len() as u64, handles.as_mut_ptr() as u64, handles.len() as u64, 0]))?;
        Ok(((r & 0xffff_ffff) as usize, (r >> 32) as usize))
    }
}

pub mod vmo {
    use super::*;

    pub fn create(size: u64) -> Result<u32> {
        check(sys(Call::VmoCreate, [size, 0, 0, 0, 0, 0])).map(|v| v as u32)
    }

    pub fn size(h: u32) -> Result<u64> {
        check(sys(Call::VmoSize, [u64::from(h), 0, 0, 0, 0, 0]))
    }

    pub fn write(h: u32, offset: u64, data: &[u8]) -> Result<()> {
        check(sys(Call::VmoWrite, [u64::from(h), offset, data.as_ptr() as u64, data.len() as u64, 0, 0])).map(|_| ())
    }

    pub fn read(h: u32, offset: u64, out: &mut [u8]) -> Result<()> {
        check(sys(Call::VmoRead, [u64::from(h), offset, out.as_mut_ptr() as u64, out.len() as u64, 0, 0])).map(|_| ())
    }
}

pub mod process {
    use super::*;

    pub fn create() -> Result<u32> {
        check(sys(Call::ProcessCreate, [0; 6])).map(|v| v as u32)
    }

    pub fn map(p: u32, vmo: u32, offset: u64, vaddr: u64, len: u64, flags: u64) -> Result<()> {
        check(sys(Call::ProcessMap, [u64::from(p), u64::from(vmo), offset, vaddr, len, flags])).map(|_| ())
    }

    /// Starts the process; `handle` (0 for none) moves into it as its bootstrap handle.
    pub fn start(p: u32, entry: u64, stack_top: u64, handle: u32) -> Result<()> {
        check(sys(Call::ProcessStart, [u64::from(p), entry, stack_top, u64::from(handle), 0, 0])).map(|_| ())
    }

    /// Waits for the process to end; returns its status (see abi::EXIT_FAULT).
    pub fn wait(p: u32) -> Result<u64> {
        check(sys(Call::ProcessWait, [u64::from(p), 0, 0, 0, 0, 0]))
    }
}

// ---------------------------------------------------------------- printing

/// Formats into a fixed buffer (no heap) and writes it to the kernel log.
pub struct LineWriter {
    buf: [u8; 512],
    len: usize,
}

impl LineWriter {
    pub const fn new() -> Self {
        Self { buf: [0; 512], len: 0 }
    }

    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.buf[..self.len]).unwrap_or("?")
    }

    pub fn flush(&mut self) {
        debug_write(self.as_str());
        self.len = 0;
    }
}

impl Default for LineWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl Write for LineWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for &b in s.as_bytes() {
            if self.len == self.buf.len() {
                self.flush();
            }
            self.buf[self.len] = b;
            self.len += 1;
        }
        Ok(())
    }
}

#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => {{
        let mut w = $crate::LineWriter::new();
        let _ = core::fmt::Write::write_fmt(&mut w, format_args!("{}\n", format_args!($($arg)*)));
        w.flush();
    }};
}

/// `format!` into a fixed buffer, for test details.
#[macro_export]
macro_rules! fmt_line {
    ($($arg:tt)*) => {{
        let mut w = $crate::LineWriter::new();
        let _ = core::fmt::Write::write_fmt(&mut w, format_args!($($arg)*));
        w
    }};
}
