---
icon: lucide/swords
---

# pfengine

**pfengine** is an engine for [platform
fighters](https://en.wikipedia.org/wiki/Platform_fighter) in the line of
*Super Smash Bros. Melee*. Games like these get their depth from many small
mechanics interacting, not from a general-purpose physics engine. pfengine is
four Rust crates held to one rule, and that rule decides most of the design.

<div class="grid cards" markdown>

-   :material-rocket-launch: __Run everywhere__

    One Cargo workspace. Desktop and the browser run today from the same
    code, and only `pf_app` carries platform `cfg`s. Android and iOS are
    decided, not built.

-   :material-sync: __Smooth online play__

    GGPO-style **rollback** through GGRS. Local play already runs through the
    rollback session; the network transport (matchbox, WebRTC) comes in
    Phase 3.

-   :material-tune-vertical: __Emergent depth__

    Movement and combat are built from small mechanics run in a fixed order,
    so techniques like the wavedash fall out of the rules instead of being
    scripted. Today each fighter has stick movement, a jump, gravity, and a
    flat floor. The state machine and combat come in Phase 5.

-   :material-language-rust: __Built in Rust__

    Fixed-point determinism, no GC pauses, first-class WASM, and a rollback
    ecosystem (GGRS, matchbox). Rust won over C++, Godot, and Unity; the
    [dev log](devlog.md#2026-06-07-stack-decided-rust) records why.

</div>

## The one rule, and what it forces

Rollback predicts remote inputs and plays on, then rewinds and re-simulates
when the real inputs arrive. That is correct only if every machine computes
**bit-identical** state from the same inputs. So the simulation is a pure
function:

```
new_state = advance(old_state, inputs)
```

That rules out floats, clocks, outside randomness, and rendering inside the
simulation. Three features of the code follow from it:

1. **`pf_core` computes in fixed point.** `Fx` is `I16F16` from the `fixed`
   crate, and `pf_core` depends on `fixed` and `serde` and nothing else.
2. **`World: Clone` is the rollback snapshot.** The state is a flat struct of
   `Copy` data, so a save is one allocation and one memcpy, cheap enough to
   take every tick.
3. **Rendering is a separate crate that only reads.** `pf_render`
   interpolates between two `World`s and never writes back. `pf_app` runs the
   60 Hz loop and owns every platform detail.

Keep the rule and rollback is nearly free: GGRS asks the engine only for a
pure `advance`, a cheap clone, and a checksum. Break it and no netcode can
make rollback correct.

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

    This site records the **design** as it is decided and the **development**
    as it happens. Today the engine is a deterministic core and a local
    N-player demo on desktop and web. Every tick runs through the GGRS
    session, which has no peers yet. Next comes replay recording, then the
    network transport. The [Dev log](devlog.md) is the running record.
