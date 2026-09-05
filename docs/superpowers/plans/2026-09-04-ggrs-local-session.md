# Local Play Through a GGRS Session — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `pf_app` runs every 60 Hz tick through a GGRS session owned by `pf_net`, built around local handles, on desktop and in the browser.

**Architecture:** A new `pf_net::Session` wraps a GGRS `P2PSession` over a no-op socket and fulfils its save/load/advance requests against `World`. `pf_app` builds one with every handle local, tells `Slots` which slots are local, and calls `advance` once per tick. Two wasm-only fixes (a `getrandom` byte source and a loader stub override) keep the web build working under macroquad's JS glue.

**Tech Stack:** Rust stable (1.96), ggrs 0.11.1, macroquad 0.4.15, getrandom 0.2 (`custom` feature, wasm only), Zensical docs.

**Spec:** `docs/superpowers/specs/2026-09-04-ggrs-local-session-design.md`

## Global Constraints

- Only `pf_app` may contain `#[cfg(target_arch = ...)]` or `#[cfg(target_os = ...)]`. `pf_core`, `pf_net`, `pf_render` stay platform-neutral.
- No `f32`/`f64`, clocks, or unordered iteration in `pf_core`. Nothing in this plan touches `pf_core`.
- `crates/pf_app/web/mq_js_bundle.js` is vendored and must not be edited.
- The four CI gates must stay green after every task: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo build -p pf_app --target wasm32-unknown-unknown`.
- Netplay cap: `MAX_NETPLAY_MACHINES = 4`. Fighters are uncapped everywhere.
- Commit messages: conventional prefix (`feat:`, `docs:`, `test:`), imperative, ending with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.
- Work on branch `feat/ggrs-local-session` (already exists with the spec committed).

## Gotchas an executor will hit

- **Sandbox.** Bash commands run sandboxed. `cargo build --target wasm32-unknown-unknown` can fail with `Operation not permitted` while unpacking a crate into `~/.cargo/registry`; rerun that one command with the sandbox disabled. Binding a local port and `pkill` also need the sandbox off. Writes under `.vscode/` are denied.
- **The browser check needs the web build route.** The native window cannot open in the sandbox and `screencapture` returns the wallpaper. Task 7 gives the exact procedure.
- **A hidden Browser pane does not tick.** Each screenshot forces one render frame. Hold a key by dispatching `keydown` from JavaScript, take two screenshots, then dispatch `keyup`.

---

## File Structure

| File | Responsibility |
| --- | --- |
| `crates/pf_net/src/lib.rs` (modify) | Cap rename; `pub mod session` + re-exports. `GgrsConfig`, `scripted_input`, `run_synctest` stay. |
| `crates/pf_net/src/session.rs` (create) | `NullSocket`, `Session`, `Advanced`, `SessionError`, `PlayerHandle`, and their tests. |
| `crates/pf_app/Cargo.toml` (modify) | `pf_net` dependency; wasm-only `getrandom` with `custom`. |
| `crates/pf_app/src/wasm_entropy.rs` (create) | The `getrandom` byte source, wasm only. |
| `crates/pf_app/src/input.rs` (modify) | `Slots` learns which slots are local. |
| `crates/pf_app/src/main.rs` (modify) | The loop calls `Session::advance`; HUD line for the session. |
| `crates/pf_app/web/index.html` (modify) | Loader stub override before `load()`. |
| `docs-site/docs/*.md`, `README.md` (modify) | Devlog entry, roadmap box, cap wording, loop sketch. |

---

### Task 1: Rename the netplay cap to count machines

**Files:**
- Modify: `crates/pf_net/src/lib.rs`
- Test: `crates/pf_net/src/lib.rs` (inline `#[cfg(test)]` module)

**Interfaces:**
- Consumes: nothing new.
- Produces: `pub const MAX_NETPLAY_MACHINES: usize = 4`, `pub struct TooManyMachines { pub requested: usize, pub max: usize }`, `pub fn check_netplay_machines(num_machines: usize) -> Result<(), TooManyMachines>`. `MAX_NETPLAY_PLAYERS`, `TooManyPlayers`, `check_netplay_players` are gone.

- [ ] **Step 1: Replace the two cap tests**

In `crates/pf_net/src/lib.rs`, replace `netplay_allows_up_to_max_players` and `netplay_rejects_more_than_max_players` inside `mod tests` with:

```rust
    #[test]
    fn netplay_allows_up_to_max_machines() {
        for n in 1..=MAX_NETPLAY_MACHINES {
            assert!(check_netplay_machines(n).is_ok(), "n = {n}");
        }
    }

    #[test]
    fn netplay_rejects_more_than_max_machines() {
        let requested = MAX_NETPLAY_MACHINES + 1;
        assert_eq!(
            check_netplay_machines(requested),
            Err(TooManyMachines {
                requested,
                max: MAX_NETPLAY_MACHINES
            })
        );
    }
```

- [ ] **Step 2: Run the tests to see them fail to compile**

Run: `cargo test -p pf_net`
Expected: compile error, `cannot find value MAX_NETPLAY_MACHINES` (and the function and struct).

- [ ] **Step 3: Rename the constant, error, and check**

In `crates/pf_net/src/lib.rs`, replace everything from the `MAX_NETPLAY_PLAYERS` doc comment through the end of `check_netplay_players` with:

```rust
/// The most machines a netplay session accepts, counting this one. Fighters
/// are uncapped: a machine may own several handles (couch + online). Links
/// and prediction cost scale with machines; re-simulation cost scales with
/// fighters and stays cheap until Phase 5 adds real mechanics.
pub const MAX_NETPLAY_MACHINES: usize = 4;

/// A netplay session asked for more machines than [`MAX_NETPLAY_MACHINES`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TooManyMachines {
    pub requested: usize,
    pub max: usize,
}

impl fmt::Display for TooManyMachines {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "netplay supports at most {} machines, got {}",
            self.max, self.requested
        )
    }
}

impl Error for TooManyMachines {}

/// Gate for the Phase 3 P2P constructor. `num_machines` is the distinct peer
/// addresses plus this machine. Local sessions and SyncTest have no cap.
pub fn check_netplay_machines(num_machines: usize) -> Result<(), TooManyMachines> {
    if num_machines > MAX_NETPLAY_MACHINES {
        return Err(TooManyMachines {
            requested: num_machines,
            max: MAX_NETPLAY_MACHINES,
        });
    }
    Ok(())
}
```

Then update the crate doc comment at the top of the file to:

```rust
//! Rollback netcode wiring (GGRS) and the determinism gate.
//!
//! [`Session`] is what `pf_app` runs every tick through; local play is the
//! all-local case of it. [`run_synctest`] is the CI gate: SyncTest
//! re-simulates past frames every tick and compares checksums, so it fails
//! fast the moment the simulation stops being bit-deterministic. Real P2P
//! transport (matchbox / WebRTC) arrives in Phase 3 and is capped at
//! [`MAX_NETPLAY_MACHINES`] machines; fighters are not capped.
```

(The `[`Session`]` link resolves after Task 2. rustdoc is not a CI gate, so an unresolved intra-doc link for one commit is fine.)

- [ ] **Step 4: Run the tests**

Run: `cargo test -p pf_net`
Expected: all pass (`determinism_holds_under_rollback`, `netplay_allows_up_to_max_machines`, `netplay_rejects_more_than_max_machines`).

- [ ] **Step 5: Commit**

```bash
git add crates/pf_net/src/lib.rs
git commit -m "feat(net): cap netplay by machines, not fighters

Four machines per session; fighters uncapped. Links and prediction cost
scale with machines, re-simulation with fighters, and the sim is cheap
until Phase 5.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 2: `Session::local` and its accessors

**Files:**
- Create: `crates/pf_net/src/session.rs`
- Modify: `crates/pf_net/src/lib.rs` (module declaration and re-exports)
- Test: `crates/pf_net/src/session.rs` (inline `#[cfg(test)]` module)

**Interfaces:**
- Consumes: `crate::GgrsConfig` (`Config` with `Input = pf_core::Input`, `State = pf_core::World`, `Address = usize`), `crate::MAX_NETPLAY_MACHINES` (tests only).
- Produces: `pub type PlayerHandle = usize`; `pub struct Session` with `pub fn local(num_players: usize) -> Result<Session, SessionError>`, `pub fn num_players(&self) -> usize`, `pub fn local_handles(&self) -> &[PlayerHandle]`, `pub fn frame_count(&self) -> u32`, `pub fn confirmed_frame(&self) -> Option<u32>`; `pub struct Advanced { pub frames: u32, pub rolled_back: bool }` (derives `Clone, Copy, PartialEq, Eq, Debug, Default`); `pub enum SessionError { NoPlayers, Ggrs(ggrs::GgrsError) }`. All re-exported from `pf_net`.

- [ ] **Step 1: Create the file with the failing tests and the module wiring**

Create `crates/pf_net/src/session.rs`:

```rust
//! The rollback session `pf_app` runs every tick through.
//!
//! Local play is the all-local case: every handle is local and the socket
//! has no peers, so GGRS starts in the Running state and never rolls back.
//! Phase 3 adds a constructor that takes a transport and remote handles; the
//! app loop does not change.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MAX_NETPLAY_MACHINES;

    #[test]
    fn local_session_marks_every_handle_local() {
        let session = Session::local(3).unwrap();
        assert_eq!(session.num_players(), 3);
        assert_eq!(session.local_handles(), &[0, 1, 2]);
        assert_eq!(session.frame_count(), 0);
        assert_eq!(session.confirmed_frame(), None);
    }

    #[test]
    fn local_session_rejects_zero_players() {
        assert!(matches!(Session::local(0), Err(SessionError::NoPlayers)));
    }

    #[test]
    fn local_session_has_no_player_cap() {
        let n = MAX_NETPLAY_MACHINES * 2;
        assert_eq!(Session::local(n).unwrap().num_players(), n);
    }
}
```

In `crates/pf_net/src/lib.rs`, directly after the `use pf_core::{buttons, Input, World};` line, add:

```rust
pub mod session;

pub use session::{Advanced, PlayerHandle, Session, SessionError};
```

- [ ] **Step 2: Run the tests to see them fail to compile**

Run: `cargo test -p pf_net`
Expected: compile errors, `cannot find type Session`, `SessionError`, `Advanced`, `PlayerHandle` in `session`.

- [ ] **Step 3: Implement the socket, the types, and the constructor**

In `crates/pf_net/src/session.rs`, insert between the module doc comment and `#[cfg(test)]`:

```rust
use std::error::Error;
use std::fmt;

use ggrs::{GgrsError, Message, NonBlockingSocket, P2PSession, PlayerType, SessionBuilder};

use crate::GgrsConfig;

/// Index of a player in the session, `0..num_players`. Slot `n` in `pf_app`
/// drives handle `n`.
pub type PlayerHandle = usize;

/// A socket with no peers: drops every send, never receives. With no remote
/// endpoints GGRS skips synchronization and starts Running.
struct NullSocket;

impl NonBlockingSocket<usize> for NullSocket {
    fn send_to(&mut self, _msg: &Message, _addr: &usize) {}

    fn receive_all_messages(&mut self) -> Vec<(usize, Message)> {
        Vec::new()
    }
}

/// What one call to [`Session::advance`] did.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Advanced {
    /// World ticks run by this call: 0 while waiting, more than 1 after a
    /// rollback.
    pub frames: u32,
    /// Whether GGRS rewound the world before re-simulating.
    pub rolled_back: bool,
}

/// Why a session could not be built or advanced.
#[derive(Debug)]
pub enum SessionError {
    /// A session needs at least one player.
    NoPlayers,
    /// GGRS refused: a bad handle, a missing local input, or a session
    /// misconfiguration. Every case is a contract bug in the caller.
    Ggrs(GgrsError),
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SessionError::NoPlayers => write!(f, "a session needs at least one player"),
            SessionError::Ggrs(e) => write!(f, "ggrs: {e}"),
        }
    }
}

impl Error for SessionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            SessionError::NoPlayers => None,
            SessionError::Ggrs(e) => Some(e),
        }
    }
}

impl From<GgrsError> for SessionError {
    fn from(e: GgrsError) -> Self {
        SessionError::Ggrs(e)
    }
}

/// A GGRS rollback session and the request loop that drives a [`World`]
/// through it.
///
/// [`World`]: pf_core::World
pub struct Session {
    inner: P2PSession<GgrsConfig>,
    local_handles: Vec<PlayerHandle>,
}

impl Session {
    /// Local play: every handle in `0..num_players` is local. There is no
    /// player cap; [`MAX_NETPLAY_MACHINES`] applies to netplay only.
    ///
    /// GGRS defaults stay: input delay 0, prediction window 8, desync
    /// detection off. All three are netplay knobs for Phase 3.
    ///
    /// [`MAX_NETPLAY_MACHINES`]: crate::MAX_NETPLAY_MACHINES
    pub fn local(num_players: usize) -> Result<Session, SessionError> {
        if num_players == 0 {
            return Err(SessionError::NoPlayers);
        }
        let mut builder = SessionBuilder::<GgrsConfig>::new().with_num_players(num_players);
        for handle in 0..num_players {
            builder = builder.add_player(PlayerType::Local, handle)?;
        }
        let inner = builder.start_p2p_session(NullSocket)?;
        Ok(Session {
            inner,
            local_handles: (0..num_players).collect(),
        })
    }

    pub fn num_players(&self) -> usize {
        self.inner.num_players()
    }

    /// The handles this machine supplies input for.
    pub fn local_handles(&self) -> &[PlayerHandle] {
        &self.local_handles
    }

    /// Frames the session has advanced; equals `world.frame` between calls.
    pub fn frame_count(&self) -> u32 {
        // GGRS counts frames as i32 with -1 for "none"; the session starts
        // at 0 and only counts up.
        self.inner.current_frame().max(0) as u32
    }

    /// The newest frame whose inputs are final. `None` before the first one.
    pub fn confirmed_frame(&self) -> Option<u32> {
        let frame = self.inner.confirmed_frame();
        (frame >= 0).then_some(frame as u32)
    }
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p pf_net`
Expected: all pass, including the three new `local_session_*` tests.

- [ ] **Step 5: Check formatting and clippy**

Run: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings`
Expected: no output from fmt; clippy finishes with no warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/pf_net/src/session.rs crates/pf_net/src/lib.rs
git commit -m "feat(net): add Session::local over a no-op socket

An all-local GGRS P2P session: every handle local, no peers, so GGRS
starts Running without a handshake.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 3: `Session::advance`

**Files:**
- Modify: `crates/pf_net/src/session.rs`
- Test: `crates/pf_net/src/session.rs` (inline `#[cfg(test)]` module)

**Interfaces:**
- Consumes: `Session` from Task 2; `crate::scripted_input(handle: usize, frame: u32) -> Input` (tests); `pf_core::World` (`new`, `advance`, `checksum`, `frame`, `PartialEq`).
- Produces: `pub fn advance<I>(&mut self, world: &mut World, local_inputs: I) -> Result<Advanced, SessionError> where I: IntoIterator<Item = (PlayerHandle, Input)>`. Contract: on `Err` or zero frames, `world` is untouched.

- [ ] **Step 1: Add the failing tests**

In `crates/pf_net/src/session.rs`, add one import at the top of `mod tests`, directly after `use crate::MAX_NETPLAY_MACHINES;`:

```rust
    use crate::scripted_input;
```

(`World` and `GgrsError` reach the tests through `use super::*;` once Step 3 imports them.) Then append inside `mod tests`:

```rust
    #[test]
    fn local_session_matches_direct_stepping() {
        for n in [1, 2, 4, 8] {
            let mut session = Session::local(n).unwrap();
            let mut through_session = World::new(n);
            let mut direct = World::new(n);
            let mut inputs = vec![Input::default(); n];

            for frame in 0..300 {
                for (handle, input) in inputs.iter_mut().enumerate() {
                    *input = scripted_input(handle, frame);
                }
                let advanced = session
                    .advance(&mut through_session, inputs.iter().copied().enumerate())
                    .unwrap_or_else(|e| panic!("{n} players, frame {frame}: {e}"));
                assert_eq!(
                    advanced,
                    Advanced {
                        frames: 1,
                        rolled_back: false
                    },
                    "{n} players, frame {frame}"
                );
                direct.advance(&inputs);
                assert_eq!(
                    session.frame_count(),
                    through_session.frame,
                    "{n} players, frame {frame}"
                );
            }

            assert_eq!(through_session, direct, "{n} players");
            assert_eq!(through_session.checksum(), direct.checksum(), "{n} players");
            // Inputs are queued for the frame about to run, so the confirmed
            // frame trails the frame count by one.
            assert_eq!(session.confirmed_frame(), Some(299), "{n} players");
        }
    }

    #[test]
    fn advance_with_a_missing_local_input_is_an_error_and_leaves_the_world_untouched() {
        let mut session = Session::local(2).unwrap();
        let mut world = World::new(2);
        let before = world.clone();

        let result = session.advance(&mut world, [(0, Input::default())]);

        assert!(matches!(
            result,
            Err(SessionError::Ggrs(GgrsError::InvalidRequest { .. }))
        ));
        assert_eq!(world, before);
        assert_eq!(session.frame_count(), 0);
    }

    #[test]
    fn advance_with_an_unknown_handle_is_an_error() {
        let mut session = Session::local(2).unwrap();
        let mut world = World::new(2);

        let result = session.advance(
            &mut world,
            [
                (0, Input::default()),
                (1, Input::default()),
                (7, Input::default()),
            ],
        );

        assert!(matches!(
            result,
            Err(SessionError::Ggrs(GgrsError::InvalidRequest { .. }))
        ));
        assert_eq!(world, World::new(2));
    }
```

- [ ] **Step 2: Run the tests to see them fail to compile**

Run: `cargo test -p pf_net`
Expected: compile error, `no method named advance found for struct Session`.

- [ ] **Step 3: Implement `advance`**

In `crates/pf_net/src/session.rs`, change the `use ggrs::{...}` line to:

```rust
use ggrs::{
    GgrsError, GgrsRequest, Message, NonBlockingSocket, P2PSession, PlayerType, SessionBuilder,
};
```

and add, directly after it:

```rust
use pf_core::{Input, World};
```

Remove the `/// [`World`]: pf_core::World` line under the `Session` doc comment (the type is now imported, so the plain link resolves). Add the scratch buffer to the struct and its constructor:

```rust
pub struct Session {
    inner: P2PSession<GgrsConfig>,
    local_handles: Vec<PlayerHandle>,
    /// Scratch for the advance-frame request; kept to avoid allocating every
    /// tick.
    frame_inputs: Vec<Input>,
}
```

and in `Session::local`, the `Ok(Session { ... })` becomes:

```rust
        Ok(Session {
            inner,
            local_handles: (0..num_players).collect(),
            frame_inputs: Vec::with_capacity(num_players),
        })
```

Then add inside `impl Session`, after `confirmed_frame`:

```rust
    /// One 60 Hz tick. Queues one input per local handle, then fulfils every
    /// request GGRS hands back: save = clone + checksum, load = replace the
    /// world, advance = [`World::advance`] with the inputs GGRS chose.
    ///
    /// When this returns `Err` or `frames == 0`, `world` is untouched: every
    /// error path runs before the first request is fulfilled.
    ///
    /// This is the seam for replay recording: every frame's inputs pass
    /// through the advance-frame arm, and [`Session::confirmed_frame`] says
    /// which of them are final.
    pub fn advance<I>(&mut self, world: &mut World, local_inputs: I) -> Result<Advanced, SessionError>
    where
        I: IntoIterator<Item = (PlayerHandle, Input)>,
    {
        for (handle, input) in local_inputs {
            self.inner.add_local_input(handle, input)?;
        }

        let requests = match self.inner.advance_frame() {
            Ok(requests) => requests,
            // Not this tick: peers are still syncing, or we are too far
            // ahead of them. Both are routine once there are peers.
            Err(GgrsError::NotSynchronized) | Err(GgrsError::PredictionThreshold) => {
                return Ok(Advanced::default());
            }
            Err(e) => return Err(e.into()),
        };

        let mut advanced = Advanced::default();
        for request in requests {
            match request {
                GgrsRequest::SaveGameState { cell, frame } => {
                    cell.save(frame, Some(world.clone()), Some(world.checksum()));
                }
                GgrsRequest::LoadGameState { cell, .. } => {
                    *world = cell
                        .load()
                        .expect("GGRS asked to load a frame it never saved");
                    advanced.rolled_back = true;
                }
                GgrsRequest::AdvanceFrame { inputs } => {
                    self.frame_inputs.clear();
                    self.frame_inputs
                        .extend(inputs.iter().map(|(input, _)| *input));
                    world.advance(&self.frame_inputs);
                    advanced.frames += 1;
                }
            }
        }
        Ok(advanced)
    }
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p pf_net`
Expected: all pass. `local_session_matches_direct_stepping` takes well under a second.

- [ ] **Step 5: Check formatting and clippy**

Run: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean. If `rustfmt` reflows the `advance` signature, accept its layout (`cargo fmt --all` then re-run the check).

- [ ] **Step 6: Commit**

```bash
git add crates/pf_net/src/session.rs
git commit -m "feat(net): Session::advance runs one tick through GGRS

Queues local inputs, fulfils save/load/advance requests, and reports
frames run and whether a rollback happened. Proven equal to direct
stepping over 300 scripted frames at 1, 2, 4, and 8 players.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 4: `pf_app` depends on `pf_net` and still builds for wasm

**Files:**
- Modify: `crates/pf_app/Cargo.toml`
- Create: `crates/pf_app/src/wasm_entropy.rs`
- Modify: `crates/pf_app/src/main.rs` (one `mod` line)
- Modify: `Cargo.lock` (cargo updates it)

**Interfaces:**
- Consumes: nothing from `pf_net` yet; the dependency edge is what this task adds.
- Produces: a wasm build of `pf_app` that links with ggrs in the graph.

- [ ] **Step 1: Add the dependency and watch the wasm build fail**

Replace the `[dependencies]` section of `crates/pf_app/Cargo.toml` (everything after `authors.workspace = true`) with:

```toml
[dependencies]
pf_core.workspace = true
pf_net.workspace = true
pf_render.workspace = true
macroquad.workspace = true
```

Run: `cargo build -p pf_app --target wasm32-unknown-unknown`
Expected: FAIL in `getrandom` with `the wasm*-unknown-unknown targets are not supported by default, you may need to enable the "js" feature`. (If it instead fails with `Operation not permitted` under `~/.cargo/registry`, rerun the command with the sandbox disabled; the getrandom error is the one this step is after.)

- [ ] **Step 2: Add the wasm-only `getrandom` dependency and the byte source**

Append to `crates/pf_app/Cargo.toml`:

```toml

# ggrs -> rand -> getrandom refuses wasm32-unknown-unknown unless told where
# bytes come from. `custom` keeps wasm-bindgen out; see src/wasm_entropy.rs.
[target.'cfg(target_arch = "wasm32")'.dependencies]
getrandom = { version = "0.2", features = ["custom"] }
```

Create `crates/pf_app/src/wasm_entropy.rs`:

```rust
//! Byte source for `getrandom` on the web.
//!
//! ggrs pulls in `rand`, whose `getrandom` refuses to compile for
//! `wasm32-unknown-unknown` unless it is told where bytes come from. Its `js`
//! feature would drag wasm-bindgen into the import table, which macroquad's
//! loader cannot satisfy; the `custom` feature lets us register this instead.
//!
//! Not cryptographic. ggrs uses it for handshake nonces only, and never while
//! the session has no peers. Lives in `pf_app` because only `pf_app` may
//! carry platform `cfg`s.

use std::sync::atomic::{AtomicU64, Ordering};

/// xorshift64* state. Zero means "not seeded yet".
static STATE: AtomicU64 = AtomicU64::new(0);

fn next_u64() -> u64 {
    let mut x = STATE.load(Ordering::Relaxed);
    if x == 0 {
        // First use: seed from the wall clock (seconds, with fraction) and
        // force the low bit so the xorshift state is never zero.
        x = (macroquad::miniquad::date::now() * 1e6) as u64 | 1;
    }
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    STATE.store(x, Ordering::Relaxed);
    x.wrapping_mul(0x2545_F491_4F6C_DD1D)
}

fn fill(dest: &mut [u8]) -> Result<(), getrandom::Error> {
    for chunk in dest.chunks_mut(8) {
        let bytes = next_u64().to_le_bytes();
        chunk.copy_from_slice(&bytes[..chunk.len()]);
    }
    Ok(())
}

getrandom::register_custom_getrandom!(fill);
```

In `crates/pf_app/src/main.rs`, change the `mod input;` line to:

```rust
mod input;
#[cfg(target_arch = "wasm32")]
mod wasm_entropy;
```

- [ ] **Step 3: Build for wasm and lint the wasm-only module**

Run: `cargo build -p pf_app --target wasm32-unknown-unknown`
Expected: `Finished`. `Cargo.lock` now lists `js-sys`, `wasm-bindgen`, and friends; that is ggrs's own wasm32 dependency and is expected.

Run: `cargo clippy -p pf_app --target wasm32-unknown-unknown -- -D warnings`
Expected: clean. (CI's clippy runs native only, so this is the one place `wasm_entropy.rs` gets linted.)

- [ ] **Step 4: Confirm the native gates still pass**

Run: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: clean; the same tests pass as before plus the `pf_net` ones from Tasks 1 to 3.

- [ ] **Step 5: Commit**

```bash
git add crates/pf_app/Cargo.toml crates/pf_app/src/wasm_entropy.rs crates/pf_app/src/main.rs Cargo.lock
git commit -m "build(app): depend on pf_net; give getrandom a byte source on wasm

ggrs's getrandom refuses wasm32-unknown-unknown without a source. The
custom feature keeps wasm-bindgen out of the import table; the source is
a clock-seeded xorshift, non-crypto, never reached with zero peers.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 5: `Slots` learns which slots are local

**Files:**
- Modify: `crates/pf_app/src/input.rs`
- Test: `crates/pf_app/src/input.rs` (inline `#[cfg(test)]` module)

**Interfaces:**
- Consumes: nothing new.
- Produces: `Slots::new(num_players: usize, local: &[usize]) -> Slots` (signature change), `Slots::is_local(&self, slot: usize) -> bool`. `tick` and `source_of` keep their signatures.

- [ ] **Step 1: Add the two failing tests and adapt the existing ones**

In `crates/pf_app/src/input.rs`, inside `mod tests`, add after the `rig` function:

```rust
    /// Every slot local, as local play has it.
    fn all_local(n: usize) -> Vec<usize> {
        (0..n).collect()
    }
```

Change every existing `Slots::new(N)` in the tests to `Slots::new(N, &all_local(N))`. There are seven: `Slots::new(2)` in `slots_start_empty_and_emit_default_input`, `pressing_jump_joins_the_lowest_free_slot`, `an_assigned_source_drives_its_slot`, `holding_jump_is_not_a_second_join`, and `an_assigned_source_cannot_claim_a_second_slot`; `Slots::new(1)` in `the_joining_press_does_not_jump` and `a_full_roster_ignores_further_joins`.

Then append inside `mod tests`, before `keyboard_sources_are_four_distinct_layouts`:

```rust
    #[test]
    fn remote_slots_are_never_claimed() {
        let (cells, mut sources) = rig(1);
        let mut slots = Slots::new(2, &[1]); // slot 0 belongs to another machine
        let mut out = vec![Input::default(); 2];
        cells[0].set(jump());
        slots.tick(&mut sources, &mut out);
        assert!(!slots.is_local(0));
        assert!(slots.is_local(1));
        assert_eq!(slots.source_of(0), None);
        assert_eq!(slots.source_of(1), Some(0));
        assert_eq!(out[0], Input::default()); // a remote slot's input comes over the wire
    }

    #[test]
    fn the_joiner_takes_the_lowest_free_local_slot() {
        let (cells, mut sources) = rig(2);
        let mut slots = Slots::new(3, &[0, 2]);
        let mut out = vec![Input::default(); 3];
        cells[0].set(jump());
        slots.tick(&mut sources, &mut out); // source 0 -> slot 0
        cells[0].set(Input::default());
        cells[1].set(jump());
        slots.tick(&mut sources, &mut out); // source 1 skips remote slot 1
        assert_eq!(slots.source_of(0), Some(0));
        assert_eq!(slots.source_of(1), None);
        assert_eq!(slots.source_of(2), Some(1));
    }
```

- [ ] **Step 2: Run the tests to see them fail to compile**

Run: `cargo test -p pf_app`
Expected: compile error, `this function takes 1 argument but 2 arguments were supplied` on `Slots::new`, and `no method named is_local`.

- [ ] **Step 3: Implement local slots**

In `crates/pf_app/src/input.rs`, replace the `Slots` struct and its `impl` block (everything from `/// Maps player slots to sources.` through the closing brace of `fn is_assigned`) with:

```rust
/// Maps player slots to sources. Slots start empty; a source joins the lowest
/// free *local* slot by pressing jump, and that joining press is swallowed so
/// it does not also jump. Remote slots belong to another machine: they are
/// never claimed here and always emit `Input::default()`, since their input
/// arrives through the session.
pub struct Slots {
    /// slot → source index
    source_of: Vec<Option<usize>>,
    /// slot → driven from this machine?
    local: Vec<bool>,
    /// Last tick's raw poll per source, for edge detection.
    prev: Vec<Input>,
    /// This tick's polls; kept to avoid allocating every tick.
    cur: Vec<Input>,
    /// Sources that joined this tick.
    joined: Vec<bool>,
}

impl Slots {
    /// `local` lists the slots a source on this machine may claim.
    ///
    /// # Panics
    /// If `local` names a slot at or past `num_players` — a contract bug.
    pub fn new(num_players: usize, local: &[usize]) -> Self {
        let mut is_local = vec![false; num_players];
        for &slot in local {
            is_local[slot] = true;
        }
        Slots {
            source_of: vec![None; num_players],
            local: is_local,
            prev: Vec::new(),
            cur: Vec::new(),
            joined: Vec::new(),
        }
    }

    /// Poll every source, join newly pressed unassigned ones, and write one
    /// `Input` per slot into `out` (`default()` for empty and remote slots).
    pub fn tick(&mut self, sources: &mut [Box<dyn InputSource>], out: &mut [Input]) {
        assert_eq!(
            out.len(),
            self.source_of.len(),
            "tick needs one output Input per slot"
        );
        let n = sources.len();
        self.prev.resize(n, Input::default());
        self.cur.clear();
        self.cur.extend(sources.iter_mut().map(|s| s.poll()));
        self.joined.clear();
        self.joined.resize(n, false);

        for src in 0..n {
            let edge =
                self.cur[src].pressed(buttons::JUMP) && !self.prev[src].pressed(buttons::JUMP);
            if !edge || self.is_assigned(src) {
                continue;
            }
            if let Some(free) = self.lowest_free_local_slot() {
                self.source_of[free] = Some(src);
                self.joined[src] = true;
            }
        }

        self.prev.copy_from_slice(&self.cur);

        for (slot, out) in self.source_of.iter().zip(out.iter_mut()) {
            *out = match *slot {
                Some(src) if !self.joined[src] => self.cur[src],
                _ => Input::default(),
            };
        }
    }

    /// The source driving `slot`, if any.
    pub fn source_of(&self, slot: usize) -> Option<usize> {
        self.source_of[slot]
    }

    /// Whether `slot` is driven from this machine.
    pub fn is_local(&self, slot: usize) -> bool {
        self.local[slot]
    }

    fn is_assigned(&self, src: usize) -> bool {
        self.source_of.contains(&Some(src))
    }

    fn lowest_free_local_slot(&self) -> Option<usize> {
        self.source_of
            .iter()
            .zip(&self.local)
            .position(|(taken, &local)| local && taken.is_none())
    }
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p pf_app`
Expected: FAIL to compile in `main.rs` only: `Slots::new(num_players)` there still passes one argument. That is Task 6's job; for now make the minimal edit so the tests can run: in `main.rs` change `let mut slots = Slots::new(num_players);` to `let mut slots = Slots::new(num_players, &(0..num_players).collect::<Vec<_>>());`.

Run again: `cargo test -p pf_app`
Expected: all pass, including `remote_slots_are_never_claimed` and `the_joiner_takes_the_lowest_free_local_slot`.

- [ ] **Step 5: Check formatting and clippy**

Run: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add crates/pf_app/src/input.rs crates/pf_app/src/main.rs
git commit -m "feat(app): Slots is told which slots are local

A source claims the lowest free local slot; remote slots are never
claimed and emit default input, since theirs arrives over the wire.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 6: The loop runs through the session

**Files:**
- Modify: `crates/pf_app/src/main.rs`

**Interfaces:**
- Consumes: `pf_net::{Session, Advanced}` (Tasks 2, 3); `Slots::new(n, &local)`, `Slots::is_local` (Task 5).
- Produces: `pf_app` with no direct call to `World::advance`; a HUD line `session frame N  confirmed M[  rollback]`.

- [ ] **Step 1: Replace `main.rs` above the tests**

Replace everything in `crates/pf_app/src/main.rs` from the top of the file through the closing brace of `async fn main()` (leave `#[cfg(test)] mod tests` untouched) with:

```rust
//! pfengine entry point.
//!
//! Runs the documented fixed-timestep loop: every 60 Hz tick goes through a
//! [`Session`] in `pf_net`, which owns the GGRS rollback session and advances
//! the deterministic [`World`]. Rendering interpolates between ticks for
//! smoothness. Input sources and slot assignment live here in the platform
//! layer (see [`input`]); the simulation only ever sees one [`Input`] per slot.

mod input;
#[cfg(target_arch = "wasm32")]
mod wasm_entropy;

use macroquad::prelude::*;
use pf_core::{Input, World};
use pf_net::{Advanced, Session};

use crate::input::{keyboard_sources, Slots};

/// Seconds per simulation tick (60 Hz).
const TICK: f32 = 1.0 / 60.0;
/// Guard against the "spiral of death" if a frame hitches badly.
const MAX_STEPS_PER_FRAME: u32 = 5;
/// Player count without `--players` — always the case on wasm, which has no argv.
const DEFAULT_PLAYERS: usize = 2;

fn window_conf() -> Conf {
    Conf {
        window_title: "pfengine".to_owned(),
        window_width: 960,
        window_height: 540,
        high_dpi: true,
        ..Default::default()
    }
}

/// Parse `--players N` from the arguments after the program name, ignoring
/// anything else.
///
/// # Panics
/// On a missing, non-numeric, or zero value — a usage error worth failing
/// loudly on.
fn parse_players<I: IntoIterator<Item = String>>(args: I) -> usize {
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        if arg == "--players" {
            let value = args.next().expect("--players needs a value");
            let n: usize = value.parse().expect("--players must be a positive integer");
            assert!(n > 0, "--players must be at least 1");
            return n;
        }
    }
    DEFAULT_PLAYERS
}

#[macroquad::main(window_conf)]
async fn main() {
    let num_players = parse_players(std::env::args().skip(1));
    let mut world = World::new(num_players);
    let mut prev = world.clone();
    let mut acc: f32 = 0.0;

    // Local play: every handle is local. Phase 3 builds the session from a
    // lobby instead; the loop below does not change.
    let mut session = Session::local(num_players).expect("local session");
    let local_handles = session.local_handles().to_vec();
    let mut sources = keyboard_sources();
    let mut slots = Slots::new(num_players, &local_handles);
    let mut inputs = vec![Input::default(); num_players];
    let mut last = Advanced::default();

    loop {
        acc += get_frame_time();

        let mut steps = 0;
        while acc >= TICK && steps < MAX_STEPS_PER_FRAME {
            prev = world.clone();
            slots.tick(&mut sources, &mut inputs);
            match session.advance(&mut world, local_handles.iter().map(|&h| (h, inputs[h]))) {
                Ok(advanced) => last = advanced,
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
        draw_text(
            format!("pfengine - {num_players} players   frame {}", world.frame),
            16.0,
            28.0,
            22.0,
            LIGHTGRAY,
        );
        for slot in 0..num_players {
            let status = match slots.source_of(slot) {
                Some(src) => sources[src].label(),
                None if slots.is_local(slot) => "press jump to join",
                None => "remote",
            };
            draw_text(
                format!("P{}: {status}", slot + 1),
                16.0,
                52.0 + 20.0 * slot as f32,
                18.0,
                pf_render::color_for(slot),
            );
        }
        let confirmed = match session.confirmed_frame() {
            Some(frame) => frame.to_string(),
            None => "-".to_owned(),
        };
        let rollback = if last.rolled_back { "  rollback" } else { "" };
        draw_text(
            format!(
                "session frame {}  confirmed {confirmed}{rollback}",
                session.frame_count()
            ),
            16.0,
            60.0 + 20.0 * num_players as f32,
            18.0,
            LIGHTGRAY,
        );

        next_frame().await;
    }
}
```

- [ ] **Step 2: Build, lint, and test natively**

Run: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: clean; all tests pass (the `parse_players` tests in `main.rs` are unchanged).

- [ ] **Step 3: Confirm the direct step is gone and the wasm build still links**

Run: `grep -n "world.advance\|World::advance" crates/pf_app/src/main.rs; echo "exit: $?"`
Expected: no matches, `exit: 1`.

Run: `cargo build -p pf_app --target wasm32-unknown-unknown`
Expected: `Finished`.

- [ ] **Step 4: Commit**

```bash
git add crates/pf_app/src/main.rs
git commit -m "feat(app): run every tick through the GGRS session

pf_app no longer calls World::advance. Slots is built from the
session's local handles and the HUD shows the session frame, the
confirmed frame, and whether the last tick rolled back.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 7: The web page loads with ggrs in the binary

**Files:**
- Modify: `crates/pf_app/web/index.html`

**Interfaces:**
- Consumes: the wasm build from Task 6.
- Produces: a page that instantiates the module and runs the session in the browser.

- [ ] **Step 1: See the page fail as it is**

Build and stage the wasm next to the page:

```bash
cargo build -p pf_app --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/debug/pf_app.wasm crates/pf_app/web/
```

Serve the directory (this needs the sandbox off, since it binds a port; run it in the background):

```bash
python3 -m http.server 8765 --bind 127.0.0.1 --directory "$PWD/crates/pf_app/web"
```

Open `http://localhost:8765/index.html` in the Browser pane (`preview_start` with that URL) and read the console.
Expected: seven `No __wbindgen_... function in gl.js` warnings, then an error `WebAssembly.instantiate(): Import #7 "__wbindgen_placeholder__": module is not an object or function`. The canvas stays blank.

- [ ] **Step 2: Add the loader override**

In `crates/pf_app/web/index.html`, replace the second `<script>` block (the one containing `load("pf_app.wasm")`) with:

```html
    <script>
        // ggrs pulls js-sys in for wasm32, which leaves imports in modules the
        // glue above does not supply (`__wbindgen_placeholder__`,
        // `__wbindgen_externref_xform__`). They are reached only through the
        // per-peer protocol, and there are no peers yet, so stub them rather
        // than let instantiate() reject the module. This replaces the bundle's
        // env-only stub pass; load() looks the function up by name at call
        // time, so the vendored file is untouched. Goes away when Phase 3
        // moves the web build to the wasm-bindgen pipeline.
        add_missing_functions_stabs = function (module) {
            for (const imp of WebAssembly.Module.imports(module)) {
                if (imp.kind !== "function") continue;
                importObject[imp.module] ??= {};
                if (importObject[imp.module][imp.name] !== undefined) continue;
                console.warn("No " + imp.module + "." + imp.name + " in the JS glue; stubbed");
                importObject[imp.module][imp.name] = function () {
                    throw new Error("stubbed import called: " + imp.module + "." + imp.name);
                };
            }
        };
        load("pf_app.wasm"); // place the built wasm next to this file (see README)
    </script>
```

- [ ] **Step 3: Verify in the browser**

Reload the page in the Browser pane. Then, because a hidden pane does not tick and each screenshot forces one frame, drive it like this (one `browser_batch` works well):

1. `navigate` to `http://localhost:8765/index.html`, `wait` 2 s, `screenshot` at scale 0.5.
2. `read_console_messages`. Expected: seven `... in the JS glue; stubbed` warnings, three `Plugin ... is present in JS bundle` logs, **no** errors.
3. `javascript_tool`: `const c = document.getElementById('glcanvas'); c.focus(); c.dispatchEvent(new KeyboardEvent('keydown', {code:'Space', key:' ', bubbles:true})); 'sent'`
4. `screenshot` twice at scale 0.5.
5. `javascript_tool`: `document.getElementById('glcanvas').dispatchEvent(new KeyboardEvent('keyup', {code:'Space', key:' ', bubbles:true})); 'sent'`
6. `screenshot` once more.

Expected on screen: the top line's `frame N` increases across screenshots; the `P1:` line reads `Arrows + Space` after the keydown; the blue fighter is airborne in the screenshots after it; the last HUD line reads `session frame N  confirmed N-1` with no `rollback`.

If the HUD line is off the bottom of the pane, `resize_window` to 960×540 first and reset to `desktop` when done.

- [ ] **Step 4: Clean up**

Stop the server (the background task; `pkill -f "http.server 8765"` with the sandbox off also works) and remove the staged wasm, which is not gitignored:

```bash
rm crates/pf_app/web/pf_app.wasm
git status --short
```

Expected: only `crates/pf_app/web/index.html` modified.

- [ ] **Step 5: Commit**

```bash
git add crates/pf_app/web/index.html
git commit -m "build(web): stub the wasm-bindgen imports ggrs leaves behind

ggrs's js-sys dependency puts seven imports in modules macroquad's
loader never supplies, so instantiate() rejected the module. Stub every
missing function import, in any module, before load(). The stubs throw
if called, which is safe while there are no peers.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 8: Docs

**Files:**
- Modify: `docs-site/docs/devlog.md`
- Modify: `docs-site/docs/roadmap.md`
- Modify: `docs-site/docs/architecture/rollback.md`
- Modify: `docs-site/docs/architecture/deterministic-core.md`
- Modify: `docs-site/docs/architecture/overview.md`
- Modify: `docs-site/docs/guide/builds.md`
- Modify: `README.md`

**Interfaces:**
- Consumes: the names from Tasks 1 to 7: `pf_net::Session`, `MAX_NETPLAY_MACHINES`, `wasm_entropy.rs`.
- Produces: docs that describe the code as it now is.

- [ ] **Step 1: Devlog entry**

In `docs-site/docs/devlog.md`, insert after the line `A running record of decisions and progress. Newest entries first.` and its blank line:

```markdown
## 2026-09-04 — Local play runs through GGRS

`pf_app` no longer steps `World` directly. Every 60 Hz tick goes through
`pf_net::Session`, which owns the GGRS P2P session and fulfils its save,
load, and advance requests. Local play is the all-local case: every handle
is local and the socket has no peers, so GGRS starts Running and never rolls
back. Phase 3 adds a constructor that takes a transport; the loop does not
change. `Slots` is told which slots are local and never claims a remote one.

- **Same world, proven.** `local_session_matches_direct_stepping` runs 300
  scripted frames through the session and through `World::advance` and
  compares the worlds and checksums, at 1, 2, 4, and 8 players.
- **The web build needed two fixes.** ggrs pulls in `rand`, whose
  `getrandom` refuses `wasm32-unknown-unknown` unless told where bytes come
  from; `pf_app` registers a non-crypto source (`wasm_entropy.rs`) through
  the `custom` feature, keeping wasm-bindgen out. ggrs's `js-sys` dependency
  still leaves seven wasm-bindgen imports in the binary, reached only through
  the per-peer protocol, and macroquad's loader only stubs missing imports
  under `env`. `index.html` now stubs missing function imports in any module;
  the stubs throw if called, which is safe exactly while there are no peers.
  Both go when Phase 3 moves the web build to the wasm-bindgen pipeline.

**Decision: the netplay cap counts machines, not fighters.** At most four
machines per session (`pf_net::MAX_NETPLAY_MACHINES`); fighters are
uncapped. This reverses the fighter cap in the entry below. Links and
prediction cost scale with machines; re-simulation cost scales with fighters,
and the sim is cheap until Phase 5. A fighter ceiling may return once Phase 5
shows the real cost. GGRS fixes the roster when a session starts, so which
machine owns which handles is the lobby's decision, and handle numbers follow
that split rather than join order.

**Next:** replay recording — seed, config, the per-frame input stream, and
periodic checksums (Phase 2).

```

- [ ] **Step 2: Roadmap**

In `docs-site/docs/roadmap.md`:

Replace the Phase 1 line

```markdown
- [x] Any number of fighters (`Vec<Fighter>`); netplay caps at
      `MAX_NETPLAY_PLAYERS = 4` in `pf_net`.
```

with

```markdown
- [x] Any number of fighters (`Vec<Fighter>`). Netplay caps machines, not
      fighters: `MAX_NETPLAY_MACHINES = 4` in `pf_net`.
```

Replace the Phase 2 item

```markdown
- [ ] Local multiplayer through a GGRS session — `pf_app` runs N local players
      today but steps `World` directly. Build the session around a set of
      *local handles*: local play is the case where every handle is local, and
      the same loop later carries couch + online.
```

with

```markdown
- [x] Local multiplayer through a GGRS session, built around a set of *local
      handles*: local play is the case where every handle is local, and the
      same loop later carries couch + online.
```

In Phase 3, replace `- [ ] matchbox WebRTC transport + signaling (≤ 4 players).` with `- [ ] matchbox WebRTC transport + signaling (≤ 4 machines).`, and replace

```markdown
- [ ] Couch + online: several local players per machine in one session. The
      cap counts fighters, not machines; the slot binder claims only local
      handles.
```

with

```markdown
- [ ] Couch + online: several local players per machine in one session. The
      cap counts machines, not fighters; the slot binder already claims only
      local handles.
```

- [ ] **Step 3: Rollback page**

In `docs-site/docs/architecture/rollback.md`:

Change the code-block title `pf_net — the rollback loop` to `pf_net::Session::advance — the rollback loop`.

In "Couch + online", replace

```markdown
- **Local play is the degenerate case** — every handle is local. So `pf_app`
  will run local play through the session loop instead of stepping `World`
  directly, and netplay then only adds a transport. Today it still steps
  `World` directly; see
  [Phase 2](../roadmap.md#phase-2-rollback-integration).
- **`MAX_NETPLAY_PLAYERS = 4` counts fighters, not machines.** Links run
  between machines, so two machines with two players each is one link —
  cheaper than four machines with one each. The cap exists because every peer
  rolls back to the laggiest one and each rollback re-simulates every fighter.
```

with

```markdown
- **Local play is the degenerate case** — every handle is local. That is why
  `pf_app` runs local play through `pf_net::Session` instead of stepping
  `World` directly: netplay then only adds a transport.
- **`MAX_NETPLAY_MACHINES = 4` counts machines, not fighters.** Links run
  between machines and every peer waits on the laggiest one, so the link
  count is what the cap bounds. Re-simulation cost scales with fighters
  instead and stays cheap until Phase 5; a fighter ceiling may return then.
```

and replace

```markdown
The slot binder in `pf_app` will be told which slots are local, so a keyboard
can never claim a remote fighter.
```

with

```markdown
The slot binder in `pf_app` is told which slots are local, so a keyboard can
never claim a remote fighter.
```

- [ ] **Step 4: Deterministic-core page**

In `docs-site/docs/architecture/deterministic-core.md`:

Replace annotation 2 under the `World` sample

```markdown
2.  One `Input` per fighter. Local play allows any number; netplay caps at
    `pf_net::MAX_NETPLAY_PLAYERS` (4) because full-mesh rollback degrades past
    that.
```

with

```markdown
2.  One `Input` per fighter, any number of them. Netplay caps machines
    (`pf_net::MAX_NETPLAY_MACHINES`, 4), not fighters.
```

In the conceptual loop, replace `        world.advance(poll_inputs());` with

```rust
        session.advance(&mut world, poll_inputs()); // GGRS: save / load / advance
```

- [ ] **Step 5: Overview tree, build guide, README**

In `docs-site/docs/architecture/overview.md`, replace the tree line

```
    ├── pf_net/               # GGRS config + SyncTest gate (matchbox transport later)
```

with

```
    ├── pf_net/               # GGRS session + SyncTest gate (matchbox transport later)
```

In `docs-site/docs/guide/builds.md`, inside the "What's actually wired today" note, after the paragraph that ends `it is not gitignored.`, add:

```markdown

        `index.html` also stubs, before calling `load()`, any function import
        the glue does not provide, in any module. ggrs's wasm build carries a
        few wasm-bindgen imports that are reached only with remote peers; the
        stubs throw if called. The stubs and the manual copy both go when the
        web build moves to Trunk.
```

In `README.md`, replace the `pf_net` row of the workspace table with:

```markdown
| `pf_net` | The GGRS session `pf_app` runs every tick through, plus the SyncTest determinism gate. |
```

and replace the two status paragraphs (from `**Phases 0–1 complete**` to the end of the file) with:

```markdown
**Phases 0–1 complete** (LUT trig deferred until knockback needs angles):
deterministic fixed-point core, SyncTest green in CI, and a local N-player demo
that runs on desktop and in the browser.

**Phase 2 in progress:** local play runs through a GGRS session built around
local handles, so netplay later adds only a transport. Netplay will cap at 4
machines; fighters are uncapped. **Next:** replay recording. See the
[roadmap](docs-site/docs/roadmap.md).
```

- [ ] **Step 6: Build the site and grep for leftovers**

Run: `cd docs-site && ../.venv/bin/zensical build --clean && cd ..`
Expected: `No issues found`.

Run: `grep -rn "MAX_NETPLAY_PLAYERS\|check_netplay_players\|TooManyPlayers\|caps at 4 fighters\|cap at 4 fighters" README.md docs-site/docs crates`
Expected: matches only in `docs-site/docs/devlog.md` under the older `2026-09-04 — N players locally, 4 over netplay` entry, which stays as history.

- [ ] **Step 7: Commit**

```bash
git add README.md docs-site/docs
git commit -m "docs: record local play through GGRS and the machine cap

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 9: Final verification and pull request

**Files:** none new.

- [ ] **Step 1: Run the four CI gates from a clean tree**

Run: `git status --short` — expected: empty.

Run: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && cargo build -p pf_app --target wasm32-unknown-unknown`
Expected: all clean.

- [ ] **Step 2: Native smoke run (optional in a sandbox)**

`cargo run -p pf_app -- --players 4` opens a window with four slots; pressing Space joins P1, W joins P2. The HUD's last line reads `session frame N  confirmed N-1`. This cannot run under the sandbox; skip it there and rely on Task 7's browser check.

- [ ] **Step 3: Push and open the PR**

```bash
git push -u origin feat/ggrs-local-session
gh pr create --base main --title "feat: run local play through a GGRS session" --body-file - <<'EOF'
## Summary

Roadmap Phase 2: `pf_app` no longer steps `World` directly. Every tick goes through a new `pf_net::Session` that owns the GGRS P2P session (no-op socket, every handle local) and fulfils its save/load/advance requests. Spec: `docs/superpowers/specs/2026-09-04-ggrs-local-session-design.md`.

- `pf_net::Session::local(n)` + `advance(&mut world, local inputs)`; proven equal to direct stepping over 300 scripted frames at 1, 2, 4, and 8 players.
- `Slots` is told which slots are local and never claims a remote one.
- Web build: ggrs's `getrandom` gets a custom byte source on wasm, and `index.html` stubs the wasm-bindgen imports ggrs leaves behind (safe with no peers; goes with the Phase 3 wasm-bindgen pipeline).
- Netplay cap now counts machines (`MAX_NETPLAY_MACHINES = 4`), fighters uncapped. Decision and reasoning in the devlog.

## Test plan

- [x] fmt, clippy, `cargo test --workspace`, wasm build
- [x] Browser check: page loads with only the expected stub warnings, session frame advances, Space joins P1 and jumps
- [ ] CI green

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
```

- [ ] **Step 4: Watch CI**

Run: `gh pr checks --watch --interval 10`
Expected: the Rust check passes. If it fails, read the log with `gh run view --log-failed`, fix on the branch, push, and watch again.
