---
icon: lucide/cpu
---

# The deterministic core

Everything in `pf_core` exists to keep one guarantee: **the same inputs
produce bit-identical state on every machine.** Rollback relies on it every
time it re-simulates a frame, and SyncTest checks it in CI.

Three mechanisms keep the guarantee:

1. Fixed-point math instead of floats.
2. A random number generator that lives inside the game state.
3. One flat world that is cheap to copy.

A fixed-timestep loop in `pf_app` drives them. The page ends with a checklist
of the classic ways to break the guarantee.

## 1. Fixed-point math, not floats

`Fx` is the engine's only scalar type. It is `I16F16` from the
[`fixed`](https://docs.rs/fixed) crate: a 32-bit integer read as 16 integer
bits and 16 fractional bits. That gives a range of about ±32 768 at a
resolution of 1/65 536.

```rust title="crates/pf_core/src/math/mod.rs"
pub use fixed::types::I16F16;

/// The engine-wide fixed-point scalar: 16 integer bits, 16 fractional bits.
pub type Fx = I16F16; // (1)!

/// A 2D fixed-point vector. Used for positions, velocities, and offsets.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct V2 {
    pub x: Fx,
    pub y: Fx,
}
```

1.  One type alias for the whole engine's scalar. Swapping precision later is
    a one-line change here.

Floating point is the classic way rollback desyncs silently. Different
CPUs, different compilers, and especially WASM can give slightly different
results for the same `f32` operation, and nothing notices until two
checksums disagree. A fixed-point number is an integer with an agreed binary
point, and integer arithmetic gives the same bits everywhere. So `pf_core`
avoids the problem entirely rather than managing it.

`I16F16` is plenty for screen-space physics. The stage runs from −200 to 200,
and the fastest thing in the game, a jump, starts at 8 units per tick. The
plan is to move to the 64-bit `I32F32` only if some subsystem needs more
headroom.

Two consequences show up in the code.

**Constants are written as raw bits.** `Fx::from_bits` is a `const fn` and
`Fx::from_num` is not, so the physics constants in `systems/mod.rs` are bit
patterns. The comment above them gives the rule, `bits = value * 2^16`, and
each doc comment gives the value:

```rust title="crates/pf_core/src/systems/mod.rs"
/// Downward acceleration per tick (≈ 0.5 px/frame²).
pub const GRAVITY: Fx = Fx::from_bits(32_768);
/// Horizontal ground/air speed at full stick (≈ 3.0 px/frame).
pub const MOVE_SPEED: Fx = Fx::from_bits(196_608);
/// Initial upward velocity of a jump (≈ 8.0 px/frame).
pub const JUMP_VELOCITY: Fx = Fx::from_bits(524_288);
```

**Overflow is never far away.** Sixteen integer bits run out quickly.
`World::new` spreads the fighters evenly across the stage, and it divides
before it multiplies for exactly this reason:

```rust title="crates/pf_core/src/world.rs"
        // Divide before multiplying: `width * (n + 1)` overflows I16F16 past
        // ~80 players.
        let step = (stage.right - stage.left) / Fx::from_num(num_players as i32 + 1);
```

The workspace `Cargo.toml` keeps `overflow-checks = true` in the release
profile, because deterministic overflow behavior matters to the sim. An `Fx`
addition or subtraction that overflows therefore panics instead of quietly
wrapping. Multiplication and division are not covered, as the warning below
explains.

!!! warning "Watch multiplication overflow"

    A fixed-point multiply can overflow the backing integer, and in a release
    build it does so silently. `fixed` 1.31 guards `*` and `/` with
    `debug_assert!` (in its `src/arith.rs`), and release builds drop debug
    assertions, so an overflowing product or quotient wraps. The release
    profile's `overflow-checks = true` catches `Fx` addition and subtraction
    only. In hot paths where values can grow large, use the `fixed` crate's
    widening or saturating operations instead of the plain `*`.

### Trig by lookup table

!!! note "Decided, not built"

    Knockback in Melee launches at fixed angles, so trig suits **lookup
    tables**. A table is deterministic, and it is also true to how the
    original game worked. The plan is a precomputed `sin`/`cos` table indexed
    by an integer angle, for example a `u16` that splits the circle into
    65 536 steps, instead of calling `f32::sin`.

    Nothing needs an angle until knockback arrives in Phase 5, so the table
    waits until then.

## 2. Deterministic randomness

`Rng` is a xorshift64\* generator that holds a single `u64`. It is a field of
`World`, so every snapshot carries it, and `World::advance` steps it once per
tick.

```rust title="crates/pf_core/src/math/rng.rs"
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
```

`rand::thread_rng()` and OS entropy are ruled out, because they differ from
machine to machine by design. The design asked instead for a tiny PRNG, such
as xorshift or PCG, seeded from **sim state** and advanced only inside the
simulation. Then every machine draws the same sequence. Because `Rng` lives
in `World`, rollback saves and restores it along with everything else, so a
re-simulated frame draws the same numbers it drew the first time.

The `seed | 1` in `Rng::new` matters because an all-zero xorshift state stays
zero forever.

Today `World::new` seeds the generator with the constant
`0x9E37_79B9_7F4A_7C15`, and no system draws from it yet. `World::checksum()`
does not hash it. Nothing escapes the checksum so far: with a constant seed
and one step per tick, the RNG state is a function of `frame`, and the
checksum does hash `frame`.

## 3. One flat, serializable world

`World` is the entire game state, and `World::advance` is the entire
simulation:

```rust title="crates/pf_core/src/world.rs"
#[derive(Clone, PartialEq, Eq, Hash, Debug)] // (1)!
pub struct World {
    pub players: Vec<Fighter>,
    pub stage: Stage,
    pub frame: u32,
    pub rng: Rng,
}
// ...
impl World {
    // ...
    pub fn advance(&mut self, inputs: &[Input]) { // (2)!
        assert_eq!(
            inputs.len(),
            self.players.len(),
            "advance needs exactly one Input per fighter"
        );
        for (fighter, &input) in self.players.iter_mut().zip(inputs) {
            systems::step_fighter(fighter, input, &self.stage);
        }
        // ...
        let _ = self.rng.next_u32();
        self.frame += 1;
    }
    // ...
}
```

1.  `#[derive(Clone)]` *is* the rollback snapshot mechanism. When GGRS asks
    for a save, `pf_net` answers with `world.clone()`. `World` has to stay
    POD (plain old data) enough that a clone is a cheap, `memcpy`-like copy.
2.  One `Input` per fighter, for any number of fighters. Netplay caps
    machines (`pf_net::MAX_NETPLAY_MACHINES`, 4), not fighters.

"Serializable" here means that a snapshot is a plain copy. `World` does not
derive serde's `Serialize`; GGRS stores the clone itself.

**Why the copy is cheap.** `Fighter` and `Stage` are `Copy`, so the state is
contiguous `Copy` data. Cloning `World` costs one allocation for the
`Vec<Fighter>` and one memcpy, however many fighters there are. That matters
because GGRS asks for a save on every `advance_frame` (its sparse-saving mode
defaults to off), so the clone runs every tick, 60 times a second.

**Why `advance` asserts.** It takes exactly one `Input` per fighter. Zero
fighters is a valid empty world, and a hundred works. A count mismatch is a
contract bug, not a runtime condition, and without the assert `zip` would
truncate silently.

**What the checksum covers.** `World::checksum()` is a 128-bit FNV-1a hash.
It encodes, always in the same order, the fighter count; each fighter's
position, velocity, action state, and facing; the stage bounds; and the
frame. Nothing in it iterates an unordered container. The checksum is what
turns "mysterious desync three weeks from now" into a test failure on the
exact frame; see
[Rollback netcode](rollback.md#synctest-determinism-as-a-ci-gate).

Two things would break the flat world, for different reasons:

- **Hash-ordered containers** (`HashMap`, `HashSet`). Iteration order
  differs between peers, so the same inputs produce different results: a
  desync. This is a determinism rule.
- **Per-entity boxes** (`Vec<Box<_>>`, `Rc` graphs, linked lists). A clone
  becomes N mallocs and N cache misses, every tick. This is a cost rule.

One heap allocation per snapshot is fine. GGRS already stores every saved
state behind an `Arc<Mutex<_>>` (its `GameStateCell`), so a snapshot is boxed
anyway. An earlier rule, "no heap indirection", bundled the two rules above
into one. It was retired as over-broad, because a contiguous `Vec` of `Copy`
data costs one allocation and one memcpy per snapshot
([dev log, 2026-09-04](../devlog.md#2026-09-04-n-players-locally-4-over-netplay)).

## The fixed-timestep loop

The simulation always advances in whole 60 Hz ticks. The renderer decouples
from it: `pf_app` adds the real elapsed time to an accumulator, runs one tick
for each whole `TICK` the accumulator holds, and hands the leftover fraction
to the renderer for interpolation.

```rust title="crates/pf_app/src/main.rs"
/// Seconds per simulation tick (60 Hz).
const TICK: f32 = 1.0 / 60.0;
/// Guard against the "spiral of death" if a frame hitches badly.
const MAX_STEPS_PER_FRAME: u32 = 5;
```

```rust title="crates/pf_app/src/main.rs"
        acc += get_frame_time();
        // ...
        let mut steps = 0;
        while acc >= TICK && steps < MAX_STEPS_PER_FRAME {
            prev = world.clone();
            slots.tick(&mut sources, &mut inputs);
            match session.advance(&mut world, local_handles.iter().map(|&h| (h, inputs[h]))) {
                Ok(advanced) => rolled_back |= advanced.rolled_back,
                // The world is untouched on an error, so prev == world and
                // nothing jumps on screen.
                Err(e) => error!("session: {e}"),
            }
            acc -= TICK;
            steps += 1;
        }
        // If we hit the step cap, drop the backlog rather than spiral.
        if steps == MAX_STEPS_PER_FRAME {
            acc = 0.0;
        }

        let alpha = (acc / TICK).clamp(0.0, 1.0);

        clear_background(Color::from_rgba(18, 18, 24, 255));
        pf_render::draw_world(&world, &prev, alpha);
```

Each step does four things:

1. Keeps the previous state in `prev`, for interpolation.
2. Polls the input sources through the slot binder (`Slots::tick`).
3. Runs one tick through `session.advance`, which fulfils GGRS's save, load,
   and advance requests. The loop never calls `World::advance` itself; the
   [rollback page](rollback.md#ggrs-the-rollback-engine) explains why.
4. Takes one `TICK` off the accumulator.

Whole ticks mean the same inputs produce the same steps at any frame rate.
The accumulator holds `f32` wall-clock time from `get_frame_time()`, and that
is allowed because it lives in `pf_app` and never enters the sim. After the
steps, `alpha` is the fraction of a tick left over, and
`pf_render::draw_world` interpolates between `prev` and `world` by it.
Rendering only reads the sim and never mutates it.

The step cap handles a bad hitch. Without it, one long frame would queue
dozens of ticks, take longer to run them, and fall further behind each time:
the "spiral of death" the comment names. After five steps the loop drops the
backlog instead. The owed ticks are never simulated, so game time falls behind
wall time by that much, and the frame does not stall.

## Determinism checklist

Each item is a classic desync source:

- [ ] No `f32` / `f64` anywhere in `pf_core`. Float rounding differs across
      CPUs, compilers, and WASM.
- [ ] No `HashMap` / `HashSet` iteration in sim logic. The order isn't
      stable; use arrays or a `BTreeMap`.
- [ ] No `Instant::now()` or other system time inside `World::advance`. A
      re-simulated frame would see a different clock than the first run did.
- [ ] No threads that can reorder simulation work.
- [ ] The RNG is seeded only from sim state and advanced only inside
      `World::advance`.
- [ ] `SyncTestSession` stays green in CI (see
      [Rollback netcode](rollback.md#synctest-determinism-as-a-ci-gate)).

The compiler checks none of the first five. `pf_core` has `std` like any other
crate, so `f32` and `std::time` compile there. What keeps them out is
`pf_core`'s dependency list (`fixed` and `serde`, nothing else), the rule that
only `pf_app` carries platform `cfg`s, review against this list, and SyncTest.
SyncTest catches anything that changes a re-simulated frame's checksum, which
covers every field of `World` except `rng` (see section 2). It runs inside one
process, though, so it cannot see a difference that only shows up between two
machines, such as float rounding on another CPU.
