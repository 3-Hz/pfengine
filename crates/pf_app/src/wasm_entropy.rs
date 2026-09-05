//! Byte source for `getrandom` on the web.
//!
//! ggrs pulls in `rand`, whose `getrandom` refuses to compile for
//! `wasm32-unknown-unknown` unless it is told where bytes come from. Its `js`
//! feature would drag wasm-bindgen into the import table, which macroquad's
//! loader cannot satisfy; the `custom` feature lets us register this instead.
//!
//! Not cryptographic. ggrs uses it for handshake nonces only, and never while
//! the session has no peers. Lives in `pf_app` because only `pf_app` may
//! carry platform `cfg`s.

use std::sync::atomic::{AtomicU64, Ordering};

/// xorshift64* state. Zero means "not seeded yet".
static STATE: AtomicU64 = AtomicU64::new(0);

fn next_u64() -> u64 {
    let mut x = STATE.load(Ordering::Relaxed);
    if x == 0 {
        // First use: seed from the wall clock (seconds, with fraction) and
        // force the low bit so the xorshift state is never zero.
        x = (macroquad::miniquad::date::now() * 1e6) as u64 | 1;
    }
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    STATE.store(x, Ordering::Relaxed);
    x.wrapping_mul(0x2545_F491_4F6C_DD1D)
}

fn fill(dest: &mut [u8]) -> Result<(), getrandom::Error> {
    for chunk in dest.chunks_mut(8) {
        let bytes = next_u64().to_le_bytes();
        chunk.copy_from_slice(&bytes[..chunk.len()]);
    }
    Ok(())
}

getrandom::register_custom_getrandom!(fill);
