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

- [ ] Cargo workspace with `pf_core`, `pf_net`, `pf_render`, `pf_app`.
- [ ] Desktop window via `winit`.
- [ ] WASM build via Trunk, running in a browser.

## Phase 1 — Deterministic core skeleton

> **Goal:** one controllable capsule with gravity and flat-ground collision,
> rendered.
> **Proves:** the deterministic core ↔ render split.

- [ ] Fixed-point `V2`, LUT trig, deterministic `Rng`.
- [ ] Fixed-timestep loop; flat `World` state.
- [ ] One capsule: gravity, ground collision, movement.
- [ ] Render with interpolation (macroquad to start).

## Phase 2 — Rollback integration

> **Goal:** `SyncTestSession` green in CI, then local 2-player.
> **Proves:** determinism is real and guarded.

- [ ] Wrap `World` behind a GGRS `Config`.
- [ ] `cargo test` runs SyncTest and stays green. :material-shield-check:
- [ ] Local two-player on one machine.

## Phase 3 — Real netplay

> **Goal:** two instances playing across a network.
> **Proves:** rollback works online.

- [ ] matchbox WebRTC transport + signaling.
- [ ] Tunable input delay / prediction window.
- [ ] Desync detection via checksums in the wild.

## Phase 4 — The fighter

> **Goal:** real Melee-style combat.
> **Proves:** the actual game feel. *(The long phase.)*

- [ ] Action-state machine + frame-data tables.
- [ ] Hitbox / hurtbox / ECB collision.
- [ ] Knockback, hitstun, DI, hitlag.
- [ ] First playable character + stage.

## Phase 5 — Content & tooling

> **Goal:** fast iteration for design.

- [ ] Character / stage data formats.
- [ ] Animation pipeline.
- [ ] Debug tools: hitbox viewer, frame-step, input display.

## Phase 6 — Ship everywhere

> **Goal:** all platforms + matchmaking.

- [ ] Android / iOS polish.
- [ ] Web netplay hardening.
- [ ] Matchmaking / lobby service.

---

!!! note "Learning Rust alongside"

    You don't need all of Rust up front. Front-load ownership/borrowing,
    `struct`/`enum` + pattern matching (your state machines *are* enums), traits
    (GGRS uses them), and `Result`/`Option`. Defer async, advanced lifetimes,
    and `unsafe`. Phases 0–1 are the on-ramp; by Phase 4 you'll be fluent.
