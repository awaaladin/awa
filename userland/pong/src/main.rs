//! pong: answers ping. A message carrying a VMO is answered with the VMO's
//! text; anything else is echoed. Ends when ping's side of the channel closes.

#![no_std]
#![no_main]

use awa::abi::Status;
use awa::{channel, vmo, Startup};

#[unsafe(no_mangle)]
fn awa_main(start: Startup) -> i64 {
    let ch = start.bootstrap;
    let mut buf = [0u8; 256];
    let mut hs = [0u32; 4];
    loop {
        match channel::read(ch, &mut buf, &mut hs) {
            Ok((_, 1)) => {
                let mut text = [0u8; 15];
                if vmo::read(hs[0], 0, &mut text).is_err() {
                    return 3;
                }
                let mut reply = [0u8; 19];
                reply[..4].copy_from_slice(b"got:");
                reply[4..].copy_from_slice(&text);
                if channel::write(ch, &reply, &[]).is_err() {
                    return 4;
                }
            }
            Ok((n, _)) => {
                if channel::write(ch, &buf[..n], &[]).is_err() {
                    return 5;
                }
            }
            Err(Status::PeerClosed) => return 0,
            Err(_) => return 6,
        }
    }
}
