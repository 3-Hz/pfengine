---
icon: lucide/cpu
---

# The deterministic core

Everything in `pf_core` exists to protect one guarantee: **the same inputs
produce bit-identical state on every machine.** This page covers the four
pillars that make that true.

## 1. Fixed-point math, not floats

Cross-platform floating point is the classic way rollback silently desyncs:
different CPUs, compilers, and especially WASM can produce slightly different
results for the same operation. We sidestep the problem entirely by using
**fixed-point** integers via the [`fixed`](https://docs.rs/fixed) crate.

Start with `I16F16` (32-bit: ~±32k range with 1/65536 precision — plenty for
screen-space physics), and move to 64-bit `I32F32` only if a subsystem needs
more headroom.

```rust title="pf_core/math/vec.rs"
use fixed::types::I16F16;

pub type Fx = I16F16; // (1)!

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct V2 {
    pub x: Fx,
    pub y: Fx,
}
```

1.  One type alias for the whole engine's scalar. Swapping precision later is a
    one-line change here.

!!! warning "Watch multiplication overflow"

    Fixed-point multiply can overflow the backing integer. Use the `fixed`
    crate's widening / saturating operations in hot paths rather than the naive
    `*` when values can grow large.

## 2. Deterministic trig via lookup tables

Knockback in Melee uses fixed launch angles, so trig is a natural fit for
**lookup tables** — which are both deterministic *and* authentic to how the
original game worked. Index a precomputed `sin`/`cos` table by an integer angle
(e.g. a `u16` representing 65536 steps around the circle) instead of calling
`f32::sin`.

## 3. Deterministic randomness

No `rand::thread_rng()`, no OS entropy. A tiny PRNG (xorshift / PCG) seeded from
**sim state** and advanced inside the simulation, so every machine draws the
same sequence.

```rust title="pf_core/math/rng.rs"
#[derive(Clone, Copy)]
pub struct Rng(u64); // part of the world state, advanced only inside update()

impl Rng {
    pub fn next_u32(&mut self) -> u32 {
        // xorshift64* — deterministic everywhere
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        ((self.0.wrapping_mul(0x2545F4914F6CDD1D)) >> 32) as u32
    }
}
```

## 4. One flat, serializable world

Rollback clones the entire state several times per second, so it must be cheap
to copy. Prefer fixed-size arrays and struct-of-arrays over `HashMap`s and heap
indirection.

```rust title="pf_core/world/mod.rs"
#[derive(Clone)] // (1)!
pub struct World {
    pub players: [Fighter; 2],
    pub stage: Stage,
    pub frame: u32,
    pub rng: Rng,
}

impl World {
    /// The pure update. This is the entire simulation.
    pub fn advance(&mut self, inputs: [Input; 2]) {
        /* ... */
        self.frame += 1;
    }

    /// Hash of the full state — used for desync detection and SyncTest.
    pub fn checksum(&self) -> u64 {
        /* ... */
        0
    }
}
```

1.  `#[derive(Clone)]` *is* the rollback snapshot mechanism. Keep this type POD
    enough that cloning it is a cheap `memcpy`-like operation.

The `checksum()` is what turns "mysterious desync three weeks from now" into a
test failure on the exact frame — see [Rollback netcode](rollback.md).

## The fixed-timestep loop

The simulation always advances in whole 60 Hz ticks. The renderer decouples
from it by accumulating real elapsed time and interpolating the leftover.

```rust title="conceptual loop (lives in pf_app, not pf_core)"
const TICK: Duration = Duration::from_nanos(16_666_667); // 60 Hz
let mut acc = Duration::ZERO;
loop {
    acc += frame_time();          // real wall-clock delta (presentation only)
    while acc >= TICK {
        world.advance(poll_inputs());
        acc -= TICK;
    }
    let alpha = acc.as_secs_f32() / TICK.as_secs_f32();
    render(&world, &prev_world, alpha); // interpolate; render never mutates sim
}
```

## Rust determinism checklist

Internalize these now — each one is a classic desync source:

- [ ] No `f32` / `f64` anywhere in `pf_core`.
- [ ] No `HashMap`/`HashSet` iteration in sim logic (order isn't stable — use
      arrays or `BTreeMap`).
- [ ] No `Instant::now()` / system time inside `update()`.
- [ ] No threads that can reorder simulation work.
- [ ] RNG seeded only from sim state, advanced only inside `update()`.
- [ ] `SyncTestSession` green in CI (see next page).
