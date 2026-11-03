//! Boot self-tests. Each prints `AWA TEST <name> PASS|FAIL <detail>` on the
//! serial port; `tools/awa.py test` fails the boot test if any line says FAIL
//! or the summary is missing. They run on every boot (they are fast), so real
//! hardware is checked too.

use crate::arch::{self, probe};
use crate::serial_println;
use core::sync::atomic::Ordering;

pub struct Report {
    passed: u32,
    failed: u32,
}

impl Report {
    pub fn new() -> Self {
        Self { passed: 0, failed: 0 }
    }

    pub fn check(&mut self, name: &str, ok: bool, detail: core::fmt::Arguments) {
        if ok {
            self.passed += 1;
        } else {
            self.failed += 1;
        }
        serial_println!("AWA TEST {} {} {}", name, if ok { "PASS" } else { "FAIL" }, detail);
    }

    pub fn finish(self) {
        serial_println!("AWA: self-test {} passed, {} failed", self.passed, self.failed);
    }
}

/// CPU tables: exceptions reach their handlers and the kernel survives them.
pub fn cpu(r: &mut Report) {
    let before = arch::idt::BREAKPOINTS.load(Ordering::Relaxed);
    arch::breakpoint();
    let after = arch::idt::BREAKPOINTS.load(Ordering::Relaxed);
    r.check("idt-breakpoint", after == before + 1, format_args!("int3 handled and returned"));

    // An address nothing maps: the page-fault handler must catch the read.
    const UNMAPPED: u64 = 0xffff_fe00_0000_0000;
    let got = probe::read(UNMAPPED);
    let ok = matches!(got, probe::Outcome::Faulted { addr, code } if addr == UNMAPPED && code & 1 == 0);
    r.check("page-fault-caught", ok, format_args!("read of {:#x} -> {:?}", UNMAPPED, got));
}
