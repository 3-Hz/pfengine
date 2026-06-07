//! Player input — the only thing that travels over the network for rollback.
//!
//! Kept tiny and `serde`-serializable, which is what GGRS requires of the
//! transmitted input type.

use serde::{Deserialize, Serialize};

/// Button bitflags packed into [`Input::buttons`].
pub mod buttons {
    pub const JUMP: u16 = 1 << 0;
    pub const ATTACK: u16 = 1 << 1;
    pub const SHIELD: u16 = 1 << 2;
    pub const GRAB: u16 = 1 << 3;
    pub const SPECIAL: u16 = 1 << 4;
}

/// One player's input for a single 60 Hz tick.
///
/// Implements `Serialize`/`Deserialize` (required by GGRS for the
/// network-transmitted input type). Analog sticks are quantized to
/// `i8` (`-127..=127`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize)]
pub struct Input {
    pub buttons: u16,
    pub stick_x: i8,
    pub stick_y: i8,
    pub cstick_x: i8,
    pub cstick_y: i8,
}

impl Input {
    #[inline]
    pub fn pressed(&self, mask: u16) -> bool {
        self.buttons & mask != 0
    }
}
