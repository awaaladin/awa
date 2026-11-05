//! Kernel random numbers: a ChaCha20 stream (RFC 8439) keyed at boot from
//! RDRAND, when the CPU has it, mixed with TSC timing jitter. RDRAND is never
//! trusted alone (docs/SECURITY.md).
//!
//! TODO(awa): reseed periodically from interrupt-timing entropy; per-CPU
//! state once SMP is up (Phase 3).

use crate::arch::{apic, cpu};
use crate::sync::SpinLock;

struct State {
    key: [u32; 8],
    nonce: [u32; 3],
    counter: u32,
    block: [u32; 16],
    used: usize,
}

static STATE: SpinLock<State> = SpinLock::new(State { key: [0; 8], nonce: [0; 3], counter: 0, block: [0; 16], used: 16 });

fn quarter(s: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
    s[a] = s[a].wrapping_add(s[b]);
    s[d] = (s[d] ^ s[a]).rotate_left(16);
    s[c] = s[c].wrapping_add(s[d]);
    s[b] = (s[b] ^ s[c]).rotate_left(12);
    s[a] = s[a].wrapping_add(s[b]);
    s[d] = (s[d] ^ s[a]).rotate_left(8);
    s[c] = s[c].wrapping_add(s[d]);
    s[b] = (s[b] ^ s[c]).rotate_left(7);
}

/// The ChaCha20 block function (RFC 8439 §2.3).
pub fn chacha20_block(key: &[u32; 8], counter: u32, nonce: &[u32; 3]) -> [u32; 16] {
    let mut init = [0u32; 16];
    init[..4].copy_from_slice(&[0x6170_7865, 0x3320_646e, 0x7962_2d32, 0x6b20_6574]);
    init[4..12].copy_from_slice(key);
    init[12] = counter;
    init[13..].copy_from_slice(nonce);
    let mut s = init;
    for _ in 0..10 {
        quarter(&mut s, 0, 4, 8, 12);
        quarter(&mut s, 1, 5, 9, 13);
        quarter(&mut s, 2, 6, 10, 14);
        quarter(&mut s, 3, 7, 11, 15);
        quarter(&mut s, 0, 5, 10, 15);
        quarter(&mut s, 1, 6, 11, 12);
        quarter(&mut s, 2, 7, 8, 13);
        quarter(&mut s, 3, 4, 9, 14);
    }
    for (o, i) in s.iter_mut().zip(init) {
        *o = o.wrapping_add(i);
    }
    s
}

/// RDRAND can transiently fail; retry a few times as Intel recommends.
fn rdrand() -> Option<u64> {
    (0..10).find_map(|_| cpu::rdrand64())
}

/// Seeds the generator. Returns which sources contributed.
pub fn init(have_rdrand: bool) -> &'static str {
    let mut seed = [0u32; 11];
    let mut used_rdrand = false;
    for (i, chunk) in seed.chunks_mut(2).enumerate() {
        // Timing jitter: low TSC bits around a short busy loop.
        let mut jitter = apic::tsc();
        for _ in 0..(64 + i * 7) {
            core::hint::spin_loop();
        }
        jitter = jitter.rotate_left(17) ^ apic::tsc();
        let mut word = jitter;
        if have_rdrand {
            if let Some(r) = rdrand() {
                word ^= r;
                used_rdrand = true;
            }
        }
        chunk[0] = word as u32;
        if chunk.len() > 1 {
            chunk[1] = (word >> 32) as u32;
        }
    }
    // Hash the raw seed through one ChaCha20 block so key bits are uniform.
    let mut key = [0u32; 8];
    key.copy_from_slice(&seed[..8]);
    let nonce = [seed[8], seed[9], seed[10]];
    let mixed = chacha20_block(&key, 0, &nonce);
    let mut st = STATE.lock();
    st.key.copy_from_slice(&mixed[..8]);
    st.nonce.copy_from_slice(&mixed[8..11]);
    st.counter = 1;
    st.used = 16;
    if used_rdrand { "RDRAND + TSC jitter" } else { "TSC jitter only (no RDRAND)" }
}

/// Next 64 random bits.
pub fn next_u64() -> u64 {
    // TODO(awa): per-CPU generators if this lock ever shows up in profiles.
    let mut st = STATE.lock();
    if st.used + 2 > 16 {
        st.block = chacha20_block(&st.key, st.counter, &st.nonce);
        st.counter = st.counter.wrapping_add(1);
        st.used = 0;
    }
    let v = u64::from(st.block[st.used]) | (u64::from(st.block[st.used + 1]) << 32);
    st.used += 2;
    v
}
