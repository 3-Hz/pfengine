# pfengine

[![Rust](https://github.com/3-Hz/pfengine/actions/workflows/rust.yml/badge.svg)](https://github.com/3-Hz/pfengine/actions/workflows/rust.yml)

A platform-agnostic [platform fighter](docs-site/docs/index.md) engine
(Melee-like) with **deterministic fixed-point simulation** and **rollback
netcode**, built in Rust. Rollback only works if every machine computes the
same bits from the same inputs, and that one requirement shapes every crate
below.

> Full design docs: <https://3-hz.github.io/pfengine/>. Their source is in
> `docs-site/` (a [Zensical](https://zensical.org) site); run `zensical serve`
> there to preview it locally.

## Workspace

| Crate | Role |
| --- | --- |
| `pf_core` | The simulation. Fixed-point math keeps every machine on the same bits, and `World` is `Clone`, so a rollback snapshot is one allocation and one memcpy. Depends on `fixed` and `serde` only. |
| `pf_net` | The GGRS session that every tick runs through, so netplay later adds only a transport. Also the SyncTest gate that CI runs. |
| `pf_render` | Presentation (macroquad). Interpolates between two `World`s and never writes back. |
| `pf_app` | The 60 Hz loop, input sources, and slot binding. The only crate with platform `cfg`s (desktop + web). |

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

**Phases 0–1 complete** (lookup-table trig waits until knockback needs
angles): a deterministic fixed-point core, SyncTest green in CI, and a local
N-player demo that runs on desktop and in the browser.

**Phase 2 in progress.** Local play runs through a GGRS session built around
local handles, so netplay later adds only a transport. Netplay will cap at 4
machines; fighters are uncapped. **Next:** replay recording. See the
[roadmap](docs-site/docs/roadmap.md).
