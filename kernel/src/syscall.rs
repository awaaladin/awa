//! System call dispatcher.
//!
//! Rules every call follows (docs/SECURITY.md, kernel threat model):
//! - only accepted from the caller's vDSO page;
//! - objects are named by handles, checked for type and rights;
//! - user memory is copied in once, through range-checked fault-safe copies,
//!   before it is used.

use crate::arch::syscall::{copy_from_user, copy_to_user, SyscallFrame};
use crate::object::{require, Endpoint, Message, Object, Vmo, CHANNEL_RIGHTS, PROCESS_RIGHTS, VMO_RIGHTS};
use crate::process::{self, Process, SYSCALL_BASE};
use crate::sched;
use crate::sync::SpinLock;
use crate::{arch, rng, serial};
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use awa_abi::{rights, Call, Status, CALL_COUNT, MAX_MESSAGE_BYTES, MAX_MESSAGE_HANDLES, USER_TOP, VDSO_SIZE};
use core::sync::atomic::{AtomicU64, Ordering};

/// Syscalls refused because they did not come from the vDSO.
pub static REJECTED_NOT_VDSO: AtomicU64 = AtomicU64::new(0);

/// Boot self-test results reported by init: (name, passed, detail).
pub static USER_RESULTS: SpinLock<Vec<(String, bool, String)>> = SpinLock::new(Vec::new());

type R = Result<u64, Status>;

#[unsafe(no_mangle)]
pub extern "C" fn dispatch(f: &mut SyscallFrame) {
    // `sysretq` to a non-canonical address would fault in ring 0 on the user
    // stack: never return to anything at or above USER_TOP.
    if f.rcx >= USER_TOP {
        process::kill_current(13, "bad syscall return address", f.rcx, None);
    }
    arch::enable_interrupts();
    let result = match process::current() {
        Some(p) => {
            if f.rcx < p.vdso_base || f.rcx >= p.vdso_base + VDSO_SIZE {
                REJECTED_NOT_VDSO.fetch_add(1, Ordering::Relaxed);
                Err(Status::NotFromVdso)
            } else {
                let slot = f.rax.wrapping_sub(SYSCALL_BASE.load(Ordering::Relaxed));
                call(&p, slot, [f.rdi, f.rsi, f.rdx, f.r10, f.r8, f.r9])
            }
        }
        None => Err(Status::BadState),
    };
    f.rax = match result {
        Ok(v) => v,
        Err(s) => (-(s as i64)) as u64,
    };
    // Keep only the user's arithmetic flags and DF; force IF on.
    f.r11 = (f.r11 & 0xcd5) | 0x202;
    arch::disable_interrupts();
}

fn slot_to_call(slot: u64) -> Option<Call> {
    if slot >= CALL_COUNT {
        return None;
    }
    use Call::*;
    Some(match slot {
        0 => DebugWrite,
        1 => ProcessExit,
        2 => ClockGet,
        3 => HandleClose,
        4 => HandleDuplicate,
        5 => ChannelCreate,
        6 => ChannelWrite,
        7 => ChannelRead,
        8 => VmoCreate,
        9 => VmoWrite,
        10 => VmoRead,
        11 => ProcessCreate,
        12 => ProcessMap,
        13 => ProcessStart,
        14 => ProcessWait,
        15 => RandomGet,
        16 => VmoSize,
        17 => TestReport,
        18 => ProcessVdso,
        19 => ChannelCall,
        _ => return None,
    })
}

fn handle(v: u64) -> Result<u32, Status> {
    u32::try_from(v).map_err(|_| Status::BadHandle)
}

fn get(p: &Process, h: u64) -> Result<(Object, u32), Status> {
    let h = handle(h)?;
    p.with_handles(|t| t.get(h))
}

fn get_vmo(p: &Process, h: u64, need: u32) -> Result<Arc<Vmo>, Status> {
    match get(p, h)? {
        (Object::Vmo(v), r) => require(r, need).map(|_| v),
        _ => Err(Status::WrongType),
    }
}

fn get_channel(p: &Process, h: u64, need: u32) -> Result<Arc<Endpoint>, Status> {
    match get(p, h)? {
        (Object::Channel(c), r) => require(r, need).map(|_| c),
        _ => Err(Status::WrongType),
    }
}

