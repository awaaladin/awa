//! Kernel objects reachable from user space, and the per-process handle table.
//!
//! A handle is a process-local number naming one object plus a set of rights.
//! It is the only way a program can act on anything (no ambient authority).
//! Handle values mix the slot index and a generation with a per-process
//! random tweak, so values cannot be guessed and stale ones are detected.

use crate::mem::{frames, phys_to_virt, PAGE_SIZE};
use crate::process::Process;
use crate::sched::WaitQueue;
use crate::sync::SpinLock;
use alloc::collections::VecDeque;
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use awa_abi::{rights, Status};

#[derive(Clone)]
pub enum Object {
    Process(Arc<Process>),
    Vmo(Arc<Vmo>),
    Channel(Arc<Endpoint>),
}

// ---------------------------------------------------------------- handles

struct Slot {
    object: Object,
    rights: u32,
    generation: u8,
}

pub struct HandleTable {
    slots: Vec<Option<Slot>>,
    generations: Vec<u8>,
    tweak: u32,
}

pub const MAX_HANDLES: usize = 4096;

impl HandleTable {
    pub fn new(tweak: u32) -> Self {
        Self { slots: Vec::new(), generations: Vec::new(), tweak: tweak & 0x00ff_ffff }
    }

    fn encode(&self, index: usize, generation: u8) -> u32 {
        (((index as u32) << 8) | u32::from(generation)) ^ self.tweak
    }

    fn decode(&self, handle: u32) -> Option<usize> {
        let v = handle ^ self.tweak;
        let (index, generation) = ((v >> 8) as usize, v as u8);
        match self.slots.get(index)? {
            Some(s) if s.generation == generation => Some(index),
            _ => None,
        }
    }

    pub fn insert(&mut self, object: Object, rights: u32) -> Result<u32, Status> {
        // Index 0 is never used, so a handle value is never 0 before the tweak.
        let index = match self.slots.iter().skip(1).position(Option::is_none) {
            Some(i) => i + 1,
            None => {
                if self.slots.len() >= MAX_HANDLES {
                    return Err(Status::NoMemory);
                }
                if self.slots.is_empty() {
                    self.slots.push(None);
                    self.generations.push(0);
                }
                self.slots.push(None);
                self.generations.push(0);
                self.slots.len() - 1
            }
        };
        let generation = self.generations[index].wrapping_add(1);
        self.generations[index] = generation;
        self.slots[index] = Some(Slot { object, rights, generation });
        Ok(self.encode(index, generation))
    }

    /// The object and rights behind `handle`.
    pub fn get(&self, handle: u32) -> Result<(Object, u32), Status> {
        let i = self.decode(handle).ok_or(Status::BadHandle)?;
        let s = self.slots[i].as_ref().ok_or(Status::BadHandle)?;
        Ok((s.object.clone(), s.rights))
    }

    pub fn remove(&mut self, handle: u32) -> Result<(Object, u32), Status> {
        let i = self.decode(handle).ok_or(Status::BadHandle)?;
        let s = self.slots[i].take().ok_or(Status::BadHandle)?;
        Ok((s.object, s.rights))
    }

    pub fn clear(&mut self) -> Vec<Object> {
        self.slots.iter_mut().filter_map(|s| s.take().map(|s| s.object)).collect()
    }
}

/// Checks that `have` includes every right in `need`.
pub fn require(have: u32, need: u32) -> Result<(), Status> {
    if have & need == need { Ok(()) } else { Err(Status::AccessDenied) }
}

// ---------------------------------------------------------------- VMO

/// Virtual memory object: a fixed set of zeroed physical pages.
/// TODO(awa): memory classes, lazy allocation and accounting (Phase 5).
pub struct Vmo {
    size: u64,
    frames: Vec<u64>,
}

pub const MAX_VMO_SIZE: u64 = 64 * 1024 * 1024;

impl Vmo {
    pub fn create(size: u64) -> Result<Arc<Vmo>, Status> {
        if size == 0 || size > MAX_VMO_SIZE {
            return Err(Status::InvalidArgs);
        }
        let pages = size.div_ceil(PAGE_SIZE);
        let mut v = Vmo { size: pages * PAGE_SIZE, frames: Vec::with_capacity(pages as usize) };
        for _ in 0..pages {
            match frames::alloc_zeroed() {
                Some(f) => v.frames.push(f),
                None => return Err(Status::NoMemory), // `v` drops and frees what it got
            }
        }
        Ok(Arc::new(v))
    }

    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn frame(&self, page: u64) -> Option<u64> {
        self.frames.get(page as usize).copied()
    }

    fn check(&self, offset: u64, len: u64) -> Result<(), Status> {
        match offset.checked_add(len) {
            Some(end) if end <= self.size => Ok(()),
            _ => Err(Status::OutOfRange),
        }
    }

