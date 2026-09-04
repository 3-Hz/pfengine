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

/// The entire game state. `Clone` is the rollback snapshot, so the state stays
/// contiguous `Copy` data: a `Vec<Fighter>` is one allocation and one memcpy.
/// What to avoid is hash-ordered containers (iteration order differs between
/// peers → desync) and per-entity boxes (a clone becomes N mallocs and N cache
/// misses).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct World {
    pub players: Vec<Fighter>,
    pub stage: Stage,
    pub frame: u32,
    pub rng: Rng,
}

impl Default for World {
    fn default() -> Self {
        Self::new(2)
    }
}

impl World {
    /// The initial state: `num_players` fighters spread evenly across a flat
    /// stage, each facing the center. Zero players is a valid, empty world.
    pub fn new(num_players: usize) -> Self {
        let stage = Stage {
            floor_y: Fx::from_num(0),
            left: Fx::from_num(-200),
            right: Fx::from_num(200),
        };
        // Divide before multiplying: `width * (n + 1)` overflows I16F16 past
        // ~80 players.
        let step = (stage.right - stage.left) / Fx::from_num(num_players as i32 + 1);
        let players = (0..num_players)
            .map(|i| {
                let x = stage.left + step * Fx::from_num(i as i32 + 1);
                Fighter {
                    pos: V2::new(x, stage.floor_y),
                    vel: V2::ZERO,
                    state: ActionState::Idle,
                    facing_right: x <= Fx::ZERO,
                }
            })
            .collect();
        World {
            players,
            stage,
            frame: 0,
            rng: Rng::new(0x9E37_79B9_7F4A_7C15),
        }
    }

    /// Advance the simulation by exactly one 60 Hz tick. **Pure function** of the
    /// current state and the given inputs, one per fighter.
    ///
    /// # Panics
    /// If `inputs.len() != self.players.len()` — a contract bug, not a runtime
    /// condition; `zip` would otherwise truncate silently.
    pub fn advance(&mut self, inputs: &[Input]) {
        assert_eq!(
            inputs.len(),
            self.players.len(),
            "advance needs exactly one Input per fighter"
        );
        for (fighter, &input) in self.players.iter_mut().zip(inputs) {
            systems::step_fighter(fighter, input, &self.stage);
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
        fnv1a(&mut h, &(self.players.len() as u32).to_le_bytes());
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

#[cfg(test)]
mod tests {
    use super::*;

    fn all_on_stage(w: &World) -> bool {
        w.players.iter().all(|f| {
            f.pos.x >= w.stage.left && f.pos.x <= w.stage.right && f.pos.y == w.stage.floor_y
        })
    }

    #[test]
    fn new_spawns_requested_number_of_fighters_on_stage() {
        for n in [0, 1, 4, 100] {
            let w = World::new(n);
            assert_eq!(w.players.len(), n, "n = {n}");
            assert!(all_on_stage(&w), "n = {n}");
        }
    }

    #[test]
    fn fighters_face_the_center() {
        let w = World::new(4);
        assert!(w.players[0].facing_right);
        assert!(w.players[1].facing_right);
        assert!(!w.players[2].facing_right);
        assert!(!w.players[3].facing_right);
    }

    #[test]
    #[should_panic]
    fn advance_rejects_wrong_input_count() {
        let mut w = World::new(2);
        w.advance(&[Input::default()]);
    }

    #[test]
    fn advance_steps_every_fighter() {
        let mut w = World::new(3);
        let right = Input {
            stick_x: 127,
            ..Input::default()
        };
        w.advance(&[right; 3]);
        for (i, f) in w.players.iter().enumerate() {
            assert!(f.vel.x > Fx::ZERO, "fighter {i} did not move");
        }
    }

    #[test]
    fn checksum_differs_by_player_count() {
        assert_ne!(World::new(2).checksum(), World::new(3).checksum());
    }
}
