//! Fixed-point math for the deterministic core.
//!
//! `Fx` is the engine's single scalar type. Swapping precision (e.g. to the
//! 64-bit `I32F32` for more headroom) is a one-line change here.

pub mod rng;

pub use fixed::types::I16F16;

/// The engine-wide fixed-point scalar: 16 integer bits, 16 fractional bits.
pub type Fx = I16F16;

/// A 2D fixed-point vector. Used for positions, velocities, and offsets.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct V2 {
    pub x: Fx,
    pub y: Fx,
}

impl V2 {
    pub const ZERO: V2 = V2 {
        x: Fx::ZERO,
        y: Fx::ZERO,
    };

    #[inline]
    pub const fn new(x: Fx, y: Fx) -> Self {
        Self { x, y }
    }

    #[inline]
    pub fn add(self, o: V2) -> V2 {
        V2 {
            x: self.x + o.x,
            y: self.y + o.y,
        }
    }

    #[inline]
    pub fn sub(self, o: V2) -> V2 {
        V2 {
            x: self.x - o.x,
            y: self.y - o.y,
        }
    }

    /// Scale both components by a fixed-point factor.
    #[inline]
    pub fn scale(self, s: Fx) -> V2 {
        V2 {
            x: self.x * s,
            y: self.y * s,
        }
    }
}
