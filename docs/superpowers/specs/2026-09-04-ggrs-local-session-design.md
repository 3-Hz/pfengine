# Local play through a GGRS session

Date: 2026-09-04. Status: approved design, awaiting implementation plan.
Roadmap: Phase 2, "Local multiplayer through a GGRS session".

## Goal

`pf_app` stops stepping `World` directly and runs every tick through a GGRS
session that lives in `pf_net`. Local play is the all-local case of that
session: every player handle is local and the socket has no peers. Phase 3
adds a transport and remote handles without touching the app loop.

Success means: the demo plays as it does today on desktop and in the browser,
`pf_app` contains no call to `World::advance`, and a test proves that the
session yields the same world as direct stepping.

## Decisions carried in

- **Couch + online.** One machine may own several handles. The session is
  built around a set of local handles; the slot binder is told which slots
  are local. (Devlog, 2026-09-04.)
- **The netplay cap counts machines, not fighters.** At most four machines
  per session; fighters are uncapped. This reverses the fighter cap recorded
  earlier on 2026-09-04. Links and prediction cost scale with machines;
  re-simulation cost scales with fighters, and the sim is cheap until Phase 5.
  A fighter ceiling may return once Phase 5 shows the real cost.
- **Spike result.** `pf_app` can depend on `pf_net` and still build and run in
  the browser under macroquad's loader, given two fixes described below.
  GGRS starts a P2P session in the Running state when it has no remote or
  spectator endpoints, so no handshake is needed for local play.

## Not in scope

Replay recording, a session trait, a SyncTest-backed live determinism mode,
remote handles, input delay, desync detection, GGRS event handling, and any
change to the vendored `mq_js_bundle.js`.

## pf_net

### Cap rename

```rust
pub const MAX_NETPLAY_MACHINES: usize = 4;
pub struct TooManyMachines { pub requested: usize, pub max: usize }
pub fn check_netplay_machines(num_machines: usize) -> Result<(), TooManyMachines>;
```

Replaces `MAX_NETPLAY_PLAYERS`, `TooManyPlayers`, and `check_netplay_players`.
Only the Phase 3 constructor calls it, with distinct peer addresses plus one.

### Session

Lives in `crates/pf_net/src/session.rs`; `lib.rs` re-exports `Session`,
`Advanced`, `SessionError`, and `PlayerHandle`.

```rust
pub type PlayerHandle = usize;

pub struct Session {
    inner: ggrs::P2PSession<GgrsConfig>,
    local_handles: Vec<PlayerHandle>,
    frame_inputs: Vec<Input>, // scratch for the advance-frame request
}

pub struct Advanced {
    /// World ticks run by this call: 0 while waiting, more than 1 after a rollback.
    pub frames: u32,
    pub rolled_back: bool,
}

pub enum SessionError {
    NoPlayers,
    Ggrs(ggrs::GgrsError),
}

impl Session {
    /// Local play: every handle in `0..num_players` is local. No player cap.
    pub fn local(num_players: usize) -> Result<Session, SessionError>;
    pub fn num_players(&self) -> usize;
    pub fn local_handles(&self) -> &[PlayerHandle];
    /// Frames the session has advanced; equals `world.frame` between calls.
    pub fn frame_count(&self) -> u32;
    /// The newest frame whose inputs are final. `None` before the first.
    pub fn confirmed_frame(&self) -> Option<u32>;
    /// One 60 Hz tick.
    pub fn advance<I>(&mut self, world: &mut World, local_inputs: I) -> Result<Advanced, SessionError>
    where
        I: IntoIterator<Item = (PlayerHandle, Input)>;
}
```

`SessionError` implements `Display`, `Error`, and `From<GgrsError>`.

`Session::local(n)`:

1. `n == 0` returns `Err(SessionError::NoPlayers)`.
2. Builds `SessionBuilder::<GgrsConfig>::new().with_num_players(n)`, adds
   `PlayerType::Local` for each handle, and starts a P2P session over
   `NullSocket`. GGRS defaults stay: input delay 0, prediction window 8,
   desync detection off.

`NullSocket` is a private `NonBlockingSocket<usize>` that drops sends and
returns no messages. `GgrsConfig::Address` stays `usize` until Phase 3.

`advance`:

1. Calls `add_local_input(handle, input)` for each pair. GGRS rejects a handle
   that is not local or out of range; that becomes `Err`.
2. Calls `advance_frame()`.
   - `Err(NotSynchronized)` and `Err(PredictionThreshold)` mean "not this
     tick" once peers exist. They return `Ok(Advanced { frames: 0, rolled_back: false })`.
   - Any other error returns `Err`.
3. Fulfils the requests in order:
   - `SaveGameState { cell, frame }`: `cell.save(frame, Some(world.clone()), Some(world.checksum()))`.
   - `LoadGameState { cell, .. }`: `*world = cell.load().expect(...)`. A
     missing state is a GGRS invariant failure and panics, as the SyncTest
     harness already does.
   - `AdvanceFrame { inputs }`: copy the inputs into `frame_inputs` and call
     `world.advance(&frame_inputs)`. Counts one frame.
4. Returns `frames` and whether any load request was seen.

Contract: when `advance` returns `Err` or zero frames, `world` is untouched.
All error paths are before the first request is fulfilled.

The advance-frame arm is the seam for the Phase 2 replay recorder: every
frame's inputs pass through it, and `confirmed_frame` says which are final.

