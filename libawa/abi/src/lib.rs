//! The contract between the AWA kernel and user programs.
//!
//! Programs never use syscall numbers. The kernel maps a read-only, executable
//! page (the vDSO) into every process at a random address and passes that
//! address at start-up. Call `n` lives at `vdso + n * VDSO_SLOT`; it is an
//! ordinary `extern "sysv64"` function taking up to six u64 arguments and
//! returning an i64 (>= 0 success, < 0 a `Status`). Syscall numbers behind
//! the slots are private and change every boot (docs/spec/05 §2).

#![no_std]

pub const VDSO_SLOT: u64 = 16;
pub const VDSO_SIZE: u64 = 4096;

/// vDSO call slots. Append only: the slot index is the stable ABI.
#[repr(u64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    /// (ptr, len) -> 0. Writes text to the kernel log.
    DebugWrite = 0,
    /// (code) -> never returns.
    ProcessExit = 1,
    /// () -> nanoseconds since boot.
    ClockGet = 2,
    /// (handle) -> 0
    HandleClose = 3,
    /// (handle, rights) -> new handle with the same or fewer rights
    HandleDuplicate = 4,
    /// (out_ptr: *mut [u32; 2]) -> 0. Creates a channel; writes both ends.
    ChannelCreate = 5,
    /// (handle, bytes, len, handles_ptr, handle_count) -> 0
    ChannelWrite = 6,
    /// (handle, buf, cap, handles_buf, handle_cap) -> packed (bytes | handles << 32). Blocks until a message arrives.
    ChannelRead = 7,
    /// (size) -> VMO handle
    VmoCreate = 8,
    /// (handle, offset, ptr, len) -> 0
    VmoWrite = 9,
    /// (handle, offset, ptr, len) -> 0
    VmoRead = 10,
    /// () -> process handle (empty address space)
    ProcessCreate = 11,
    /// (process, vmo, vmo_offset, vaddr, len, flags) -> 0. Maps pages; W and X together are refused.
    ProcessMap = 12,
    /// (process, entry, stack_top, handle_to_transfer) -> 0. Starts the main thread.
    ProcessStart = 13,
    /// (process) -> exit status (see `ExitStatus`). Blocks until it ends.
    ProcessWait = 14,
    /// () -> 64 random bits
    RandomGet = 15,
    /// (vmo) -> size in bytes
    VmoSize = 16,
    /// (name_ptr, name_len, ok, detail_ptr, detail_len) -> 0. Boot self-test result (init only).
    TestReport = 17,
    /// (process) -> vDSO base address in that process (for its start-up arguments).
    ProcessVdso = 18,
    /// (handle, wbuf, wlen, rbuf, rcap) -> reply length. Sends a message
    /// (no handles) and waits for the reply on the same channel end.
    ChannelCall = 19,
}

pub const CALL_COUNT: u64 = 20;

/// Error codes (returned negated).
#[repr(i64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    BadHandle = 1,
    WrongType = 2,
    AccessDenied = 3,
    InvalidArgs = 4,
    OutOfRange = 5,
    NoMemory = 6,
    PeerClosed = 7,
    BufferTooSmall = 8,
    NotFromVdso = 9,
    BadState = 10,
    NotSupported = 11,
    ShouldWait = 12,
}

impl Status {
    pub fn from_code(code: i64) -> Option<Status> {
        use Status::*;
        Some(match -code {
            1 => BadHandle,
            2 => WrongType,
            3 => AccessDenied,
            4 => InvalidArgs,
            5 => OutOfRange,
            6 => NoMemory,
            7 => PeerClosed,
            8 => BufferTooSmall,
            9 => NotFromVdso,
            10 => BadState,
            11 => NotSupported,
            12 => ShouldWait,
            _ => return None,
        })
    }
}

/// Handle rights.
pub mod rights {
    pub const READ: u32 = 1 << 0;
    pub const WRITE: u32 = 1 << 1;
    pub const EXECUTE: u32 = 1 << 2;
    pub const MAP: u32 = 1 << 3;
    pub const DUPLICATE: u32 = 1 << 4;
    pub const TRANSFER: u32 = 1 << 5;
    pub const WAIT: u32 = 1 << 6;
    pub const MANAGE: u32 = 1 << 7;
    pub const ALL: u32 = 0xff;
}

/// `ProcessMap` flags.
pub mod map {
    pub const READ: u64 = 1;
    pub const WRITE: u64 = 2;
    pub const EXECUTE: u64 = 4;
}

/// Process exit status from `ProcessWait` (always >= 0): low 32 bits are the
/// exit code for a normal exit; if bit 62 is set the process was killed by a
/// CPU fault and bits 0-7 hold the exception vector (14 = page fault).
pub const EXIT_FAULT: u64 = 1 << 62;

/// User address space limit: no mapping may reach this address. The last
/// page below the canonical boundary stays unmapped (sysret safety).
pub const USER_TOP: u64 = 0x0000_7fff_ffff_0000;

/// Limits for one channel message.
pub const MAX_MESSAGE_BYTES: u64 = 64 * 1024;
pub const MAX_MESSAGE_HANDLES: u64 = 64;
