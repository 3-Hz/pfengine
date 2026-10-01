---
icon: lucide/swords
---

# pfengine

**pfengine** is a platform-agnostic engine for building [platform
fighters](https://en.wikipedia.org/wiki/Platform_fighter) in the lineage of
*Super Smash Bros. Melee*. In games like these, depth comes from many small,
interacting mechanics rather than from a general-purpose physics engine.

The engine is built around two goals, and together they decide almost every
architectural choice: run everywhere, and play smoothly online.

<div class="grid cards" markdown>

-   :material-rocket-launch: __Run everywhere__

    One codebase targeting native **macOS, Linux, Windows**, the **browser**
    (WebAssembly + WebGPU/WebGL), and **Android / iOS**. Desktop and the
    browser run today; Android and iOS are decided, not built.

-   :material-sync: __Smooth online play__

    GGPO-style **rollback netcode** for responsive, low-latency matches:
    the requirement that shapes the whole engine. Every tick already runs
    through the GGRS session; the network transport comes in Phase 3.

-   :material-tune-vertical: __Emergent depth__

    Movement and combat built from small composable mechanics (gravity,
    friction, hitstun, knockback, ledges, ECB collision…) that interact to
    produce techniques nobody explicitly scripted.

-   :material-language-rust: __Built in Rust__

    Fixed-point determinism, no GC pauses, first-class WASM and mobile
    targets, and a mature rollback ecosystem. The
    [dev log](devlog.md#2026-06-07-stack-decided-rust) records why Rust won
    over C++, Godot, and Unity.

</div>

## Why this is hard (and why the design looks the way it does)

The most demanding requirement is **rollback netcode**. Rollback predicts
remote inputs, then, when the real inputs arrive, rewinds the game state
and re-simulates the frames it got wrong. For that to be correct, every
machine must compute **bit-identical** results from the same inputs.

That one requirement cascades into three hard constraints:

1. **Deterministic simulation.** No reliance on floating point across
   platforms, no wall-clock time, no unordered iteration, and no outside
   randomness.
2. **Cheap save and restore.** The entire game state must snapshot and
   restore many times per second. In practice GGRS takes a snapshot every
   tick.
3. **A hard sim / render split.** The simulation is a pure function of state
   and inputs; rendering only ever *reads* it.

!!! tip "The one rule everything follows"

    The simulation is a pure function: `new_state = update(old_state, inputs)`.
    In the code, `update` is `World::advance` in `pf_core`. No floats, no
    clocks, no outside randomness, no rendering. Get this right and rollback
    is nearly free. Break it and rollback is impossible.

## Where to go next

- [Architecture overview](architecture/overview.md): the two-world model and
  where its boundary is enforced.
- [Deterministic core](architecture/deterministic-core.md): fixed-point math,
  the fixed timestep, and the serializable world.
- [Rollback netcode](architecture/rollback.md): GGRS, SyncTest, couch +
  online, and the web-netplay transport.
- [Mechanics model](architecture/mechanics.md): how Melee-style depth is
  structured.
- [Building everywhere](guide/builds.md): each target, what the web build
  carries, and CI.
- [Roadmap](roadmap.md): the phased build plan.

!!! note "Status"

    This site documents the **design** as it is decided and the
    **development** as it happens. The engine today is a deterministic core
    and a local N-player demo on desktop and web. Every tick runs through
    the GGRS rollback session, which has no remote peers yet and so never
    rolls back; SyncTest exercises rollback itself in CI. See the
    [Dev log](devlog.md) for the running record.
