---
icon: lucide/cpu
---

# The deterministic core

`pf_core` keeps one promise: **the same inputs produce bit-identical state on
every machine.** Rollback relies on it every time it re-simulates a frame, and
SyncTest checks it in CI. Three mechanisms keep the promise and one loop
drives them. The last section lists what catches a break.

## 1. Fixed-point math

`Fx` is the engine's only scalar type: the [`fixed`](https://docs.rs/fixed)
crate's `I16F16`, with 16 integer bits and 16 fractional bits. That gives a
range of ±32 768 at a resolution of 1/65 536.

```rust title="crates/pf_core/src/math/mod.rs"
/// The engine-wide fixed-point scalar: 16 integer bits, 16 fractional bits.
pub type Fx = I16F16;

/// A 2D fixed-point vector. Used for positions, velocities, and offsets.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct V2 {
    pub x: Fx,
    pub y: Fx,
}
```

Floating point is the classic silent desync. One `f32` expression can round
differently across CPUs, compilers, and the WASM runtime, and rollback only
notices when checksums disagree. A fixed-point number is an integer with an
agreed binary point, and integer arithmetic gives the same answer everywhere.

`I16F16` is enough for screen-space physics: the stage is 400 units wide, and
the fastest thing moves 8 units a tick. The type alias is the swap point. If a
subsystem needs headroom, moving to the 64-bit `I32F32` is a one-line change.

Two consequences show in the code:

- **Constants are raw bits.** `Fx::from_bits` is a `const fn` and
  `Fx::from_num` is not, so the physics constants are written as bit patterns
  with the value in the comment (`bits = value × 2^16`).

    ```rust title="crates/pf_core/src/systems/mod.rs"
    /// Downward acceleration per tick (≈ 0.5 px/frame²).
    pub const GRAVITY: Fx = Fx::from_bits(32_768);
    /// Horizontal ground/air speed at full stick (≈ 3.0 px/frame).
    pub const MOVE_SPEED: Fx = Fx::from_bits(196_608);
    /// Initial upward velocity of a jump (≈ 8.0 px/frame).
    pub const JUMP_VELOCITY: Fx = Fx::from_bits(524_288);
    ```

- **Overflow is close.** `World::new` divides the stage width by the player
  count *before* multiplying by the index, because `width * (n + 1)`
  overflows past about 80 players. The release profile keeps
  `overflow-checks = true` (`Cargo.toml`), so an `Fx` addition or subtraction
  that overflows panics instead of wrapping into a wrong value that every
  machine would agree on. Multiplication and division are not covered:
  `fixed` 1.31 checks them with `debug_assert!`, which release builds drop,
  so in release they wrap.

**Decided, not built: trig by lookup table.** Melee launches at fixed angles,
so a `sin`/`cos` table indexed by an integer angle is both deterministic and
true to the original. `f32::sin` is the rejected route. Nothing needs an angle
until knockback in Phase 5, so the table waits.

## 2. Deterministic randomness

`Rng` is a xorshift64\* generator that holds one `u64`. It lives inside
`World`, and `advance` steps it once per tick.

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
    // ...
    /// A fixed-point value in `[0, 1)`.
    #[inline]
    pub fn next_fx(&mut self) -> Fx {
        // Use the high 16 bits as the fractional part of an I16F16.
        let frac = (self.next_u32() >> 16) as i32;
        Fx::from_bits(frac)
    }
}
```

`rand::thread_rng()` and OS entropy are out because they differ per machine
by design. `Rng` is seeded from state and advanced only inside `advance`, so
it is part of what rollback saves and restores, and a rewound frame draws the
same numbers again. The `seed | 1` matters because an all-zero xorshift state
stays zero forever.

Today `World::new` seeds it with a constant, and no system draws from it.
`checksum()` does not hash it either. Nothing is lost yet: with a constant
seed and one step per tick, the RNG state is a function of `frame`, which the
checksum does hash.

## 3. One flat, serializable world

```rust title="crates/pf_core/src/world.rs"
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct World {
    pub players: Vec<Fighter>,
    pub stage: Stage,
    pub frame: u32,
    pub rng: Rng,
}
```

`#[derive(Clone)]` *is* the rollback snapshot. When GGRS asks for a save,
`pf_net` answers with `world.clone()`. `Fighter` and `Stage` are `Copy`, so
the clone is one allocation and one memcpy however many fighters there are.
GGRS asks for a save every tick, so that cost is paid 60 times a second.

