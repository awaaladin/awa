//! Boot self-tests. Each prints `AWA TEST <name> PASS|FAIL <detail>` on the
//! serial port; `tools/awa.py test` fails the boot test if any line says FAIL
//! or the summary is missing. They run on every boot (they are fast), so real
//! hardware is checked too.

use crate::arch::{self, probe};
use crate::mem::{self, frames, paging};
use crate::serial_println;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::sync::atomic::Ordering;

const PF_PRESENT: u64 = 1 << 0;
const PF_WRITE: u64 = 1 << 1;
const PF_FETCH: u64 = 1 << 4;

/// Memory protection and allocators: W^X, NX, guard pages, frames, heap.
pub fn memory(r: &mut Report) {
    // W^X: kernel code must not be writable, even by the kernel.
    let text = paging::kernel_text_start();
    let got = probe::write_same(text);
    let ok = matches!(got, probe::Outcome::Faulted { code, .. } if code & (PF_PRESENT | PF_WRITE) == PF_PRESENT | PF_WRITE);
    r.check("wx-text-not-writable", ok, format_args!("write to kernel code {:#x} -> {:?}", text, got));

    let rodata = paging::kernel_rodata_start();
    let got = probe::write_same(rodata);
    let ok = matches!(got, probe::Outcome::Faulted { code, .. } if code & PF_WRITE != 0);
    r.check("rodata-not-writable", ok, format_args!("write to read-only data {:#x} -> {:?}", rodata, got));

    // NX: data on the heap must never run as code.
    let ret = Box::new([0xC3u8; 64]); // 0xC3 = `ret`, so a wrongly allowed call just returns
    let addr = ret.as_ptr() as u64;
    // SAFETY: the target holds `ret` bytes, as `probe::exec` requires.
    let got = unsafe { probe::exec(addr) };
    let ok = matches!(got, probe::Outcome::Faulted { code, .. } if code & PF_FETCH != 0);
    r.check("nx-heap-not-executable", ok, format_args!("call into heap {:#x} -> {:?}", addr, got));
    drop(ret);

    // NX on the direct map (all of RAM).
    match frames::alloc() {
        Some(f) => {
            let v = mem::phys_to_virt(f);
            // SAFETY: `f` is a frame we own, mapped read-write in the direct map.
            unsafe { core::ptr::write_bytes(v as *mut u8, 0xC3, 16) };
            // SAFETY: the target holds `ret` bytes.
            let got = unsafe { probe::exec(v) };
            frames::free(f);
            let ok = matches!(got, probe::Outcome::Faulted { code, .. } if code & PF_FETCH != 0);
            r.check("nx-directmap-not-executable", ok, format_args!("call into RAM {:#x} -> {:?}", v, got));
        }
        None => r.check("nx-directmap-not-executable", false, format_args!("no frame to test with")),
    }

    // Guard page: the page right below the kernel stack is never mapped.
    let guard = crate::KERNEL_STACK_BOTTOM.load(Ordering::Relaxed) - 8;
    let got = probe::read(guard);
    let ok = matches!(got, probe::Outcome::Faulted { code, .. } if code & PF_PRESENT == 0);
    r.check("stack-guard-page", ok, format_args!("read below the stack {:#x} -> {:?}", guard, got));

    // Frame allocator: many frames, all distinct and aligned, all returned.
    let before = frames::stats();
    let mut taken: Vec<u64> = Vec::with_capacity(4096);
    while taken.len() < 4096 {
        match frames::alloc() {
            Some(f) => taken.push(f),
            None => break,
        }
    }
    let count = taken.len();
    let aligned = taken.iter().all(|f| f % mem::PAGE_SIZE == 0 && *f >= 1 << 20);
    taken.sort_unstable();
    taken.dedup();
    let distinct = taken.len() == count;
    for f in taken {
        frames::free(f);
    }
    let after = frames::stats();
    r.check(
        "frames-stress",
        count == 4096 && aligned && distinct && after.free == before.free,
        format_args!("{} frames: aligned {}, distinct {}, free {} -> {}", count, aligned, distinct, before.free, after.free),
    );

    // Heap: mixed sizes, patterns verified, everything returned.
    let start = mem::heap::usage().used;
    let mut blocks: Vec<Vec<u8>> = Vec::new();
    for i in 0..300usize {
        let len = 16 + (i * 97) % 4000;
        blocks.push(alloc::vec![(i % 251) as u8; len]);
    }
    let intact = blocks.iter().enumerate().all(|(i, b)| b.iter().all(|&x| x == (i % 251) as u8));
    blocks.retain(|b| b.len() % 2 == 0); // free about half, fragmenting the heap
    let big: Vec<u64> = alloc::vec![0xA5A5_A5A5_A5A5_A5A5; 128 * 1024]; // 1 MiB
    let big_ok = big.iter().all(|&x| x == 0xA5A5_A5A5_A5A5_A5A5);
    drop(big);
    drop(blocks);
    let end = mem::heap::usage().used;
    r.check("heap-stress", intact && big_ok && end == start, format_args!("300 mixed blocks + 1 MiB; used bytes {} -> {}", start, end));
}

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
