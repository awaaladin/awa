//! init: the first user program. The kernel starts it with one bootstrap
//! message: the names of the other boot files and a VMO handle for each.
//!
//! Today init runs the Phase 4 self-tests and reports them to the kernel:
//! in-process checks of handles, rights and user-pointer validation, then it
//! launches ping and pong (which exchange messages and a VMO handle), a
//! program that crashes on purpose, and one that tries to use a handle it
//! was never given.

#![no_std]
#![no_main]

use awa::abi::{map, rights, Status, EXIT_FAULT};
use awa::{channel, fmt_line, loader, println, process, test_report, vmo, Startup};

const MAX_FILES: usize = 16;

struct Files {
    names: [u8; 512],
    names_len: usize,
    handles: [u32; MAX_FILES],
    count: usize,
}

impl Files {
    fn find(&self, name: &str) -> Option<u32> {
        let text = core::str::from_utf8(&self.names[..self.names_len]).ok()?;
        text.split('\n').filter(|n| !n.is_empty()).position(|n| n == name).and_then(|i| self.handles.get(i).copied())
    }
}

fn report(name: &str, ok: bool, detail: core::fmt::Arguments) {
    let mut w = awa::LineWriter::new();
    let _ = core::fmt::Write::write_fmt(&mut w, detail);
    test_report(name, ok, w.as_str());
}


#[unsafe(no_mangle)]
fn awa_main(start: Startup) -> i64 {
    println!("init: running in user mode (ring 3)");

    // ---- the bootstrap message from the kernel
    let mut files = Files { names: [0; 512], names_len: 0, handles: [0; MAX_FILES], count: 0 };
    match channel::read(start.bootstrap, &mut files.names, &mut files.handles) {
        Ok((n, h)) => {
            files.names_len = n;
            files.count = h;
            report("user-mode-entry", true, format_args!("init runs in ring 3 and received {} boot files from the kernel", h));
        }
        Err(e) => {
            report("user-mode-entry", false, format_args!("bootstrap read failed: {:?}", e));
            return 1;
        }
    }

    in_process_checks(&files);
    cross_process_checks(&files);
    0
}

fn in_process_checks(files: &Files) {
    // Rights: a read-only copy of a channel end cannot write; rights never grow.
    let (a, _b) = channel::create().unwrap_or((0, 0));
    let ro = awa::duplicate(a, rights::READ);
    let write_ro = ro.map(|h| channel::write(h, b"x", &[]));
    let grow = ro.map(|h| awa::duplicate(h, rights::ALL));
    report(
        "rights-enforced",
        matches!(write_ro, Ok(Err(Status::AccessDenied))) && matches!(grow, Ok(Err(Status::AccessDenied))),
        format_args!("write through a read-only handle -> {:?}; widening rights -> {:?}", write_ro, grow),
    );

    // Unknown handles are refused.
    let bogus = vmo::size(0x00ab_cdef);
    report("bad-handle-refused", bogus == Err(Status::BadHandle), format_args!("made-up handle -> {:?}", bogus));

    // User pointers into the kernel are refused (no kernel memory read or written).
    let r = awa::sys(awa::abi::Call::DebugWrite, [0xffff_ffff_8000_0000, 16, 0, 0, 0, 0]);
    report(
        "kernel-pointer-refused",
        r == -(Status::InvalidArgs as i64),
        format_args!("debug_write(kernel address) -> {}", r),
    );

    // W^X: a mapping cannot be writable and executable.
    let wx = match (process::create(), vmo::create(4096)) {
        (Ok(p), Ok(v)) => process::map(p, v, 0, 0x10_0000, 4096, map::READ | map::WRITE | map::EXECUTE),
        _ => Err(Status::NoMemory),
    };
    report("wx-mapping-refused", wx == Err(Status::InvalidArgs), format_args!("map(write+execute) -> {:?}", wx));

    // A raw `syscall` instruction outside the vDSO is refused.
    let raw: i64;
    // SAFETY: a bare syscall; the kernel refuses it and returns an error in rax.
    // rcx and r11 are clobbered by the instruction.
    unsafe { core::arch::asm!("syscall", inlateout("rax") 0x7777u64 as i64 => raw, out("rcx") _, out("r11") _, options(nostack)) };
    report("syscall-outside-vdso-refused", raw == -(Status::NotFromVdso as i64), format_args!("raw syscall from program code -> {}", raw));

    // Messages and handles move through a channel; the handle leaves the sender.
    let ok = (|| -> awa::Result<bool> {
        let (x, y) = channel::create()?;
        let v = vmo::create(4096)?;
        vmo::write(v, 0, b"carried by a channel")?;
        channel::write(x, b"here", &[v])?;
        let gone = vmo::size(v) == Err(Status::BadHandle);
        let mut buf = [0u8; 16];
        let mut hs = [0u32; 4];
        let (n, h) = channel::read(y, &mut buf, &mut hs)?;
        let mut text = [0u8; 20];
        vmo::read(hs[0], 0, &mut text)?;
        Ok(gone && &buf[..n] == b"here" && h == 1 && &text == b"carried by a channel")
    })();
    report("channel-handle-transfer", ok == Ok(true), format_args!("message + VMO handle through a channel -> {:?}", ok));
    let _ = files;
}

