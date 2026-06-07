# pfengine

A platform-agnostic [platform fighter](docs-site/docs/index.md) engine
(Melee-like) with **deterministic fixed-point simulation** and **rollback
netcode**, built in Rust.

> Full design docs live in `docs-site/` (a [Zensical](https://zensical.org)
> site). Run `zensical serve` from `docs-site/` to read them locally.

## Workspace

| Crate | Role |
| --- | --- |
| `pf_core` | Deterministic simulation — fixed-point math, the serializable `World`, systems. No rendering/OS deps. |
| `pf_net` | Rollback wiring (GGRS) + the SyncTest determinism gate. |
| `pf_render` | Presentation (macroquad). Reads the sim, interpolates, draws. |
| `pf_app` | Entry point: the 60 Hz fixed-timestep loop (desktop + web). |

## Develop

```bash
# Determinism gate (must stay green):
cargo test -p pf_net

# Run the Phase 0 demo on desktop:
cargo run -p pf_app
#   P1: ← → move, Space jump      P2: A D move, W jump

# Build for the web:
cargo build -p pf_app --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/debug/pf_app.wasm crates/pf_app/web/
#   then serve crates/pf_app/web/ over http (e.g. `python3 -m http.server`)
#   and open index.html
```

## Status

**Phase 0 complete** — workspace scaffolded, deterministic core + SyncTest green,
a runnable window on desktop and a compiling web build. See the
[roadmap](docs-site/docs/roadmap.md) for what's next (rollback netplay → the
fighter mechanics).
