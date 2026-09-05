---
icon: lucide/swords
---

# pfengine

**pfengine** is a platform-agnostic engine for building [platform
fighters](https://en.wikipedia.org/wiki/Platform_fighter) in the lineage of
*Super Smash Bros. Melee* — games whose depth emerges from many small,
interacting mechanics rather than from a general-purpose physics engine.

It is designed from the ground up around two goals that, together, dictate
almost every architectural decision:

<div class="grid cards" markdown>

-   :material-rocket-launch: __Run everywhere__

    One codebase targeting native **macOS, Linux, Windows**, the **browser**
    (WebAssembly + WebGPU/WebGL), and **Android / iOS**.

-   :material-sync: __Smooth online play__

    GGPO-style **rollback netcode** for responsive, low-latency matches —
    the requirement that shapes the whole engine.

-   :material-tune-vertical: __Emergent depth__

    Movement and combat built from small composable mechanics
    (gravity, friction, hitstun, knockback, ledges, ECB collision…) that
    interact to produce techniques nobody explicitly scripted.

-   :material-language-rust: __Built in Rust__

    Fixed-point determinism, no GC pauses, first-class WASM and mobile
    targets, and a mature rollback ecosystem.

</div>

## Why this is hard (and why the design looks the way it does)

The single most demanding requirement is **rollback netcode**. Rollback works
by predicting remote inputs, then — when the real inputs arrive — rewinding the
game state and re-simulating the missed frames. For that to be correct, every
machine must compute **bit-identical** results from the same inputs.

That one requirement cascades into three hard constraints:

1. **Deterministic simulation** — no reliance on floating point across
   platforms, no wall-clock time, no unordered iteration, no external
   randomness.
2. **Cheap save/restore** — the entire game state must snapshot and restore
   many times per second.
3. **A hard sim / render split** — the simulation is a pure function of state
   and inputs; rendering only ever *reads* it.

!!! tip "The one rule everything follows"

    The simulation is a pure function: `new_state = update(old_state, inputs)`.
    No floats, no clocks, no outside randomness, no rendering. Get this right
    and rollback is nearly free. Break it and rollback is impossible.

## Where to go next

- [Architecture overview](architecture/overview.md) — the two-world model.
- [Deterministic core](architecture/deterministic-core.md) — fixed-point math,
  fixed timestep, the serializable world.
- [Rollback netcode](architecture/rollback.md) — GGRS, SyncTest, and the
  web-netplay transport.
- [Mechanics model](architecture/mechanics.md) — how Melee-style depth is
  structured.
- [Roadmap](roadmap.md) — the phased build plan.

!!! note "Status"

    This site documents the **design** as it is decided and the **development**
    as it happens. The engine today is a deterministic core and a local
    N-player demo on desktop and web; rollback is tested but not yet wired
    into the app. See the [Dev log](devlog.md) for the running record.
