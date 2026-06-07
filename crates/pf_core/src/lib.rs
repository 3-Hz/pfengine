//! `pf_core` — the deterministic simulation core of pfengine.
//!
//! # The one rule
//! The simulation is a pure function: `new_state = update(old_state, inputs)`.
//! To keep rollback correct across every platform, this crate uses **fixed-point
//! math only** — no `f32`/`f64` in simulation logic — and avoids wall-clock time,
//! unordered iteration, and any randomness sourced from outside the state.
//!
//! Anything that would break determinism (rendering, OS, time) lives in other
//! crates. Keep `pf_core`'s dependency list tiny so it physically cannot leak in.

pub mod input;
pub mod math;
pub mod systems;
pub mod world;

pub use input::{buttons, Input};
pub use math::rng::Rng;
pub use math::{Fx, V2};
pub use world::{ActionState, Fighter, Stage, World};
