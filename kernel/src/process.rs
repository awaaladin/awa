//! Processes: an address space, a handle table, and an exit status.
//!
//! Every process gets the vDSO (the only place `syscall` is accepted from)
//! mapped read-only and executable at a random address. The kernel loads
//! exactly one program itself, `init`; everything else is loaded by init
//! through system calls.
//!
//! TODO(awa): free page tables and kernel stacks when a process ends;
//! multiple threads per process; a capability for creating processes.

use crate::mem::{frames, paging, phys_to_virt, PAGE_SIZE};
use crate::object::{Endpoint, HandleTable, Message, Object, Vmo, CHANNEL_RIGHTS, VMO_RIGHTS};
use crate::sched::{self, WaitQueue};
use crate::sync::SpinLock;
use crate::{rng, serial_println};
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use awa_abi::{map, Status, CALL_COUNT, EXIT_FAULT, USER_TOP, VDSO_SLOT};
use core::sync::atomic::{AtomicU64, Ordering};

// ---------------------------------------------------------------- vDSO

/// Private syscall numbers are SYSCALL_BASE + slot; the base changes every boot.
pub static SYSCALL_BASE: AtomicU64 = AtomicU64::new(0);
static VDSO_FRAME: AtomicU64 = AtomicU64::new(0);

/// Builds the vDSO page: slot n = `mov r10, rcx; mov eax, BASE+n; syscall; ret`.
pub fn init_vdso() -> Result<(), &'static str> {
    let base = 0x100 + (rng::next_u64() & 0xff_ff00) as u32;
    SYSCALL_BASE.store(u64::from(base), Ordering::Relaxed);
    let frame = frames::alloc_zeroed().ok_or("out of memory for the vDSO")?;
    let page = phys_to_virt(frame) as *mut u8;
    for slot in 0..CALL_COUNT {
        let n = base + slot as u32;
        let mut code = [0xCCu8; VDSO_SLOT as usize]; // int3 padding
        code[..3].copy_from_slice(&[0x49, 0x89, 0xCA]); // mov r10, rcx
        code[3] = 0xB8; // mov eax, imm32
        code[4..8].copy_from_slice(&n.to_le_bytes());
        code[8..10].copy_from_slice(&[0x0F, 0x05]); // syscall
        code[10] = 0xC3; // ret
        // SAFETY: the frame is ours, mapped by the direct map, and the slot
        // lies inside it (CALL_COUNT * VDSO_SLOT <= 4096).
        unsafe { core::ptr::copy_nonoverlapping(code.as_ptr(), page.add((slot * VDSO_SLOT) as usize), code.len()) };
    }
    VDSO_FRAME.store(frame, Ordering::Relaxed);
    Ok(())
}

/// A random, page-aligned address in [lo, hi).
fn random_page(lo: u64, hi: u64) -> u64 {
    let pages = (hi - lo) / PAGE_SIZE;
    lo + (rng::next_u64() % pages) * PAGE_SIZE
}

// ---------------------------------------------------------------- processes

struct Inner {
    handles: HandleTable,
    status: Option<u64>,
    started: bool,
    /// Keeps mapped memory alive as long as the process exists.
    mappings: Vec<Arc<Vmo>>,
}

pub struct Process {
    pub id: u64,
    pub name: String,
    pml4: u64,
    pub vdso_base: u64,
    /// Only init may report boot self-test results.
    pub is_init: bool,
    inner: SpinLock<Inner>,
    exited: WaitQueue,
}

static NEXT_PID: AtomicU64 = AtomicU64::new(1);

impl Process {
    pub fn create(name: &str, is_init: bool) -> Result<Arc<Process>, Status> {
        let pml4 = paging::new_user_pml4().map_err(|_| Status::NoMemory)?;
        let vdso_base = random_page(0x7000_0000_0000, 0x7f00_0000_0000);
        paging::map_user_page(pml4, vdso_base, VDSO_FRAME.load(Ordering::Relaxed), false, true).map_err(|_| Status::NoMemory)?;
        Ok(Arc::new(Process {
            id: NEXT_PID.fetch_add(1, Ordering::Relaxed),
            name: String::from(name),
            pml4,
            vdso_base,
            is_init,
            inner: SpinLock::new(Inner { handles: HandleTable::new(rng::next_u64() as u32), status: None, started: false, mappings: Vec::new() }),
            exited: WaitQueue::new(),
        }))
    }

