//! fuzz: throws random and malformed system calls at the kernel.
//!
//! Every call gets random arguments drawn from values that tend to break
//! kernels: zero, all ones, kernel addresses, addresses at the user/kernel
//! boundary, unaligned and huge sizes, real and made-up handles. The kernel
//! must answer each one with a result or an error code and never crash.
//!
//! Skipped on purpose: calls that end this process (ProcessExit), calls that
//! can wait forever by design (ChannelRead/ChannelCall on a channel only we
//! hold, ProcessWait on a process that never starts),
//! Random pointers may land on fuzz's own memory and crash it: that is the
//! program hurting itself, not a kernel failure; init judges only whether
//! the kernel survived. ProcessStart (each would
//! leave a crashing child behind) and DebugWrite (it would spray the log).

#![no_std]
#![no_main]

use awa::abi::{Call, CALL_COUNT, USER_TOP};
use awa::{channel, clock_ns, fmt_line, process, random, sys, vmo, Startup};

const ITERATIONS: u64 = 30_000;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
}

static mut BUF: [u8; 8192] = [0; 8192];

#[unsafe(no_mangle)]
fn awa_main(start: Startup) -> i64 {
    let mut rng = Rng(random() | 1);
    let buf = core::ptr::addr_of_mut!(BUF) as u64;

    // A few real handles, so the fuzzer also reaches deep paths.
    let mut handles = [0u64; 8];
    if let Ok((a, b)) = channel::create() {
        handles[0] = u64::from(a);
        handles[1] = u64::from(b);
    }
    if let Ok(v) = vmo::create(16 * 4096) {
        handles[2] = u64::from(v);
    }
    if let Ok(p) = process::create() {
        handles[3] = u64::from(p);
    }

    let pick = |rng: &mut Rng, handles: &[u64; 8]| -> u64 {
        match rng.next() % 16 {
            0 => 0,
            1 => u64::MAX,
            2 => 1,
            3 => USER_TOP - 1,
            4 => USER_TOP,
            5 => 0xffff_8000_0000_0000,
            6 => 0xffff_ffff_8000_1000,
            7 => 0x0000_8000_0000_0000,
            8 => buf + rng.next() % 8192,
            9 => buf,
            10 => rng.next() % 0x1_0000,
            11 => handles[(rng.next() % 8) as usize],
            12 => rng.next() & 0xffff_ffff,
            13 => 4096 * (rng.next() % 64),
            14 => rng.next() % USER_TOP,
            _ => rng.next(),
        }
    };

    let t0 = clock_ns();
    let mut calls = 0u64;
    let mut errors = 0u64;
    for i in 0..ITERATIONS {
        let slot = rng.next() % (CALL_COUNT + 2);
        let skip = [Call::DebugWrite as u64, Call::ProcessExit as u64, Call::ChannelRead as u64, Call::ChannelCall as u64, Call::ProcessWait as u64, Call::ProcessStart as u64];
        if skip.contains(&slot) {
            continue;
        }
        let args = [pick(&mut rng, &handles), pick(&mut rng, &handles), pick(&mut rng, &handles), pick(&mut rng, &handles), pick(&mut rng, &handles), pick(&mut rng, &handles)];
        let r = if slot >= CALL_COUNT {
            // Slots past the end and raw syscalls with random numbers.
            let raw: i64;
            // SAFETY: a bare syscall; the kernel refuses it (not from the vDSO).
            unsafe { core::arch::asm!("syscall", inlateout("rax") rng.next() as i64 => raw, in("rdi") args[0], in("rsi") args[1], in("rdx") args[2], out("rcx") _, out("r11") _, options(nostack)) };
            raw
        } else {
            // SAFETY of the call itself is the kernel's job: that is what we test.
            let call: Call = unsafe { core::mem::transmute(slot) };
            sys(call, args)
        };
        calls += 1;
        if r < 0 {
            errors += 1;
        }
        // Keep the handle set fresh: new handles returned by successful calls.
        if r > 0 && r < 0xffff_ffff {
            handles[(4 + i % 4) as usize] = r as u64;
        }
        // Read/write calls on our two channel ends can fill a queue; drain one
        // message now and then so writes keep exercising the full path.
        if i % 64 == 0 {
            let _ = awa::close(handles[(4 + rng.next() % 4) as usize] as u32);
        }
    }
    let ms = (clock_ns() - t0) / 1_000_000;
    let report = fmt_line!("calls={} errors={} ms={}", calls, errors, ms);
    let _ = channel::write(start.bootstrap, report.as_str().as_bytes(), &[]);
    0
}