`advance` takes one `Input` per fighter, for any number of fighters. Zero is
a valid empty world, and a hundred works. The count is asserted because `zip`
would otherwise truncate silently, and a mismatch is a contract bug, not a
runtime condition.

```rust title="crates/pf_core/src/world.rs"
    pub fn advance(&mut self, inputs: &[Input]) {
        assert_eq!(
            inputs.len(),
            self.players.len(),
            "advance needs exactly one Input per fighter"
        );
        for (fighter, &input) in self.players.iter_mut().zip(inputs) {
            systems::step_fighter(fighter, input, &self.stage);
        }
```

`checksum()` is a 128-bit FNV-1a hash. It covers the fighter count, then each
fighter's position, velocity, state, and facing, then the stage and the
frame, always in that order. It lets a desync fail a test on the frame where
it happens, rather than surface weeks later as a mystery; see
[SyncTest](rollback.md#synctest-determinism-as-a-ci-gate).

Two things stay out of `World`, for different reasons:

- **Hash-ordered containers** (`HashMap`, `HashSet`). Iteration order differs
  between processes, so peers running the same inputs diverge.
- **Per-entity boxes** (`Vec<Box<_>>`, `Rc` graphs, linked lists). A clone
  becomes N mallocs and N cache misses, every tick.

The first is a determinism rule, the second a cost rule. An earlier "no heap
indirection" rule bundled the two and was retired as over-broad
([dev log, 2026-09-04](../devlog.md#2026-09-04-n-players-locally-4-over-netplay)):
one allocation per snapshot is fine, and GGRS already stores every saved state
behind an `Arc<Mutex<_>>`.

## The fixed-timestep loop

The simulation advances in whole 60 Hz ticks. `pf_app` adds real elapsed time
to an accumulator, runs one tick for each whole `TICK` the accumulator holds,
and hands the leftover fraction to the renderer.

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
```

Whole ticks mean the same inputs produce the same steps at any frame rate. The
accumulator holds `f32` wall-clock time, which is allowed because it lives in
`pf_app` and never enters the sim. `prev` keeps the previous tick so
`pf_render` can interpolate by `alpha`.

The step cap handles a hitch. Without it, one long frame would run dozens of
ticks and fall further behind each time. After five steps the loop drops the
backlog instead: the game skips ahead rather than stalls.

Each tick goes through `session.advance`, not `World::advance`; the
[rollback page](rollback.md#ggrs-the-rollback-engine) says why.

## What catches a violation

Each rule below blocks a classic desync source. The last column is honest
about coverage: SyncTest re-simulates inside one process, so it cannot see
anything that differs only *between* machines.

| Rule | What goes wrong | What catches it |
| --- | --- | --- |
| No `f32`/`f64` in `pf_core` | One expression rounds differently across CPUs and WASM | Review, plus `pf_core`'s dependency list (`fixed`, `serde`), which pulls in no float-based math. The compiler does not forbid `f32` here. |
| No `HashMap`/`HashSet` iteration in sim logic | Order differs per process | Review; peers, once Phase 3 turns desync detection on. |
| No `Instant::now()` or system time in `advance` | A rewound frame sees a different clock | SyncTest: the re-simulated frame runs at a different wall time. |
| No threads reordering sim work | Operation order varies from run to run | Review; SyncTest when a reorder changes a frame. |
| RNG seeded from state, advanced in `advance` | A rewound frame draws different numbers | SyncTest. |
| Everything the sim reads lives in `World` | Rollback restores a partial state | SyncTest: the case it was built for. |

SyncTest runs in `cargo test --workspace` on every push to `main` and every
pull request (`.github/workflows/rust.yml`). The
[rollback page](rollback.md#synctest-determinism-as-a-ci-gate) explains how
it works.