    pub fn pml4(&self) -> u64 {
        self.pml4
    }

    pub fn with_handles<R>(&self, f: impl FnOnce(&mut HandleTable) -> R) -> R {
        f(&mut self.inner.lock().handles)
    }

    /// Maps `len` bytes of `vmo` (from `offset`) at user address `vaddr`.
    pub fn map(&self, vmo: &Arc<Vmo>, offset: u64, vaddr: u64, len: u64, flags: u64) -> Result<(), Status> {
        let (w, x) = (flags & map::WRITE != 0, flags & map::EXECUTE != 0);
        if (w && x) || flags & !(map::READ | map::WRITE | map::EXECUTE) != 0 {
            return Err(Status::InvalidArgs); // W^X
        }
        if len == 0 || vaddr % PAGE_SIZE != 0 || offset % PAGE_SIZE != 0 || len % PAGE_SIZE != 0 {
            return Err(Status::InvalidArgs);
        }
        let end = vaddr.checked_add(len).ok_or(Status::InvalidArgs)?;
        if vaddr == 0 || end > USER_TOP || offset.checked_add(len).is_none_or(|e| e > vmo.size()) {
            return Err(Status::OutOfRange);
        }
        let mut inner = self.inner.lock();
        for i in 0..len / PAGE_SIZE {
            let frame = vmo.frame(offset / PAGE_SIZE + i).ok_or(Status::OutOfRange)?;
            paging::map_user_page(self.pml4, vaddr + i * PAGE_SIZE, frame, w, x).map_err(|_| Status::BadState)?;
        }
        inner.mappings.push(vmo.clone());
        Ok(())
    }

    /// Starts the main thread. Returns BadState if already started.
    pub fn start(self: &Arc<Self>, entry: u64, stack_top: u64, args: [u64; 3]) -> Result<(), Status> {
        if entry >= USER_TOP || stack_top >= USER_TOP || stack_top < 16 {
            return Err(Status::InvalidArgs);
        }
        {
            let mut inner = self.inner.lock();
            if inner.started || inner.status.is_some() {
                return Err(Status::BadState);
            }
            inner.started = true;
        }
        // A function is entered with rsp % 16 == 8 (as if a return address were pushed).
        let rsp = (stack_top & !0xf) - 8;
        let name: &'static str = alloc::boxed::Box::leak(self.name.clone().into_boxed_str());
        sched::spawn_user(self.clone(), name, entry, rsp, args).map_err(|_| Status::NoMemory)?;
        Ok(())
    }

    /// Records the exit status (first one wins), closes every handle and wakes waiters.
    pub fn finish(&self, status: u64) {
        let closed = {
            let mut inner = self.inner.lock();
            if inner.status.is_some() {
                return;
            }
            inner.status = Some(status);
            inner.handles.clear()
        };
        drop(closed); // dropping endpoints notifies their peers; outside the lock
        self.exited.wake_all();
    }

    pub fn status(&self) -> Option<u64> {
        self.inner.lock().status
    }

    /// Blocks until the process has ended; returns its status.
    pub fn wait(&self) -> u64 {
        sched::wait_event(&self.exited, || self.status().is_some());
        self.status().unwrap_or(0)
    }
}

/// The process of the calling thread (syscalls only come from user threads).
pub fn current() -> Option<Arc<Process>> {
    sched::current().process.clone()
}

/// Ends the current process because its thread raised a CPU exception in
/// ring 3. Called from exception handlers; never returns.
pub fn kill_current(vector: u8, what: &str, rip: u64, addr: Option<u64>) -> ! {
    if let Some(p) = current() {
        match addr {
            Some(a) => serial_println!("AWA: process {} ({}) killed: {} at {:#x}, rip {:#x}", p.id, p.name, what, a, rip),
            None => serial_println!("AWA: process {} ({}) killed: {}, rip {:#x}", p.id, p.name, what, rip),
        }
        p.finish(EXIT_FAULT | u64::from(vector));
    }
    sched::exit()
}

