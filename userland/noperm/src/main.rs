//! noperm: is told the number of a handle that belongs to init and tries to
//! use it. Handles are per process, so the kernel must refuse. Exits 0 if it
//! was refused, 1 if it somehow got access.

#![no_std]
#![no_main]

use awa::abi::Status;
use awa::{channel, vmo, Startup};

#[unsafe(no_mangle)]
fn awa_main(start: Startup) -> i64 {
    let mut buf = [0u8; 8];
    let Ok((4, 0)) = channel::read(start.bootstrap, &mut buf, &mut []) else { return 2 };
    let stolen = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    let mut out = [0u8; 8];
    match vmo::read(stolen, 0, &mut out) {
        Err(Status::BadHandle) => 0,
        _ => 1,
    }
}
