---
icon: lucide/notebook-pen
---

# Dev log

A running record of decisions and progress. Newest entries first.

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
  A third lies in wait: ggrs's per-peer code calls `instant::Instant::now()`,
  which panics on wasm without the `instant/wasm-bindgen` feature. All three
  go when Phase 3 moves the web build to the wasm-bindgen pipeline (ggrs's
  `wasm-bindgen` feature, Trunk).

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

## 2026-09-04 — N players locally, 4 over netplay

`World` no longer hardcodes two fighters.

- **`players: Vec<Fighter>`.** `World::new(n)` spawns `n` fighters spread
  across the stage; `advance(&[Input])` takes one input per fighter. Zero is a
  valid empty world; 100 works. This keeps a future "one player vs. 100 bots"
  mode open.
- **Netplay caps at 4 fighters.** `pf_net::MAX_NETPLAY_PLAYERS` and
  `check_netplay_players()` gate the Phase 3 P2P builder. Every peer rolls
  back to the laggiest one and each rollback re-simulates every fighter.
  SyncTest is local and now runs at 1, 2, 4, and 8 players.
- **Input sources + press-to-join.** `pf_app::input` has an `InputSource`
  trait, four keyboard layouts, and `Slots`: slots start empty, and pressing
  jump on any source claims the lowest free slot. Any source can drive any
  slot; gamepads plug in later with no new binding code.
  `cargo run -p pf_app -- --players 4`.
- **Web build runs in a browser.** Phase 0's last box: the manual-copy web
  build (see [Building everywhere](guide/builds.md)) loads, ticks, joins P1 on
  Space, and moves on the arrows. `crates/pf_app/web/mq_js_bundle.js` is
  macroquad's JS glue, vendored from the pinned crate so the page fetches no
  third-party script. It carries one patch: a `var register_plugin`
  declaration the upstream `quad_net` chunk still lacks.
- **Rust CI.** `.github/workflows/rust.yml` runs `cargo fmt --check`, clippy
  with `-D warnings`, `cargo test --workspace` (the SyncTest determinism
  gate), and the `wasm32-unknown-unknown` build on every push to `main` and
  every pull request. macroquad needs no apt packages on the Linux runner:
  miniquad opens X11, GL, and ALSA with `dlopen` at runtime, so a headless
  build links nothing.

**Decision: the "no heap indirection" rule was over-broad.** It bundled a
determinism rule (no hash-ordered containers) with a performance heuristic
(no per-entity boxes). A contiguous `Vec` of `Copy` data is one allocation and
one memcpy per snapshot — and GGRS already boxes every saved state in an
`Arc<Mutex<_>>`.
[Deterministic core §4](architecture/deterministic-core.md#4-one-flat-serializable-world)
now states the rule as meant.

**Decision: couch + online.** A machine may own several of a session's
handles — two machines playing doubles each register two handles as local and
two as remote. GGRS supports this as-is (one `add_local_input` per local
handle). So the Phase 2 session is built around a set of local handles, with
local play as the all-local case, and the slot binder must know which slots
are local. The cap counts fighters, not machines; links run between machines,
so 2×2 is one link. See
[Rollback — couch + online](architecture/rollback.md#couch-online).

**Next:** wire `pf_net` into the live app so local play runs through a GGRS
session built around local handles (Phase 2).

## 2026-06-07 — Phase 0 scaffold complete

The Rust workspace is up and the foundation is verified.

- **Workspace:** `pf_core`, `pf_net`, `pf_render`, `pf_app` (see
  [architecture](architecture/overview.md)).
- **Deterministic core:** fixed-point `Fx`/`V2`, a state-seeded `Rng`, the
  serializable `World` with `advance()` + `checksum()`, and a minimal physics
  slice (stick movement, jump, gravity, floor collision).
- **Determinism gate is green:** `cargo test -p pf_net` runs a 300-frame GGRS
  `SyncTestSession` and passes — the simulation is bit-deterministic under
  rollback, exactly as the
  [rollback design](architecture/rollback.md#synctest-determinism-as-a-ci-gate)
  requires. This guardrail is in place from day one.
- **Runs on desktop:** `cargo run -p pf_app` opens a 60 Hz fixed-timestep window
  with two interpolated fighters on a stage.
- **Web build compiles:** `cargo build -p pf_app --target wasm32-unknown-unknown`.

**Decisions made during scaffolding:**

- **GGRS uses serde, not bytemuck.** v0.11's `Config::Input` requires
  `Serialize + DeserializeOwned + Default`, so `Input` derives serde (and
  `fixed` gets its `serde` feature) rather than `bytemuck::Pod`.
- **`pf_app` doesn't depend on `pf_net` yet.** It was unused in Phase 0 and
  pulled `ggrs → rand → getrandom`, which needs extra wasm config. It returns in
  Phase 3 with the matchbox transport and the `getrandom` js feature.
- **Wasm linker flag.** macroquad/miniquad's host functions (`sapp_*`, `gl*`,
  `fs_*`) come from JS glue at runtime; recent `rust-lld` needs
  `--allow-undefined` (set in `.cargo/config.toml`) to emit them as imports.
- **Toolchain:** updated to Rust 1.96 + the `wasm32-unknown-unknown` target.

**Next:** Phase 1 deepening / Phase 2 — wire `pf_net` into the live app for local
two-player rollback. See the [Roadmap](roadmap.md).

## 2026-06-07 — Stack decided: Rust

After weighing C++ / Godot / Unity / Rust against the project's hardest
requirement (cross-platform rollback with a custom deterministic physics
model), we chose **Rust**.

**Why:**

- Fixed-point determinism is trivial to enforce; no GC pauses.
- First-class WASM and mobile targets via `wgpu` + `winit`.
- A mature rollback ecosystem ([GGRS](https://github.com/gschup/ggrs)) and a
  WebRTC transport ([matchbox](https://github.com/johanhelsing/matchbox)) that
  solves browser netplay.

**Trade-off accepted:** Rust is a new language coming from a .NET / web
background — a real learning curve, but the hard parts of this project
(determinism, rollback, the mechanics model) are equally hard in any language,
so we invest the learning where it compounds.

**Architecture locked in:**

- Strict [two-world split](architecture/overview.md) — pure deterministic sim
  vs. read-only presentation.
- [`pf_core`](architecture/deterministic-core.md) with no rendering/OS deps as a
  compiler-enforced determinism wall.
- [SyncTest in CI](architecture/rollback.md#synctest-determinism-as-a-ci-gate)
  from day one.

**Next:** Phase 0 — scaffold the Cargo workspace and get a window clearing the
screen on desktop and web. See the [Roadmap](roadmap.md).

## 2026-06-07 — Documentation site stood up

Created this Zensical site (`docs-site/`) to document the design as it's decided
and development as it happens. Structure: vision → architecture deep-dives →
building → roadmap → this log.
