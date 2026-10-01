---
icon: lucide/atom
---

# Mechanics model

This is where "many small mechanics interacting" becomes deep gameplay, and
where most of the engine's development time will go. The depth in Melee-like
games is **emergent**. Nobody explicitly programmed wavedashing or
ledgedashing; they fall out of simple rules interacting.

Today the engine holds a minimal slice of mechanics. This page describes that
slice first, then the design it grows into.

## What runs today

`step_fighter` advances one fighter by one tick. It is the whole mechanics
model so far:

```rust title="crates/pf_core/src/systems/mod.rs"
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
```

The function is pure: it reads the fighter, its input, and the stage, and
nothing else. That is what lets `World::advance` call it for each fighter in
turn and stay pure itself.

The rules are simple. The stick sets horizontal velocity directly every
tick, with no acceleration. A jump needs the ground. Gravity applies only in
the air. The stage is a flat floor with a wall at each end.

`ActionState` has four variants today, and `step_fighter` sets two of them:

```rust title="crates/pf_core/src/world.rs"
/// A character's high-level action state (a stub of Melee's action-state IDs).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ActionState {
    Idle,
    Walk,
    Airborne,
    Hitstun,
}
```

`Walk` and `Hitstun` are declared for the shape to come; nothing sets them
yet.

## Fighters are state machines

!!! note "Decided, not built"

    Each fighter is an **action-state machine**, exactly like Melee's
    action-state IDs. A character is always in exactly one state, and the
    state defines what is possible. The four-variant stub above grows into
    something like this:

    ```rust title="ActionState (design sketch)"
    #[derive(Clone, Copy, PartialEq, Eq)]
    pub enum ActionState {
        Idle,
        Walk,
        Dash,
        Jumpsquat,
        Airborne,
        Attack(AttackId),
        Shield,
        Hitstun,
        LedgeHang,
        // ...dozens more
    }
    ```

    The state machine maps cleanly onto a Rust `enum` plus `match`. Each
    state's per-frame behavior is one arm of the `match`.

## Frame data drives each state

!!! note "Decided, not built"

    Each state is described by **data tables**, not code: which frames have
    active hitboxes, the earliest frame a fighter can act out of it (IASA),
    animation timing, and so on. Keeping this as data is what makes the game
    tunable without recompiling logic.

    ```rust title="AttackData (design sketch)"
    pub struct AttackData {
        pub startup: u8,             // frames before the first active hitbox
        pub active: Range<u8>,       // frames the hitbox is live
        pub iasa: u8,                // interruptible-as-soon-as frame
        pub hitboxes: Vec<Hitbox>,   // damage, angle, knockback growth/base...
    }
    ```

## The layered update order

Every tick, each fighter goes through an explicit, **deterministic** sequence
of layers. Mechanics compose because later layers can override earlier ones,
and the *order* is fixed so the result is reproducible. `step_fighter`
already works this way, with numbered steps in a fixed order.

!!! note "Decided, not built"

    The full design has six layers:

    ```mermaid
    flowchart TB
      I[1 · Read input → state transitions] --> P[2 · Generic physics<br/>gravity · friction · air drag]
      P --> O[3 · State overrides<br/>e.g. wavedash applies air momentum on land]
      O --> C[4 · Collision<br/>ECB vs stage surfaces · ledge detection]
      C --> H[5 · Combat<br/>hitbox vs hurtbox → knockback · hitstun · DI · hitlag]
      H --> R[6 · Resolve<br/>apply velocities, set resulting states]
    ```

    1. **Input → transitions.** The current state decides which inputs are
       valid and what they transition to.
    2. **Generic physics.** Gravity, ground friction, and air drag, applied
       uniformly in fixed point.
    3. **State overrides.** The active state can modify the generic result.
       This is where signature mechanics live.
    4. **Collision.** The character's **ECB** (environmental collision box)
       is resolved against stage surfaces, and ledges are detected here.
    5. **Combat.** Active hitboxes are tested against opponent hurtboxes. On
       a hit, this layer computes knockback (Melee's knockback formula),
       hitstun, DI, and hitlag.
    6. **Resolve.** The accumulated velocity changes are applied and the new
       states committed.

`step_fighter` is a first cut of this order, not a copy of it. Its first step,
stick to horizontal velocity, stands in for layer 1. Gravity is all of layer 2
so far, and the jump is the only state override (layer 3). Collision is a flat
floor and two walls, with no ECB and no ledges, and it runs before the state
label is resolved, as layers 4 and 6 require. Combat, layer 5, arrives in
Phase 5.

One difference in order is worth naming. `step_fighter` applies the jump
*before* gravity, the reverse of layers 2 and 3. That is harmless for now: the
jump needs the fighter on the ground and gravity needs it in the air, so the
two never run in the same tick.

## Why emergence works

Consider the wavedash, which nobody programs directly. None of its pieces
exist yet; the example shows why the fixed order matters.

- *Air dodge* is a state with a directional momentum burst (layer 3).
- *Diagonal-into-ground* means that burst's downward component meets a
  surface during collision (layer 4).
- *Landing* converts the remaining horizontal momentum into a slide governed
  by ordinary ground friction (layers 2 and 6).

Three independent mechanics, in a fixed order, produce a technique with its
own skill curve. The engine's job is to keep each mechanic **small,
orthogonal, and deterministically ordered**. Depth takes care of itself.

## Data layout: skip the ECS at first

`World` is a plain struct that holds a `Vec<Fighter>`, a `Stage`, the frame
count, and the RNG. A full ECS is tempting, but a plain struct with arrays
gives total control over the state's layout, and that control is exactly
what cheap rollback snapshots need (see
[One flat, serializable world](deterministic-core.md#3-one-flat-serializable-world)).
A lightweight ECS such as [`hecs`](https://docs.rs/hecs) stays in reserve,
for a later need that justifies it.