// ---------------------------------------------------------------- loading init

/// Loads an ELF image into `p` at a random base. Returns the entry address.
fn load_elf(p: &Arc<Process>, image: &[u8]) -> Result<u64, &'static str> {
    let elf = awa_elf::Elf::parse(image).map_err(|_| "not a valid AWA executable")?;
    let size = elf.image_size().map_err(|_| "bad segments")?.next_multiple_of(PAGE_SIZE);
    let vmo = Vmo::create(size).map_err(|_| "image too large")?;
    elf.segments(|s| {
        let bytes = &image[s.file_offset as usize..(s.file_offset + s.file_size) as usize];
        vmo.write(s.vaddr, bytes).map_err(|_| awa_elf::Error::SegmentOutOfFile)
    })
    .map_err(|_| "bad segment")?;

    let base = random_page(0x1_0000_0000, 0x4000_0000_0000);
    elf.relocations(|offset, addend| vmo.write(offset, &(base + addend).to_le_bytes()).map_err(|_| awa_elf::Error::BadRelocations))
        .map_err(|_| "bad relocations")?;

    // A page shared by two segments is mapped once, and only if both want
    // the same permissions (same rule as libawa's loader).
    let mut result = Ok(());
    let (mut mapped_to, mut last_flags) = (0u64, 0u64);
    elf.segments(|s| {
        let mut start = s.vaddr / PAGE_SIZE * PAGE_SIZE;
        let end = (s.vaddr + s.mem_size).next_multiple_of(PAGE_SIZE);
        let flags = map::READ | if s.write { map::WRITE } else { 0 } | if s.execute { map::EXECUTE } else { 0 };
        if start < mapped_to {
            if flags != last_flags {
                result = Err(Status::InvalidArgs);
                return Ok(());
            }
            start = mapped_to;
        }
        if start < end {
            if let Err(e) = p.map(&vmo, start, base + start, end - start, flags) {
                result = Err(e);
            }
        }
        mapped_to = mapped_to.max(end);
        last_flags = flags;
        Ok(())
    })
    .map_err(|_| "bad segment")?;
    result.map_err(|_| "segments overlap or cannot be mapped")?;
    Ok(base + elf.entry())
}

const STACK_SIZE: u64 = 256 * 1024;

/// Creates init from `image` and starts it. Its bootstrap channel carries one
/// message: the names of `files` (newline-separated) and a VMO handle for each.
pub fn start_init(image: &[u8], files: &[(&str, &[u8])]) -> Result<Arc<Process>, &'static str> {
    let p = Process::create("init", true).map_err(|_| "cannot create process")?;
    let entry = load_elf(&p, image)?;

    let stack = Vmo::create(STACK_SIZE).map_err(|_| "no memory for the stack")?;
    // Leave an unmapped guard page below the stack.
    let stack_base = random_page(0x5000_0000_0000, 0x6000_0000_0000);
    p.map(&stack, 0, stack_base, STACK_SIZE, map::READ | map::WRITE).map_err(|_| "cannot map the stack")?;

    let (ours, theirs) = Endpoint::pair();
    let mut names = Vec::new();
    let mut handles = Vec::new();
    for (name, bytes) in files {
        let vmo = Vmo::create((bytes.len() as u64).max(1)).map_err(|_| "no memory for a file")?;
        vmo.write(0, bytes).map_err(|_| "file copy failed")?;
        names.extend_from_slice(name.as_bytes());
        names.push(b'\n');
        handles.push((Object::Vmo(vmo), VMO_RIGHTS));
    }
    ours.write(Message { bytes: names, handles }).map_err(|_| "bootstrap message failed")?;
    let bootstrap = p.with_handles(|h| h.insert(Object::Channel(theirs), CHANNEL_RIGHTS)).map_err(|_| "handle table full")?;
    drop(ours); // init sees the message, then PeerClosed

    p.start(entry, stack_base + STACK_SIZE, [p.vdso_base, u64::from(bootstrap), 0]).map_err(|_| "cannot start init")?;
    Ok(p)
}
