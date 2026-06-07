//! Deterministic pseudo-random number generator.
//!
//! Seeded from simulation state and advanced *inside* the simulation, so every
//! machine draws the same sequence. Never seed this from the OS or wall clock.

use crate::math::Fx;

/// A small, fast xorshift64* generator. Lives inside [`crate::World`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Rng(u64);

impl Rng {
    /// Create an RNG from a seed. A non-zero state is enforced.
    #[inline]
    pub const fn new(seed: u64) -> Self {
        // xorshift must never have an all-zero state.
        Rng(seed | 1)
    }

    /// Advance the state and return the next 32-bit value.
    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        (x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 32) as u32
    }

    /// A fixed-point value in `[0, 1)`.
    #[inline]
    pub fn next_fx(&mut self) -> Fx {
        // Use the high 16 bits as the fractional part of an I16F16.
        let frac = (self.next_u32() >> 16) as i32;
        Fx::from_bits(frac)
    }
}
