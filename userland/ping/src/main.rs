//! ping: receives its channel to pong from init, hands pong a VMO, measures
//! message round trips, and reports back to init.

#![no_std]
#![no_main]

use awa::{channel, clock_ns, fmt_line, vmo, Startup};

const ROUNDS: u64 = 2000;

#[unsafe(no_mangle)]
fn awa_main(start: Startup) -> i64 {
    let to_init = start.bootstrap;
    let mut buf = [0u8; 64];
    let mut hs = [0u32; 1];
    // init sends us our end of the channel to pong.
    let Ok((_, 1)) = channel::read(to_init, &mut buf, &mut hs) else { return 1 };
    let pong = hs[0];

    // Hand pong a memory object; pong reads it and replies with its text.
    let handoff = (|| -> awa::Result<bool> {
        let v = vmo::create(4096)?;
        vmo::write(v, 0, b"hello from ping")?;
        channel::write(pong, b"vmo", &[v])?;
        let (n, _) = channel::read(pong, &mut buf, &mut [])?;
        Ok(&buf[..n] == b"got:hello from ping")
    })();

    // Round trips: 64-byte message to pong and back.
    let msg = [7u8; 64];
    let t0 = clock_ns();
    let mut ok = true;
    for _ in 0..ROUNDS {
        ok &= channel::write(pong, &msg, &[]).is_ok();
        ok &= matches!(channel::read(pong, &mut buf, &mut []), Ok((64, 0)));
    }
    let rtt = (clock_ns() - t0) / ROUNDS;

    let report = fmt_line!("handoff={} rtt_ns={} rounds_ok={}", if handoff == Ok(true) { "ok" } else { "failed" }, rtt, ok);
    let _ = channel::write(to_init, report.as_str().as_bytes(), &[]);
    if handoff == Ok(true) && ok { 0 } else { 2 }
}
