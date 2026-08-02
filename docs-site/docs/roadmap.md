---
icon: lucide/map
---

# Roadmap

Each phase is independently testable and de-risks the next. The ordering is
deliberate: **prove determinism and rollback on a single moving capsule before
building any real fighting mechanics.** That is the opposite of what's tempting,
and it is exactly why this plan avoids the trap that stalls most rollback
projects — building a deep game on an unproven foundation.

## Phase 0 — Scaffold

> **Goal:** a window that clears the screen on desktop *and* web.
> **Proves:** the toolchain and cross-compilation work end to end.

- [x] Cargo workspace with `pf_core`, `pf_net`, `pf_render`, `pf_app`.
- [x] Desktop window via `macroquad` (`wgpu` + `winit` remain the eventual
      target — see [Building everywhere](guide/builds.md)).
- [x] WASM build compiling for `wasm32-unknown-unknown`.
- [ ] That WASM build confirmed running in a browser.

## Phase 1 — Deterministic core skeleton

> **Goal:** one controllable capsule with gravity and flat-ground collision,
> rendered.
> **Proves:** the deterministic core ↔ render split.

- [x] Fixed-point `V2`, deterministic `Rng`.
- [ ] LUT trig — deferred; nothing needs angles until knockback in Phase 5.
- [x] Fixed-timestep loop; flat `World` state.
- [x] One capsule: gravity, ground collision, movement.
- [x] Render with interpolation (macroquad to start).

## Phase 2 — Rollback integration

> **Goal:** `SyncTestSession` green in CI, then local 2-player.
> **Proves:** determinism is real and guarded.

- [x] Wrap `World` behind a GGRS `Config`.
- [x] `cargo test` runs SyncTest and stays green. :material-shield-check:
- [ ] Local two-player on one machine — `pf_app` polls two keyboard players
      today, but steps `World` directly; it doesn't yet run through a GGRS
      session.
- [ ] Replay recording: initial seed + config + per-frame input stream, with periodic checksums. (Foundation for the Phase 6 viewer.)

## Phase 3 — Real netplay

> **Goal:** two instances playing across a network.
> **Proves:** rollback works online.

- [ ] matchbox WebRTC transport + signaling.
- [ ] Tunable input delay / prediction window.
- [ ] Desync detection via checksums in the wild.

## Phase 4 — Controllers & input

> **Goal:** keyboard, standard gamepads, and a native GameCube adapter all map
> to the same `Input`.
> **Proves:** the input-source abstraction holds and analog fidelity survives
> quantization — without ever touching determinism.

- [ ] Input-source abstraction in `pf_app` (platform layer only; keyboard already wired).
- [ ] Standard gamepads: `gilrs` on native, the Gamepad API on web.
- [ ] Native GameCube adapter (WUP-028): USB-HID via `hidapi`/`rusb` on native, WebHID in the browser.
- [ ] Analog calibration: deadzones, notch/edge clamping, deterministic quantization to the `i8` stick fields.
- [ ] Per-player binding + hotplug.

## Phase 5 — The fighter

> **Goal:** real Melee-style combat.
> **Proves:** the actual game feel. *(The long phase.)*

- [ ] Action-state machine + frame-data tables.
- [ ] Hitbox / hurtbox / ECB collision.
- [ ] Knockback, hitstun, DI, hitlag.
- [ ] First playable character + stage.

## Phase 6 — Content & tooling

> **Goal:** fast iteration for design.

- [ ] Character / stage data formats.
- [ ] Animation pipeline.
- [ ] Debug tools: hitbox viewer, frame-step, input display.
- [ ] Replay viewer: load an input-stream replay, scrub + frame-step, and validate playback against the recorded checksums.

## Phase 7 — Ship everywhere

> **Goal:** all platforms + matchmaking.

- [ ] Android / iOS polish.
- [ ] Web netplay hardening.
- [ ] Matchmaking / lobby service.

---

!!! note "Learning Rust alongside"

    You don't need all of Rust up front. Front-load ownership/borrowing,
    `struct`/`enum` + pattern matching (your state machines *are* enums), traits
    (GGRS uses them), and `Result`/`Option`. Defer async, advanced lifetimes,
    and `unsafe`. Phases 0–1 are the on-ramp; by Phase 5 you'll be fluent.