fn cross_process_checks(files: &Files) {
    // ping and pong: two processes that exchange messages and a VMO handle.
    let result = (|| -> awa::Result<(u64, u64, [u8; 128], usize)> {
        let ping_file = files.find("ping").ok_or(Status::BadHandle)?;
        let pong_file = files.find("pong").ok_or(Status::BadHandle)?;
        let (x, y) = channel::create()?; // ping <-> pong
        let (to_ping, ping_side) = channel::create()?; // init <-> ping
        let pong = loader::spawn(pong_file, y)?;
        let ping = loader::spawn(ping_file, ping_side)?;
        channel::write(to_ping, b"talk to pong", &[x])?; // hand ping its end
        let mut buf = [0u8; 128];
        let (n, _) = channel::read(to_ping, &mut buf, &mut [])?;
        let ping_status = process::wait(ping)?;
        let pong_status = process::wait(pong)?;
        Ok((ping_status, pong_status, buf, n))
    })();
    match result {
        Ok((ps, qs, buf, n)) => {
            let text = core::str::from_utf8(&buf[..n]).unwrap_or("?");
            let handoff = text.contains("handoff=ok");
            report("two-processes-exchange", ps == 0 && qs == 0 && handoff, format_args!("ping exit {}, pong exit {}; ping says: {}", ps, qs, text));
            let rtt = text.split("rtt_ns=").nth(1).and_then(|t| t.split(' ').next()).and_then(|t| t.parse::<u64>().ok());
            let split = text.split("split_rtt_ns=").nth(1).and_then(|t| t.split(' ').next()).and_then(|t| t.parse::<u64>().ok());
            report("ipc-roundtrip", rtt.is_some(), format_args!("{} ns per channel_call round trip between two processes ({} ns as separate write + read)", rtt.unwrap_or(0), split.unwrap_or(0)));
        }
        Err(e) => report("two-processes-exchange", false, format_args!("could not run ping/pong: {:?}", e)),
    }

    // A process that faults is ended and reported; the kernel and init go on.
    let crash = files.find("crashme").ok_or(Status::BadHandle).and_then(|f| loader::spawn(f, 0)).and_then(process::wait);
    report(
        "process-fault-isolated",
        matches!(crash, Ok(s) if s & EXIT_FAULT != 0 && s & 0xff == 14),
        format_args!("crashme reading kernel memory -> status {} (page fault = 14)", match crash { Ok(s) => fmt_line!("{:#x}", s), Err(e) => fmt_line!("{:?}", e) }.as_str()),
    );

    // A process cannot use a handle that belongs to another process.
    let result = (|| -> awa::Result<u64> {
        let file = files.find("noperm").ok_or(Status::BadHandle)?;
        let secret = vmo::create(4096)?; // a handle only init holds
        let (ours, theirs) = channel::create()?;
        let p = loader::spawn(file, theirs)?;
        channel::write(ours, &secret.to_le_bytes(), &[])?; // just the number, not the handle
        process::wait(p)
    })();
    report("no-handle-no-access", result == Ok(0), format_args!("noperm using init's handle number -> exit {:?} (0 = refused as expected)", result));

    // Fuzzing: tens of thousands of random system calls. Passing means the
    // kernel answered every one and is still running.
    // fuzz may crash itself (random pointers can hit its own stack); what
    // matters is that it ended and the kernel, and init, are still running.
    let result = (|| -> awa::Result<(u64, [u8; 96], usize)> {
        let file = files.find("fuzz").ok_or(Status::BadHandle)?;
        let (ours, theirs) = channel::create()?;
        let p = loader::spawn(file, theirs)?;
        let mut buf = [0u8; 96];
        let n = channel::read(ours, &mut buf, &mut []).map(|(n, _)| n).unwrap_or(0);
        Ok((process::wait(p)?, buf, n))
    })();
    match result {
        Ok((status, buf, n)) => {
            let summary = if n > 0 { core::str::from_utf8(&buf[..n]).unwrap_or("?") } else { "fuzz ended before reporting (it damaged its own memory)" };
            report("syscall-fuzz", true, format_args!("kernel survived random syscalls: {}; fuzz status {:#x}", summary, status));
        }
        Err(e) => report("syscall-fuzz", false, format_args!("fuzz did not run: {:?}", e)),
    }
}
