# pfengine

[![Rust](https://github.com/3-Hz/pfengine/actions/workflows/rust.yml/badge.svg)](https://github.com/3-Hz/pfengine/actions/workflows/rust.yml)

A platform-agnostic [platform fighter](docs-site/docs/index.md) engine
(Melee-like) with **deterministic fixed-point simulation** and **rollback
netcode**, built in Rust. Rollback needs every machine to compute the same
bits from the same inputs; that one requirement shapes every crate below.

> Full design docs: <https://3-hz.github.io/pfengine/>. Source in `docs-site/`
> (a [Zensical](https://zensical.org) site); `zensical serve` there previews
> it locally.

## Workspace

| Crate | Role |
| --- | --- |
| `pf_core` | The simulation: fixed-point math and a `Clone`-able `World`, so every machine computes the same bits and a rollback snapshot is one memcpy. Depends on `fixed` and `serde` only. |
| `pf_net` | The GGRS session every tick runs through, so netplay later adds only a transport, plus the SyncTest gate CI runs. |
| `pf_render` | Presentation (macroquad). Reads two `World`s and interpolates; never writes back. |
| `pf_app` | The 60 Hz loop, input sources, slot binding, and the only platform `cfg`s (desktop + web). |

## Develop

```bash
# Determinism gate (must stay green):
cargo test -p pf_net

# Run the demo on desktop (any player count; press jump on a layout to join):
cargo run -p pf_app -- --players 4
#   Arrows + Space    A D + W    J L + I    Numpad 4 6 + 8

# Build for the web:
cargo build -p pf_app --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/debug/pf_app.wasm crates/pf_app/web/
#   then serve crates/pf_app/web/ over http (e.g. `python3 -m http.server`)
#   and open index.html
```

## Status

**Phases 0–1 complete** (LUT trig deferred until knockback needs angles):
deterministic fixed-point core, SyncTest green in CI, and a local N-player demo
that runs on desktop and in the browser.

**Phase 2 in progress:** local play runs through a GGRS session built around
local handles, so netplay later adds only a transport. Netplay will cap at 4
machines; fighters are uncapped. **Next:** replay recording. See the
[roadmap](docs-site/docs/roadmap.md).