fn get_process(p: &Process, h: u64, need: u32) -> Result<Arc<Process>, Status> {
    match get(p, h)? {
        (Object::Process(q), r) => require(r, need).map(|_| q),
        _ => Err(Status::WrongType),
    }
}

fn read_user(ptr: u64, len: u64, limit: u64) -> Result<Vec<u8>, Status> {
    if len > limit {
        return Err(Status::InvalidArgs);
    }
    let mut buf = vec![0u8; len as usize];
    if copy_from_user(&mut buf, ptr) { Ok(buf) } else { Err(Status::InvalidArgs) }
}

fn write_user(ptr: u64, data: &[u8]) -> Result<(), Status> {
    if copy_to_user(ptr, data) { Ok(()) } else { Err(Status::InvalidArgs) }
}

fn read_handles(ptr: u64, count: u64) -> Result<Vec<u32>, Status> {
    if count > MAX_MESSAGE_HANDLES {
        return Err(Status::InvalidArgs);
    }
    let raw = read_user(ptr, count * 4, MAX_MESSAGE_HANDLES * 4)?;
    Ok(raw.chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect())
}

fn call(p: &Arc<Process>, slot: u64, a: [u64; 6]) -> R {
    let Some(c) = slot_to_call(slot) else { return Err(Status::NotSupported) };
    match c {
        Call::DebugWrite => {
            let bytes = read_user(a[0], a[1], 4096)?;
            let text = String::from_utf8_lossy(&bytes);
            serial::_print(format_args!("{}", text));
            Ok(0)
        }
        Call::ProcessExit => {
            p.finish(a[0] & 0xffff_ffff);
            sched::exit()
        }
        Call::ClockGet => Ok(sched::tsc_to_ns(arch::apic::tsc())),
        Call::RandomGet => Ok(rng::next_u64()),
        Call::HandleClose => {
            let h = handle(a[0])?;
            let removed = p.with_handles(|t| t.remove(h))?;
            drop(removed); // outside the handle-table lock
            Ok(0)
        }
        Call::HandleDuplicate => {
            let (obj, have) = get(p, a[0])?;
            require(have, rights::DUPLICATE)?;
            let want = a[1] as u32;
            if want & !have != 0 || a[1] > u64::from(u32::MAX) {
                return Err(Status::AccessDenied); // rights can only shrink
            }
            Ok(u64::from(p.with_handles(|t| t.insert(obj, want))?))
        }
        Call::ChannelCreate => {
            let (x, y) = Endpoint::pair();
            let hx = p.with_handles(|t| t.insert(Object::Channel(x), CHANNEL_RIGHTS))?;
            let hy = match p.with_handles(|t| t.insert(Object::Channel(y), CHANNEL_RIGHTS)) {
                Ok(h) => h,
                Err(e) => {
                    let _ = p.with_handles(|t| t.remove(hx));
                    return Err(e);
                }
            };
            let mut out = [0u8; 8];
            out[..4].copy_from_slice(&hx.to_le_bytes());
            out[4..].copy_from_slice(&hy.to_le_bytes());
            if let Err(e) = write_user(a[0], &out) {
                let _ = p.with_handles(|t| (t.remove(hx), t.remove(hy)));
                return Err(e);
            }
            Ok(0)
        }
        Call::ChannelWrite => {
            let ch = get_channel(p, a[0], rights::WRITE)?;
            let bytes = read_user(a[1], a[2], MAX_MESSAGE_BYTES)?;
            let values = read_handles(a[3], a[4])?;
            // Validate every handle before moving any of them.
            let own = handle(a[0])?;
            for (i, &v) in values.iter().enumerate() {
                if v == own || values[..i].contains(&v) {
                    return Err(Status::InvalidArgs); // no sending a channel into itself, no duplicates
                }
                let (_, r) = p.with_handles(|t| t.get(v))?;
                require(r, rights::TRANSFER)?;
            }
            let moved: Vec<(Object, u32)> = p.with_handles(|t| values.iter().filter_map(|&v| t.remove(v).ok()).collect());
            match ch.write(Message { bytes, handles: moved }) {
                Ok(()) => Ok(0),
                Err((status, msg)) => {
                    // Give the handles back (as new values) so nothing leaks silently.
                    let _ = p.with_handles(|t| msg.handles.into_iter().map(|(o, r)| t.insert(o, r)).collect::<Vec<_>>());
                    Err(status)
                }
            }
        }
        Call::ChannelRead => {
            let ch = get_channel(p, a[0], rights::READ)?;
            let (cap, hcap) = (a[2].min(MAX_MESSAGE_BYTES) as usize, a[4].min(MAX_MESSAGE_HANDLES) as usize);
            let msg = ch.read(cap, hcap)?;
            write_user(a[1], &msg.bytes)?;
            let count = msg.handles.len() as u64;
            let len = msg.bytes.len() as u64;
            let values: Vec<u32> = p.with_handles(|t| msg.handles.into_iter().filter_map(|(o, r)| t.insert(o, r).ok()).collect());
            let raw: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
            write_user(a[3], &raw)?;
            Ok(len | (count << 32))
        }
        Call::ChannelCall => {
            let ch = get_channel(p, a[0], rights::READ | rights::WRITE)?;
            let bytes = read_user(a[1], a[2], MAX_MESSAGE_BYTES)?;
            ch.write(Message { bytes, handles: Vec::new() }).map_err(|(s, _)| s)?;
            let msg = ch.read(a[4].min(MAX_MESSAGE_BYTES) as usize, 0)?;
            write_user(a[3], &msg.bytes)?;
            Ok(msg.bytes.len() as u64)
        }
        Call::VmoCreate => {
            let v = Vmo::create(a[0])?;
            Ok(u64::from(p.with_handles(|t| t.insert(Object::Vmo(v), VMO_RIGHTS))?))
        }
        Call::VmoSize => Ok(get_vmo(p, a[0], 0)?.size()),
        Call::VmoWrite => {
            let v = get_vmo(p, a[0], rights::WRITE)?;
            let mut done = 0;
            while done < a[3] {
                let n = (a[3] - done).min(4096);
                let chunk = read_user(a[2] + done, n, 4096)?;
                v.write(a[1] + done, &chunk)?;
                done += n;
            }
            Ok(0)
        }
        Call::VmoRead => {
            let v = get_vmo(p, a[0], rights::READ)?;
            let mut done = 0;
            let mut chunk = [0u8; 4096];
            while done < a[3] {
                let n = (a[3] - done).min(4096) as usize;
                v.read(a[1] + done, &mut chunk[..n])?;
                write_user(a[2] + done, &chunk[..n])?;
                done += n as u64;
            }
            Ok(0)
        }
        Call::ProcessCreate => {
            // TODO(awa): require a process-creation capability (no ambient authority).
            let child = Process::create("process", false)?;
            Ok(u64::from(p.with_handles(|t| t.insert(Object::Process(child), PROCESS_RIGHTS))?))
        }
        Call::ProcessMap => {
            let target = get_process(p, a[0], rights::MANAGE)?;
            let mut need = rights::MAP | rights::READ;
            if a[5] & awa_abi::map::WRITE != 0 {
                need |= rights::WRITE;
            }
            let v = get_vmo(p, a[1], need)?;
            target.map(&v, a[2], a[3], a[4], a[5])?;
            Ok(0)
        }
        Call::ProcessStart => {
            let target = get_process(p, a[0], rights::MANAGE)?;
            let mut child_handle = 0;
            if a[3] != 0 {
                let (obj, r) = get(p, a[3])?;
                require(r, rights::TRANSFER)?;
                child_handle = target.with_handles(|t| t.insert(obj, r))?;
                let _ = p.with_handles(|t| t.remove(handle(a[3])?));
            }
            target.start(a[1], a[2], [target.vdso_base, u64::from(child_handle), 0])?;
            Ok(0)
        }
        Call::ProcessWait => {
            let target = get_process(p, a[0], rights::WAIT)?;
            Ok(target.wait())
        }
        Call::ProcessVdso => Ok(get_process(p, a[0], rights::MANAGE)?.vdso_base),
        Call::TestReport => {
            if !p.is_init {
                return Err(Status::AccessDenied);
            }
            let name = String::from_utf8_lossy(&read_user(a[0], a[1], 64)?).into_owned();
            let detail = String::from_utf8_lossy(&read_user(a[3], a[4], 256)?).into_owned();
            USER_RESULTS.lock().push((name, a[2] != 0, detail));
            Ok(0)
        }
    }
}