`run_synctest` and `scripted_input` stay as they are. The SyncTest session is
a different GGRS session type and remains the CI determinism gate.

## pf_app

### Manifest

`pf_net` returns as a dependency. For wasm only:

```toml
[target.'cfg(target_arch = "wasm32")'.dependencies]
getrandom = { version = "0.2", features = ["custom"] }
```

`Cargo.lock` gains ggrs's wasm32 dependencies (`js-sys`, `wasm-bindgen`);
that is expected.

### Byte source for wasm

`crates/pf_app/src/wasm_entropy.rs`, compiled only for `wasm32`. Registers a
`getrandom` custom implementation: xorshift64* state in a static, seeded on
first use from `macroquad::miniquad::date::now()`. Not cryptographic. ggrs
uses it for handshake nonces only, and never with zero peers. It lives in
`pf_app` because only `pf_app` may carry platform `cfg`s.

### Slots

`Slots::new(num_players: usize, local: &[PlayerHandle])`. A slot is local if
its index is in `local`. Pressing jump on an unassigned source claims the
lowest free **local** slot; remote slots are never claimed and always emit
`Input::default()`. `Slots::is_local(slot) -> bool` for the HUD, which labels
a remote slot "remote". `source_of` is unchanged.

### Loop

```rust
let mut session = Session::local(num_players).expect("local session");
let local_handles = session.local_handles().to_vec();
let mut slots = Slots::new(num_players, &local_handles);
let mut last = Advanced { frames: 0, rolled_back: false };

// per tick, inside the existing accumulator loop:
prev = world.clone();
slots.tick(&mut sources, &mut inputs);
match session.advance(&mut world, local_handles.iter().map(|&h| (h, inputs[h]))) {
    Ok(a) => last = a,
    Err(e) => error!("session: {e}"), // world untouched, prev == world
}
```

The step cap and accumulator do not change. `World::advance` is no longer
called from `pf_app`.

### HUD

One added line under the player list: `session frame N  confirmed M`, with
`  rollback` appended when `last.rolled_back`. `M` shows `-` while
`confirmed_frame()` is `None`.

### Web page

Before `load("pf_app.wasm")`, `index.html` reassigns the global
`add_missing_functions_stabs` to a function that walks
`WebAssembly.Module.imports(module)` and, for every missing **function**
import in any module, installs a stub that throws
`"stubbed import called: <module>.<name>"`. This replaces the bundle's
env-only pass; `load()` looks the function up by name at call time, so the
vendored bundle is unchanged.

Why: ggrs's wasm32 `js-sys` dependency leaves seven imports in
`__wbindgen_placeholder__` and `__wbindgen_externref_xform__`, reached only
through the per-peer protocol. Without stubs `WebAssembly.instantiate`
rejects the module. The comment in `index.html` says this and that the
override goes when Phase 3 moves the web build to the wasm-bindgen pipeline.

## Tests

Written before the code they cover.

pf_net:

- `local_session_matches_direct_stepping`: for 1, 2, 4, and 8 players, 300
  frames of `scripted_input` through `Session::local` equal the same inputs
  through `World::advance`: worlds equal, checksums equal, every call returns
  one frame and no rollback, `frame_count()` equals `world.frame`.
- `local_session_marks_every_handle_local`.
- `local_session_rejects_zero_players`.
- `local_session_has_no_player_cap`: `MAX_NETPLAY_MACHINES * 2` players.
- `advance_with_a_missing_local_input_is_an_error_and_leaves_the_world_untouched`.
- `advance_with_an_unknown_handle_is_an_error`.
- `netplay_allows_up_to_max_machines`, `netplay_rejects_more_than_max_machines`
  (renamed from the player versions).
- `determinism_holds_under_rollback` unchanged.

pf_app:

- `remote_slots_are_never_claimed`: `Slots::new(2, &[1])`, one source presses
  jump, slot 1 is taken and slot 0 stays empty.
- `the_joiner_takes_the_lowest_free_local_slot`: `Slots::new(3, &[0, 2])`,
  two sources join in turn and take 0 then 2.
- Existing Slots tests use an `all_local(n)` helper for the new constructor.
- `parse_players` tests unchanged.

## Docs in the same change

- Devlog: a 2026-09-04 entry for local play through GGRS, the loader stubs and
  why they are safe, and the cap reversal with its reasoning.
- Roadmap: check the Phase 2 session box; Phase 3 reads "≤ 4 machines".
- `architecture/rollback.md`: the two "will" sentences in couch + online back
  to the present tense; the cap bullet counts machines.
- `architecture/deterministic-core.md`: the annotation on `advance` and the
  conceptual loop, which now shows the session call.
- `architecture/overview.md`: the `pf_net` tree line.
- `guide/builds.md`: the web section describes the loader override and when
  it goes.
- README: status names Phase 2 progress and the machine cap; next step is
  replay recording.

## Verification

1. `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo test --workspace`, `cargo build -p pf_app --target wasm32-unknown-unknown`.
2. Browser check: copy the wasm next to `index.html`, serve the directory,
   open it in the Browser pane, confirm the HUD shows the session frame
   advancing, and that Space joins P1 and jumps. No console errors beyond
   the expected stub warnings.
3. `cargo run -p pf_app -- --players 4` behaves as before.
4. One PR against `main` from `feat/ggrs-local-session`; CI green.
