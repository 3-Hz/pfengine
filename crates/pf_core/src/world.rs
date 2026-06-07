//! The serializable game state and the pure `advance` function.
//!
//! [`World`] is `Clone` so rollback can snapshot it cheaply, and it provides a
//! [`World::checksum`] used by GGRS to detect desyncs between peers.

use crate::input::Input;
use crate::math::rng::Rng;
use crate::math::{Fx, V2};
use crate::systems;

/// A character's high-level action state (a stub of Melee's action-state IDs).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ActionState {
    Idle,
    Walk,
    Airborne,
    Hitstun,
}

/// One fighter's complete simulation state.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Fighter {
    pub pos: V2,
    pub vel: V2,
    pub state: ActionState,
    pub facing_right: bool,
}

/// The stage geometry (a single flat floor for Phase 0).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Stage {
    pub floor_y: Fx,
    pub left: Fx,
    pub right: Fx,
}

/// The entire game state. Cloning this is the rollback snapshot mechanism, so it
/// is kept flat and free of heap indirection.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct World {
    pub players: [Fighter; 2],
    pub stage: Stage,
    pub frame: u32,
    pub rng: Rng,
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

impl World {
    /// The initial state: two fighters standing on a flat stage.
    pub fn new() -> Self {
        let stage = Stage {
            floor_y: Fx::from_num(0),
            left: Fx::from_num(-200),
            right: Fx::from_num(200),
        };
        let fighter = |x: i32, facing_right: bool| Fighter {
            pos: V2::new(Fx::from_num(x), stage.floor_y),
            vel: V2::ZERO,
            state: ActionState::Idle,
            facing_right,
        };
        World {
            players: [fighter(-60, true), fighter(60, false)],
            stage,
            frame: 0,
            rng: Rng::new(0x9E37_79B9_7F4A_7C15),
        }
    }

    /// Advance the simulation by exactly one 60 Hz tick. **Pure function** of the
    /// current state and the given inputs.
    pub fn advance(&mut self, inputs: [Input; 2]) {
        for i in 0..self.players.len() {
            systems::step_fighter(&mut self.players[i], inputs[i], &self.stage);
        }
        // Keep the RNG evolving as part of the state so it participates in the
        // checksum even before any system consumes it.
        let _ = self.rng.next_u32();
        self.frame += 1;
    }

    /// A 128-bit FNV-1a hash of the full state, used for desync detection.
    /// Encodes every field in a fixed order — no unordered iteration.
    pub fn checksum(&self) -> u128 {
        const OFFSET: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
        let mut h = OFFSET;
        for p in &self.players {
            fnv1a(&mut h, &p.pos.x.to_bits().to_le_bytes());
            fnv1a(&mut h, &p.pos.y.to_bits().to_le_bytes());
            fnv1a(&mut h, &p.vel.x.to_bits().to_le_bytes());
            fnv1a(&mut h, &p.vel.y.to_bits().to_le_bytes());
            fnv1a(&mut h, &[p.state as u8, p.facing_right as u8]);
        }
        fnv1a(&mut h, &self.stage.floor_y.to_bits().to_le_bytes());
        fnv1a(&mut h, &self.stage.left.to_bits().to_le_bytes());
        fnv1a(&mut h, &self.stage.right.to_bits().to_le_bytes());
        fnv1a(&mut h, &self.frame.to_le_bytes());
        h
    }
}

#[inline]
fn fnv1a(h: &mut u128, bytes: &[u8]) {
    const PRIME: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013B;
    for &b in bytes {
        *h ^= b as u128;
        *h = h.wrapping_mul(PRIME);
    }
}
