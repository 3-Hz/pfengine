//! Simulation systems.
//!
//! Mechanics are applied in a fixed, explicit order each tick so the result is
//! deterministic and so later layers can override earlier ones (which is how
//! emergent techniques arise). Phase 0 implements a minimal slice of this:
//! input → horizontal velocity → jump → gravity → integrate → collision →
//! resolve. Combat layers (hitboxes, knockback, hitstun) arrive in Phase 5.

use crate::input::{buttons, Input};
use crate::math::Fx;
use crate::world::{ActionState, Fighter, Stage};

// Physics constants, expressed directly in fixed-point bits so they are `const`
// and identical on every platform. `bits = value * 2^16` for `I16F16`.
/// Downward acceleration per tick (≈ 0.5 px/frame²).
pub const GRAVITY: Fx = Fx::from_bits(32_768);
/// Horizontal ground/air speed at full stick (≈ 3.0 px/frame).
pub const MOVE_SPEED: Fx = Fx::from_bits(196_608);
/// Initial upward velocity of a jump (≈ 8.0 px/frame).
pub const JUMP_VELOCITY: Fx = Fx::from_bits(524_288);
/// Magnitude of a fully deflected analog stick (127.0).
pub const STICK_MAX: Fx = Fx::from_bits(127 << 16);

/// Advance a single fighter by one tick. Pure: depends only on its inputs.
pub fn step_fighter(f: &mut Fighter, input: Input, stage: &Stage) {
    // 1. Input → horizontal velocity.
    let dir = Fx::from_num(input.stick_x as i32) / STICK_MAX;
    f.vel.x = dir * MOVE_SPEED;
    if dir > Fx::ZERO {
        f.facing_right = true;
    } else if dir < Fx::ZERO {
        f.facing_right = false;
    }

    let on_ground = f.pos.y <= stage.floor_y;

    // 2. Jump (a state override on the generic physics below).
    if on_ground && input.pressed(buttons::JUMP) {
        f.vel.y = JUMP_VELOCITY;
    }

    // 3. Generic physics: gravity while airborne.
    if !on_ground {
        f.vel.y -= GRAVITY;
    }

    // 4. Integrate.
    f.pos = f.pos + f.vel;

    // 5. Collision against the stage.
    if f.pos.y < stage.floor_y {
        f.pos.y = stage.floor_y;
        if f.vel.y < Fx::ZERO {
            f.vel.y = Fx::ZERO;
        }
    }
    if f.pos.x < stage.left {
        f.pos.x = stage.left;
    }
    if f.pos.x > stage.right {
        f.pos.x = stage.right;
    }

    // 6. Resolve the action state label.
    f.state = if f.pos.y <= stage.floor_y {
        ActionState::Idle
    } else {
        ActionState::Airborne
    };
}
