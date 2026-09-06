---
icon: lucide/swords
---

# pfengine

**pfengine** is an engine for building [platform
fighters](https://en.wikipedia.org/wiki/Platform_fighter) in the lineage of
*Super Smash Bros. Melee*: games whose depth comes from many small, interacting
mechanics rather than from a general-purpose physics engine. It is four Rust
crates and one rule, and the rule decides nearly everything else.

<div class="grid cards" markdown>

-   :material-rocket-launch: __Run everywhere__

    One Cargo workspace. Desktop and the browser run today from the same
    code, and only `pf_app` carries platform `cfg`s. Android and iOS are
    decided, not built.

-   :material-sync: __Smooth online play__

    GGPO-style **rollback** through GGRS. Local play already runs through the
    rollback session; the network transport (matchbox, WebRTC) is Phase 3.

-   :material-tune-vertical: __Emergent depth__

    Movement and combat built from small mechanics in a fixed order, so
    techniques like the wavedash fall out instead of being scripted. Today: one
    fighter with gravity, a jump, and a flat floor. The state machine and
    combat are Phase 5.

-   :material-language-rust: __Built in Rust__

    Fixed-point determinism, no GC pauses, first-class WASM, and a rollback
    ecosystem (GGRS, matchbox). Chosen over C++, Godot, and Unity; the
    [dev log](devlog.md#2026-06-07-stack-decided-rust) records why.

</div>

## The one rule, and what it forces

Rollback works by predicting remote inputs, then rewinding and re-simulating
when the real ones arrive. That is correct only if every machine computes
**bit-identical** state from the same inputs. So the simulation is a pure
function:

```
new_state = advance(old_state, inputs)
```

No floats, no clocks, no outside randomness, no rendering. Three things in the
code follow from it:

1. **`pf_core` computes in fixed point.** `Fx` is `I16F16` from the `fixed`
   crate, and `pf_core` depends on `fixed` and `serde` alone.
2. **`World: Clone` is the rollback snapshot.** The state is a flat struct of
   `Copy` data, so a save is one memcpy, cheap enough to take several times a
   second.
3. **Rendering is a separate crate that only reads.** `pf_render` interpolates
   between two `World`s and never writes back; `pf_app` runs the 60 Hz loop
   and owns every platform detail.

Get the rule right and rollback is nearly free. Break it and rollback is
impossible.

## Where to go next

- [Architecture overview](architecture/overview.md): the two-world model and
  where its boundary is enforced.
- [Deterministic core](architecture/deterministic-core.md): fixed point, the
  RNG, the flat `World`, the loop, and what catches a violation.
- [Rollback netcode](architecture/rollback.md): the GGRS session, SyncTest,
  couch + online, and the web transport.
- [Mechanics model](architecture/mechanics.md): what runs today and the
  layered design it grows into.
- [Building everywhere](guide/builds.md): each target, what the web build
  carries, and CI.
- [Roadmap](roadmap.md): the phased plan.

!!! note "Status"

    This site documents the **design** as it is decided and the **development**
    as it happens. Today the engine is a deterministic core and a local
    N-player demo on desktop and web, with every tick running through the GGRS
    session and no peers yet. Next: replay recording, then the network
    transport. The [Dev log](devlog.md) is the running record.
