---
icon: lucide/package
---

# Building everywhere

One Rust codebase targets every platform. One discipline keeps that
painless: all OS-specific code lives in `pf_app`, behind `#[cfg(...)]`, so
the lower crates stay fully portable.

## Rendering & platform layer

| Concern | Crate | Covers | Today |
| --- | --- | --- | --- |
| Window, input, drawing | [`macroquad`](https://macroquad.rs) | desktop · web | in use, for now |
| GPU | [`wgpu`](https://wgpu.rs) | Vulkan · Metal · DX12 · WebGPU · WebGL2 | decided, not built |
| Window + input | [`winit`](https://docs.rs/winit) | desktop · web · Android · iOS | decided, not built |
| Audio | [`kira`](https://docs.rs/kira) | desktop · web (incl. wasm) | decided, not built |
| Rollback | [`ggrs`](https://github.com/gschup/ggrs) | the netcode engine | in use |
| Transport | [`matchbox`](https://github.com/johanhelsing/matchbox) | WebRTC, native + browser | decided, Phase 3 |
| Fixed point | [`fixed`](https://docs.rs/fixed) | deterministic math | in use |

!!! tip "Prototype faster with macroquad"

    `pf_render` draws with [`macroquad`](https://macroquad.rs) today. It is
    dead simple and cross-platform, the web included, which made it the
    fastest way to put early visuals on screen. That was possible because
    `pf_core` is engine-independent: `pf_render` reads `World`, and nothing
    in `pf_core` knows a renderer exists.

    **Decided, not built:** swap macroquad for `wgpu` + `winit` later, for
    real control over rendering. The swap will not touch the simulation.

## Per-platform builds

=== "Desktop (Mac / Linux / Win)"

    ```bash
    cargo run -p pf_app                   # two players
    cargo run -p pf_app -- --players 4    # any player count
    ```

    Just works: desktop is the default target. Each player presses jump on
    a keyboard layout to join.

=== "Web (WASM)"

    While the renderer is still macroquad, the web build is a manual copy.
    There is no Trunk in the repo yet.

    ```bash
    cargo build -p pf_app --target wasm32-unknown-unknown
    cp target/wasm32-unknown-unknown/debug/pf_app.wasm crates/pf_app/web/
    # then serve crates/pf_app/web/ over http and open index.html
    ```

    The page always runs two players, because wasm has no command line to pass
    `--players` on. All four keyboard layouts are live, and the first one to
    press jump takes P1.

    `crates/pf_app/web/` also holds `mq_js_bundle.js`, macroquad's JS glue,
    vendored from the pinned crate so the page fetches no third-party script
    at runtime. Delete the copied `.wasm` when you are done; it is not
    gitignored.

    `index.html` also stubs, before calling `load()`, any function import
    that the glue does not provide, in any module. ggrs's wasm build carries
    a few wasm-bindgen imports that are reached only with remote peers, and
    the stubs throw if called. The stubs and the manual copy both go when
    the web build moves to Trunk. [What the web build
    carries](#what-the-web-build-carries) has the details.

    !!! note "Decided, not built"

        The intended end state compiles to `wasm32-unknown-unknown` and
        bundles with [Trunk](https://trunkrs.dev). `wgpu` serves WebGPU with
        a WebGL2 fallback.

        ```bash
        rustup target add wasm32-unknown-unknown
        trunk serve            # local preview
        trunk build --release  # static site output
        ```

    Web netplay will use
    [matchbox](../architecture/rollback.md#the-web-netplay-trap-and-the-fix)
    (WebRTC), since browsers can't open raw UDP sockets.

=== "Android"

    **Decided, not built.**

    ```bash
    cargo install cargo-ndk
    rustup target add aarch64-linux-android
    cargo ndk -t arm64-v8a build --release
    ```

    Packaged with `cargo-apk` / `xbuild`.

=== "iOS"

    **Decided, not built.**

    ```bash
    cargo install cargo-mobile2
    cargo mobile init      # generates the Xcode project
    cargo apple open       # build & run via Xcode
    ```

## What the web build carries

`crates/pf_app/web/index.html` loads because of four things. Each one
answers a problem the web build hit.

- **The JS glue is vendored.** `mq_js_bundle.js` is macroquad's JS glue,
  which supplies the `sapp_*`, `gl*`, and `fs_*` host functions. It is
  copied from the pinned crates (macroquad 0.4.15, miniquad 0.4.10). It
  replaced the copy on `not-fl3.github.io/miniquad-samples`, which lags the
  pinned versions and lacks six GL renderbuffer and blit entry points. The
  file carries one patch: a `var register_plugin` declaration that the
  upstream `quad_net` chunk still lacks.
- **The linker allows undefined symbols.** The host functions arrive from JS
  at runtime, but recent `rust-lld` rejects them at link time instead of
  emitting them as wasm imports. `.cargo/config.toml` passes
  `--allow-undefined` for the wasm target.
- **`getrandom` has a byte source.** `ggrs` pulls in `rand`, whose
  `getrandom` refuses to build for `wasm32-unknown-unknown` unless told
  where its bytes come from. Its `js` feature would drag wasm-bindgen into
  the import table, which macroquad's loader cannot satisfy. So `pf_app`
  enables the `custom` feature instead and registers `wasm_entropy.rs`: a
  xorshift64\* generator seeded on first use from `miniquad::date::now()`.
  It is not cryptographic. ggrs uses it for handshake nonces only, and never
  while the session has no peers.
- **`index.html` stubs the imports the glue lacks.** ggrs's `js-sys`
  dependency still leaves seven wasm-bindgen imports in the binary, in the
  `__wbindgen_placeholder__` and `__wbindgen_externref_xform__` modules.
  Only the per-peer protocol reaches them. The bundle stubs missing imports
  only under `env`, so without help the browser would reject the module.
  Before `load()`, the page replaces the bundle's stub pass with one that
  stubs every missing function import in every module. The bundle's own
  stubs only warn; these throw, because a stubbed import that gets called is
  a bug to fail on, not a warning to scroll past.

One more problem waits in ggrs's per-peer code: `instant::Instant::now()`
panics on wasm without the `instant/wasm-bindgen` feature. It cannot run
while there are no peers. The byte source, the stubs, and this trap all go
when Phase 3 moves the web build to the wasm-bindgen pipeline (ggrs's
`wasm-bindgen` feature, and Trunk).

## Keeping platform code contained

Only `pf_app` may contain `#[cfg(target_os = ...)]` or
`#[cfg(target_arch = ...)]`. `pf_core`, `pf_net`, and `pf_render` stay
platform-neutral, which is also what keeps the
[determinism wall](../architecture/overview.md#where-the-boundary-is-enforced)
intact.

Today the rule covers one module declaration in `main.rs` and one wasm-only
dependency in `pf_app`'s manifest:

```rust title="crates/pf_app/src/main.rs"
#[cfg(target_arch = "wasm32")]
mod wasm_entropy;
```

```toml title="crates/pf_app/Cargo.toml"
[target.'cfg(target_arch = "wasm32")'.dependencies]
getrandom = { version = "0.2", features = ["custom"] }
```

`wasm_entropy.rs` lives in `pf_app` for exactly this reason: only `pf_app`
may carry platform `cfg`s.

**Decided, not built:** once the renderer moves to `winit` and the web build
to Trunk, the platform seams become separate entry points, still in `pf_app`
and still behind `cfg`:

```rust title="pf_app entry points (design sketch)"
#[cfg(target_arch = "wasm32")]
fn entry() { /* trunk / wasm-bindgen startup */ }

#[cfg(not(target_arch = "wasm32"))]
fn entry() { /* native winit event loop */ }
```

## Continuous integration

`.github/workflows/rust.yml` runs on every push to `main`, on every pull
request, and on manual dispatch. It runs five steps:

1. `cargo fmt --all --check`.
2. `cargo clippy --workspace --all-targets -- -D warnings`.
3. `cargo test --workspace`. This is the determinism gate: `pf_net`'s
   SyncTest fails on any checksum mismatch.
4. `cargo build -p pf_app --target wasm32-unknown-unknown`, so the web build
   cannot break unnoticed.
5. `cargo clippy -p pf_app --target wasm32-unknown-unknown -- -D warnings`.
   This is the only lint pass that sees `wasm_entropy.rs`, which does not
   compile on the host.

macroquad needs no apt packages on the Linux runner. miniquad opens X11, GL,
and ALSA with `dlopen` at runtime, and its build script links a system
library only on Darwin and iOS, so a headless build links nothing.

`rust-toolchain.toml` pins the stable channel and lists the wasm target. The
workflow names the target again, along with the `rustfmt` and `clippy`
components, so the toolchain action installs them up front instead of
leaving rustup to fetch them mid-build.

## Documentation site (this site)

This site is built with [Zensical](https://zensical.org). From `docs-site/`:

```bash
# one-time: create and activate a virtualenv, then:
pip install zensical

zensical serve        # live preview at http://localhost:8000
zensical build        # static output to docs-site/site/
```

`.github/workflows/docs.yml` runs `zensical build --clean` and deploys the
result to <https://3-hz.github.io/pfengine/>. It runs on every push to `main`
(or `master`) that touches `docs-site/` or the workflow file itself, and on
manual dispatch.