    /// Copies kernel bytes into the VMO.
    pub fn write(&self, offset: u64, data: &[u8]) -> Result<(), Status> {
        self.check(offset, data.len() as u64)?;
        let mut done = 0;
        while done < data.len() {
            let at = offset + done as u64;
            let in_page = (at % PAGE_SIZE) as usize;
            let n = (PAGE_SIZE as usize - in_page).min(data.len() - done);
            let dst = phys_to_virt(self.frames[(at / PAGE_SIZE) as usize]) + in_page as u64;
            // SAFETY: the frame belongs to this VMO and is mapped by the direct
            // map; [in_page, in_page+n) lies inside it.
            unsafe { core::ptr::copy_nonoverlapping(data[done..].as_ptr(), dst as *mut u8, n) };
            done += n;
        }
        Ok(())
    }

    pub fn read(&self, offset: u64, out: &mut [u8]) -> Result<(), Status> {
        self.check(offset, out.len() as u64)?;
        let mut done = 0;
        while done < out.len() {
            let at = offset + done as u64;
            let in_page = (at % PAGE_SIZE) as usize;
            let n = (PAGE_SIZE as usize - in_page).min(out.len() - done);
            let src = phys_to_virt(self.frames[(at / PAGE_SIZE) as usize]) + in_page as u64;
            // SAFETY: as in `write`.
            unsafe { core::ptr::copy_nonoverlapping(src as *const u8, out[done..].as_mut_ptr(), n) };
            done += n;
        }
        Ok(())
    }
}

impl Drop for Vmo {
    fn drop(&mut self) {
        for &f in &self.frames {
            frames::free(f);
        }
    }
}

// ---------------------------------------------------------------- channels

pub struct Message {
    pub bytes: Vec<u8>,
    pub handles: Vec<(Object, u32)>,
}

/// One end of a channel. Messages written to an end arrive at its peer.
pub struct Endpoint {
    queue: SpinLock<VecDeque<Message>>,
    peer: SpinLock<Weak<Endpoint>>,
    readers: WaitQueue,
}

/// At most this many unread messages wait at one end.
pub const QUEUE_LIMIT: usize = 64;

impl Endpoint {
    pub fn pair() -> (Arc<Endpoint>, Arc<Endpoint>) {
        let mk = || Arc::new(Endpoint { queue: SpinLock::new(VecDeque::new()), peer: SpinLock::new(Weak::new()), readers: WaitQueue::new() });
        let (a, b) = (mk(), mk());
        *a.peer.lock() = Arc::downgrade(&b);
        *b.peer.lock() = Arc::downgrade(&a);
        (a, b)
    }

    fn peer_alive(&self) -> bool {
        self.peer.lock().strong_count() > 0
    }

    /// Delivers `msg` to the peer. On failure the message is handed back.
    pub fn write(&self, msg: Message) -> Result<(), (Status, Message)> {
        let Some(peer) = self.peer.lock().upgrade() else { return Err((Status::PeerClosed, msg)) };
        {
            let mut q = peer.queue.lock();
            if q.len() >= QUEUE_LIMIT {
                return Err((Status::ShouldWait, msg));
            }
            q.push_back(msg);
        }
        // The reader runs on this CPU next: senders usually wait for a reply.
        peer.readers.wake_all_here();
        Ok(())
    }

    /// Waits for a message. If it does not fit (`cap` bytes, `hcap`
    /// handles) it stays queued and BufferTooSmall is returned.
    pub fn read(&self, cap: usize, hcap: usize) -> Result<Message, Status> {
        crate::sched::wait_event(&self.readers, || !self.queue.lock().is_empty() || !self.peer_alive());
        let mut q = self.queue.lock();
        match q.front() {
            None => Err(Status::PeerClosed),
            Some(m) if m.bytes.len() > cap || m.handles.len() > hcap => Err(Status::BufferTooSmall),
            Some(_) => Ok(q.pop_front().expect("checked above")),
        }
    }
}

impl Drop for Endpoint {
    fn drop(&mut self) {
        // Readers blocked on the other end must learn the peer is gone.
        if let Some(peer) = self.peer.lock().upgrade() {
            peer.readers.wake_all();
        }
    }
}

/// Rights a fresh object handle gets.
pub const CHANNEL_RIGHTS: u32 = rights::READ | rights::WRITE | rights::TRANSFER | rights::DUPLICATE | rights::WAIT;
pub const VMO_RIGHTS: u32 = rights::READ | rights::WRITE | rights::MAP | rights::TRANSFER | rights::DUPLICATE;
pub const PROCESS_RIGHTS: u32 = rights::ALL;
