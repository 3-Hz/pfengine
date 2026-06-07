---
icon: lucide/rewind
---

# Rollback netcode

Rollback netcode hides latency by **predicting** remote inputs and playing the
game immediately, then **rewinding and re-simulating** when the real inputs
arrive. Done right, both players feel like they're playing offline. It is the
requirement that shaped the entire [deterministic core](deterministic-core.md).

## GGRS — the rollback engine

[GGRS](https://github.com/gschup/ggrs) is a pure-Rust reimplementation of GGPO.
You implement its `Config` trait, then drive a session that hands you back
*requests* to fulfill each frame: save the state, sometimes load (rewind), and
advance.

```rust title="pf_net — the rollback loop"
for request in session.advance_frame(/* local inputs */)? {
    match request {
        GgrsRequest::SaveGameState { cell, frame } => {
            cell.save(frame, Some(world.clone()), Some(world.checksum())); // (1)!
        }
        GgrsRequest::LoadGameState { cell, .. } => {
            world = cell.load().unwrap(); // (2)!
        }
        GgrsRequest::AdvanceFrame { inputs } => {
            world.advance(extract(inputs)); // (3)!
        }
    }
}
```

1.  **Save** — snapshot via `clone()`, plus the `checksum()` GGRS uses to detect
    desyncs between peers.
2.  **Load** — rewind: GGRS discovered a misprediction and is rolling back.
3.  **Advance** — run one deterministic tick. May be called several times in a
    single frame while re-simulating mispredicted frames.

That's the whole loop. GGRS handles prediction, the rollback decision, and input
synchronization; you just provide a deterministic `advance` and a cheap
`clone`/`checksum`.

## SyncTest — determinism as a CI gate

The most valuable tool GGRS gives you is `SyncTestSession`. It runs entirely
locally, but **every frame it rolls back N frames, re-simulates, and compares
checksums.** If the simulation is non-deterministic in any way, it panics with
the exact offending frame.

!!! tip "Run SyncTest from day one"

    Wire `SyncTestSession` into CI before building any real mechanics. It turns
    "mysterious desync three weeks from now" into "this commit broke
    determinism." It is the single highest-leverage habit on this project.

```rust title="determinism test"
let mut session = SessionBuilder::<Config>::new()
    .with_num_players(2)
    .with_check_distance(7)         // roll back up to 7 frames each tick
    .start_synctest_session()?;
// Feed inputs, advance frames, assert no checksum mismatch panics.
```

## The web netplay trap (and the fix)

GGRS's built-in socket is **UDP**, which is native-only — **browsers cannot open
raw UDP sockets.** A rollback engine that can't do netplay in the browser would
miss one of pfengine's core targets.

The fix is [**matchbox**](https://github.com/johanhelsing/matchbox): it provides
WebRTC sockets that implement GGRS's socket trait and work on **both native and
web**. So matchbox is the universal transport, with raw UDP available as an
optional native-only fast path later.

```mermaid
flowchart LR
  A[Player A] <-->|WebRTC via matchbox| SS[Signaling server]
  B[Player B] <-->|WebRTC via matchbox| SS
  A <-.->|P2P inputs after handshake| B
```

| Transport | Native | Browser | Use |
| --- | :---: | :---: | --- |
| matchbox (WebRTC) | ✅ | ✅ | **Default — works everywhere** |
| GGRS UDP | ✅ | ❌ | Optional native-only fast path |

## What travels over the wire

Only **inputs** — never game state. Each player's per-frame input is a tiny
bit-packed struct (a few bytes). Because both sides run the identical
deterministic simulation, identical inputs reproduce identical state. This is
what keeps rollback bandwidth tiny and cheating harder.

```rust title="pf_core/input/mod.rs"
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub struct Input {
    pub buttons: u16,  // bitflags: jump, attack, shield, grab, ...
    pub stick_x: i8,   // quantized analog stick
    pub stick_y: i8,
    pub cstick_x: i8,
    pub cstick_y: i8,
}
```
