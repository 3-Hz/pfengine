---
icon: lucide/rewind
---

# Rollback netcode

Rollback netcode hides latency. Each machine **predicts** the remote inputs
and plays the frame at once, then **rewinds and re-simulates** when the real
inputs arrive. Done right, every player feels as if the game were offline.
Rollback is the requirement that shaped the whole
[deterministic core](deterministic-core.md): a re-simulated frame is only
correct if every machine computes bit-identical state from the same inputs.

## GGRS — the rollback engine

[GGRS](https://github.com/gschup/ggrs) is a pure-Rust reimplementation of
GGPO. `pf_net` implements its `Config` trait, which names three types: what
travels over the wire, what gets snapshotted, and how a peer is addressed.

```rust title="crates/pf_net/src/lib.rs"
impl Config for GgrsConfig {
    type Input = Input;
    type State = World;
    type Address = usize;
}
```

The address is a plain handle index until Phase 3 brings real network
addresses.

`pf_app` never calls `World::advance` itself. Every 60 Hz tick goes through
`pf_net::Session`, which owns a GGRS `P2PSession`. Each frame the session
hands back *requests* to fulfil: save the state, sometimes load an older
state (a rewind), and advance. `Session::advance` queues one input per local
handle, then carries out those requests in order:

```rust title="crates/pf_net/src/session.rs"
        for (handle, input) in local_inputs {
            self.inner.add_local_input(handle, input)?;
        }
        // ...
        for request in requests {
            match request {
                GgrsRequest::SaveGameState { cell, frame } => {
                    cell.save(frame, Some(world.clone()), Some(world.checksum())); // (1)!
                }
                GgrsRequest::LoadGameState { cell, .. } => {
                    *world = cell
                        .load()
                        .expect("GGRS asked to load a frame it never saved"); // (2)!
                    advanced.rolled_back = true;
                }
                GgrsRequest::AdvanceFrame { inputs } => {
                    self.frame_inputs.clear();
                    self.frame_inputs
                        .extend(inputs.iter().map(|(input, _)| *input));
                    world.advance(&self.frame_inputs); // (3)!
                    advanced.frames += 1;
                }
            }
        }
```

1.  **Save**: a snapshot via `clone()`, plus the `checksum()` that GGRS uses
    to detect desyncs between peers.
2.  **Load**: a rewind. GGRS found a misprediction and is rolling back.
3.  **Advance**: one deterministic tick. One call can run it several times
    while GGRS re-simulates mispredicted frames.

That is the whole loop. GGRS handles prediction, the decision to roll back,
and input synchronization. The engine supplies only a deterministic
`advance`, a cheap `clone`, and a `checksum`.

GGRS asks for a save on every `advance_frame`, because its sparse-saving mode
defaults to off. So the engine takes a snapshot every tick.

`Session::advance` reports what it did as `Advanced { frames, rolled_back }`.
When it returns an error or zero frames, the world is untouched, because
every error path runs before the first request is fulfilled. `pf_app` logs
the error and draws the last good state.

Two cases mean "not this tick" once there are peers, and both come back as
zero frames rather than an error:

- `NotSynchronized`: the peers are still synchronizing.
- A session too far ahead of its peers. GGRS 0.11.1 signals this to a P2P
  session by returning requests with no `AdvanceFrame`. `Session::advance`
  also maps the `PredictionThreshold` error to zero frames, but in GGRS
  0.11.1 only the spectator session returns that error.

## SyncTest — determinism as a CI gate

The most valuable tool GGRS provides is `SyncTestSession`. It runs entirely
locally, but it keeps rolling back, re-simulating, and comparing checksums.
`pf_net::run_synctest` drives one with a check distance of 2:

```rust title="crates/pf_net/src/lib.rs"
pub fn run_synctest(num_players: usize, frames: u32) -> Result<(), GgrsError> {
    let mut session = SessionBuilder::<GgrsConfig>::new()
        .with_num_players(num_players)
        .with_check_distance(2)
        .start_synctest_session()?;
```

Once the session is past frame 2, every `advance_frame` rolls back two frames,
re-simulates them, and compares their checksums with the ones recorded the
first time. If the simulation is non-deterministic in any way that changes a
re-simulated frame's checksum, GGRS returns a `MismatchedChecksum` error
naming the offending frame, and the test panics with it:

```rust title="crates/pf_net/src/lib.rs"
    fn determinism_holds_under_rollback() {
        for n in [1, 2, 4, 8] {
            run_synctest(n, 300)
                .unwrap_or_else(|e| panic!("{n} players desynced under rollback: {e}"));
        }
    }
```

The test runs SyncTest for 300 frames at 1, 2, 4, and 8 players. Its inputs
come from `scripted_input`: each fighter's stick flips direction every 20
frames and jump fires every 45, offset by 17 frames per handle. A motionless
game cannot reveal nondeterminism, so the script keeps every fighter moving.

`.github/workflows/rust.yml` runs `cargo test --workspace`, and with it this
test, on every push to `main` and every pull request.

!!! tip "Run SyncTest from day one"

    Wire SyncTest into CI before building any real mechanics. pfengine added
    it in Phase 0, alongside the first physics slice, and CI now runs it. It
    turns "mysterious desync three weeks from now" into "this commit broke
    determinism", and it is the single highest-leverage habit on this
    project.

## The web netplay trap (and the fix)

GGRS's built-in socket is **UDP**, which is native-only: **browsers cannot
open raw UDP sockets.** A rollback engine that cannot play over the network
in a browser would miss one of pfengine's core targets.

!!! note "Decided, not built"

    The fix is [**matchbox**](https://github.com/johanhelsing/matchbox). It
    provides WebRTC sockets that implement GGRS's socket trait and work on
    **both native and web**. So matchbox is the universal transport, with
    raw UDP available later as an optional native-only fast path. This lands
    in Phase 3.

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

The web build already links `ggrs` today, with no transport. What it carries
to build and load in the browser is on
[Building everywhere](../guide/builds.md#what-the-web-build-carries).

## Couch + online

A GGRS session is a set of player handles, each **local** or **remote**. This
machine supplies the input for its local handles, and the inputs for remote
handles arrive over the network. GGRS wants one input per *local* handle
before each `advance_frame`; nothing else changes.

### Local play is the degenerate case

In local play every handle is local. `Session::local(n)` registers each
handle in `0..n` as `PlayerType::Local` and starts the `P2PSession` over a
`NullSocket`, a socket that drops every send and never receives. With no
remote endpoints, GGRS skips synchronization, starts in the Running state,
and never rolls back. The GGRS defaults stay: input delay 0, prediction
window 8, and desync detection off. All three are netplay knobs for Phase 3.

That is why `pf_app` runs local play through `pf_net::Session` instead of
stepping `World` directly: netplay then only adds a transport. Until Phase 2,
`pf_app` did call `World::advance` directly. Routing every tick through the
session means Phase 3 can add a transport and remote handles without
touching the app loop.

The session does not change the game.
`local_session_matches_direct_stepping` runs 300 scripted frames at 1, 2, 4,
and 8 players, once through the session and once through `World::advance`,
and asserts that the worlds and their checksums are equal.

### The slot binder knows which slots are local

`Session::local_handles()` lists the handles this machine supplies, and
`pf_app` passes that list to `Slots::new`. A source that presses jump claims
the lowest free *local* slot, so a keyboard can never claim a remote fighter.
The binder always emits `Input::default()` for a remote slot, because that
slot's input arrives through the session, and the HUD labels it "remote".

### The cap counts machines, not fighters

`MAX_NETPLAY_MACHINES = 4` in `pf_net`. `check_netplay_machines` takes the
number of distinct peer addresses plus this machine, and it will gate the
Phase 3 constructor. Local sessions and SyncTest have no cap.

Links run between machines, and every peer waits on the laggiest one, so the
link count is what the cap bounds. Re-simulation cost scales with fighters
instead, and it stays cheap until Phase 5 adds real mechanics. A fighter
ceiling may return then. The machine cap reverses an earlier cap of 4
fighters (`MAX_NETPLAY_PLAYERS`); the
[dev log](../devlog.md#2026-09-04-local-play-runs-through-ggrs) records the
reversal.

### Two machines, four fighters

!!! note "Decided, not built"

    Phase 3 adds the constructor that registers remote handles. Two machines
    playing doubles register the same four handles from opposite sides:

    | Handle | Machine A | Machine B |
    | :---: | --- | --- |
    | 0, 1 | local | remote |
    | 2, 3 | remote | local |

    GGRS fixes the roster when a session starts. So the lobby decides which
    machine owns which handles, and handle numbers follow that split rather
    than the order in which players join.

## What travels over the wire

Only **inputs**, never game state. Each player's input for one frame is a
tiny bit-packed struct of a few bytes:

```rust title="crates/pf_core/src/input.rs"
/// Button bitflags packed into [`Input::buttons`].
pub mod buttons {
    pub const JUMP: u16 = 1 << 0;
    pub const ATTACK: u16 = 1 << 1;
    pub const SHIELD: u16 = 1 << 2;
    pub const GRAB: u16 = 1 << 3;
    pub const SPECIAL: u16 = 1 << 4;
}

/// One player's input for a single 60 Hz tick.
///
/// Implements `Serialize`/`Deserialize` (required by GGRS for the
/// network-transmitted input type). Analog sticks are quantized to
/// `i8` (`-127..=127`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize)]
pub struct Input {
    pub buttons: u16,
    pub stick_x: i8,
    pub stick_y: i8,
    pub cstick_x: i8,
    pub cstick_y: i8,
}
```

Both sides run the identical deterministic simulation, so identical inputs
reproduce identical state. That keeps rollback bandwidth tiny and makes
cheating harder.

The serde derives are there because GGRS requires them: in version 0.11,
`Config::Input` must be `Serialize + DeserializeOwned + Default`. Deriving
`bytemuck::Pod` instead was the alternative, and that requirement ruled it
out ([dev log](../devlog.md#2026-06-07-phase-0-scaffold-complete)). The same
decision turned on `fixed`'s `serde` feature, although nothing fixed-point
is serialized today.

## Input sources

Where an `Input` *comes from* is the platform layer's job, not the sim's.
`pf_app` defines the seam:

```rust title="crates/pf_app/src/input.rs"
pub trait InputSource {
    fn poll(&mut self) -> Input;
    /// Short name for the HUD.
    fn label(&self) -> &str;
}
```

`poll` takes `&mut self` because gamepad backends pump events when polled.
Today `keyboard_sources()` returns four layouts: Arrows + Space, A D + W,
J L + I, and Numpad 4 6 + 8. Each drives `stick_x` to ±110 with its left
and right keys and sets the jump button with its third key. `Slots::tick`
polls every source each tick. A rising edge on jump from an unassigned
source claims the lowest free local slot, and the binder swallows that press
so that joining does not also jump.

Every source lives entirely in `pf_app` and is reduced to the same quantized
`Input` before a single tick runs. Only `Input` ever crosses into `pf_core`,
so no controller, however exotic, can affect determinism.

**Decided, not built** (Phase 4). Two more kinds of source follow the
keyboard. One is a standard gamepad, through
[`gilrs`](https://github.com/gabomdq/gilrs) on native and the
[Gamepad API](https://developer.mozilla.org/docs/Web/API/Gamepad_API) on web.
The other is a **native GameCube adapter** (the WUP-028), read over USB-HID
natively and through
[WebHID](https://developer.mozilla.org/docs/Web/API/WebHID_API) in the
browser. Analog calibration (deadzones, notch and edge clamping, the
Melee-style coordinate feel) happens in `pf_app` too, **before** quantization
to the `i8` stick fields.

## Replays — determinism's other dividend

!!! note "Decided, not built"

    The same property that makes rollback cheap makes **replays nearly
    free**. The sim is a pure function of its inputs, so a replay is just:

    ```
    initial seed + match config + the per-frame input stream
    ```

    Played through the identical deterministic `advance`, that stream
    reproduces the match **bit for bit**: the [Slippi](https://slippi.gg)
    model.

    - The files are tiny: a few bytes per frame, and no game state.
    - The per-frame `checksum()` already used for desync detection doubles
      as a playback validator.
    - A sim-version hash in the header keeps engine changes from silently
      invalidating old replays.

    That makes replays a first-class tool for gameplay analysis,
    frame-stepping, and desync debugging, for the cost of writing the input
    stream to disk. Recording is the next step in Phase 2, and the viewer
    comes in Phase 6.

The seam for the recorder already exists. Every frame's inputs pass through
the advance-frame arm of `Session::advance`, and `Session::confirmed_frame()`
says which of them are final.
