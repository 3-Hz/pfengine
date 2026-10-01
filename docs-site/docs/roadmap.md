---
icon: lucide/map
---

# Roadmap

Each phase can be tested on its own and lowers the risk of the next. The
order is deliberate: **prove determinism and rollback on a single moving
capsule before building any real fighting mechanics.** That is the opposite
of the tempting order, and it is exactly how this plan avoids the trap that
stalls most rollback projects: building a deep game on an unproven
foundation.

## Phase 0 — Scaffold

> **Goal:** a window that clears the screen on desktop *and* web.
> **Proves:** the toolchain and cross-compilation work end to end.

- [x] Cargo workspace with `pf_core`, `pf_net`, `pf_render`, `pf_app`.
- [x] Desktop window via `macroquad` (`wgpu` + `winit` remain the eventual
      target; see [Building everywhere](guide/builds.md)).
- [x] WASM build compiling for `wasm32-unknown-unknown`.
- [x] That WASM build confirmed running in a browser, through a manual copy
      and a static server (see [Building everywhere](guide/builds.md)); it
      joins and moves on keyboard input.

## Phase 1 — Deterministic core skeleton

> **Goal:** one controllable capsule with gravity and flat-ground collision,
> rendered.
> **Proves:** the split between the deterministic core and rendering.

- [x] Fixed-point `V2`, deterministic `Rng`.
- [ ] LUT trig: deferred, since nothing needs angles until knockback in
      Phase 5.
- [x] Fixed-timestep loop; flat `World` state.
- [x] One capsule: gravity, ground collision, movement.
- [x] Render with interpolation (macroquad to start).
- [x] Any number of fighters (`Vec<Fighter>`). Netplay caps machines, not
      fighters: `MAX_NETPLAY_MACHINES = 4` in `pf_net`.

## Phase 2 — Rollback integration

> **Goal:** `SyncTestSession` green in CI, then local 2-player.
> **Proves:** determinism is real and guarded.

- [x] Wrap `World` behind a GGRS `Config`.
- [x] `cargo test` runs SyncTest and stays green. :material-shield-check:
- [x] Local multiplayer through a GGRS session, built around a set of
      *local handles*. Local play is the case where every handle is local,
      and the same loop later carries couch + online.
- [ ] Replay recording: initial seed, config, and the per-frame input
      stream, with periodic checksums. (The foundation for the Phase 6
      viewer.)

## Phase 3 — Real netplay

> **Goal:** two instances playing across a network.
> **Proves:** rollback works online.

- [ ] matchbox WebRTC transport + signaling (≤ 4 machines). On web this
      means the wasm-bindgen pipeline (ggrs's `wasm-bindgen` feature, Trunk)
      and dropping the loader stubs in `index.html` and the `getrandom`
      byte source.
- [ ] Couch + online: several local players per machine in one session. The
      cap counts machines, not fighters; the slot binder already claims only
      local handles.
- [ ] Tunable input delay and prediction window.
- [ ] Desync detection via checksums in the wild.

## Phase 4 — Controllers & input

> **Goal:** keyboard, standard gamepads, and a native GameCube adapter all
> map to the same `Input`.
> **Proves:** the input-source abstraction holds and analog fidelity
> survives quantization, without ever touching determinism.

- [x] Input-source abstraction in `pf_app` (platform layer only; four
      keyboard layouts wired).
- [ ] Standard gamepads: `gilrs` on native, the Gamepad API on web.
- [ ] Native GameCube adapter (WUP-028): USB-HID via `hidapi`/`rusb` on
      native, WebHID in the browser.
- [ ] Analog calibration: deadzones, notch/edge clamping, and deterministic
      quantization to the `i8` stick fields.
- [x] Per-player binding: any source claims any free slot by pressing jump.
- [ ] Hotplug.

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
- [ ] Replay viewer: load an input-stream replay, scrub and frame-step, and
      validate playback against the recorded checksums.

## Phase 7 — Ship everywhere

> **Goal:** all platforms + matchmaking.

- [ ] Android / iOS polish.
- [ ] Web netplay hardening.
- [ ] Matchmaking / lobby service.
