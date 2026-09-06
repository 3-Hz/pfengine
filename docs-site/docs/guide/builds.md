---
icon: lucide/package
---

# Building everywhere

One Rust workspace targets every platform. The discipline that keeps this
painless: all OS-specific code lives in `pf_app` behind `#[cfg(...)]`, so the
lower crates stay portable. Today that is one line and the file it names:

```rust title="crates/pf_app/src/main.rs"
#[cfg(target_arch = "wasm32")]
mod wasm_entropy;
```

## Per-platform builds

=== "Desktop (Mac / Linux / Win)"

    ```bash
    cargo run -p pf_app -- --players 4
    ```

    The default target. macroquad opens the window and reads the keyboard;
    `--players` sets the fighter count (default 2).

=== "Web (WASM)"

    ```bash
    cargo build -p pf_app --target wasm32-unknown-unknown
    cp target/wasm32-unknown-unknown/debug/pf_app.wasm crates/pf_app/web/
    # then serve crates/pf_app/web/ over http and open index.html
    ```

    Runs in the browser; joins P1 on Space and moves on the arrows. Delete
    the copied `.wasm` when you are done; it is not gitignored. What that
    page needs in order to load is [below](#what-the-web-build-carries).

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

Four things make `crates/pf_app/web/index.html` load, each answering a problem
the build hit.

- **`mq_js_bundle.js` is vendored.** It is macroquad's JS glue, the `sapp_*`,
  `gl*`, and `fs_*` host functions, copied from the pinned crate (macroquad
  0.4.15, miniquad 0.4.10). The upstream copy on
  `not-fl3.github.io/miniquad-samples` lags the pinned versions and lacks six
  GL entry points, so the page fetches no third-party script. The file carries
  one patch: a `var register_plugin` declaration the upstream `quad_net` chunk
  still lacks.
- **The linker allows undefined symbols.** Those host functions arrive from JS
  at runtime, and recent `rust-lld` rejects them at link time instead of
  emitting imports. `.cargo/config.toml` passes `--allow-undefined` for the
  wasm target.
- **`getrandom` has a byte source.** `ggrs` pulls in `rand`, whose `getrandom`
  refuses `wasm32-unknown-unknown` unless told where bytes come from. Its `js`
  feature would drag wasm-bindgen into the import table, which macroquad's
  loader cannot satisfy, so `pf_app` enables the `custom` feature and registers
  `wasm_entropy.rs`: a xorshift64\* seeded from `miniquad::date::now()`. Not
  cryptographic; ggrs uses it for handshake nonces only, and never while the
  session has no peers.
- **`index.html` stubs the imports the glue lacks.** ggrs's `js-sys`
  dependency still leaves seven wasm-bindgen imports in the binary, reached
  only through the per-peer protocol, and the bundle stubs missing imports
  only under `env`. Before `load()`, the page replaces that pass with one that
  stubs missing function imports in any module. The stubs throw where the
  bundle's own only warn: a stubbed import that is reached is a bug to fail
  on, not a warning to scroll past.

One more waits in ggrs's per-peer code: `instant::Instant::now()` panics on
wasm without the `instant/wasm-bindgen` feature, and cannot run while there
are no peers. All of it goes when Phase 3 moves the web build to the
wasm-bindgen pipeline (ggrs's `wasm-bindgen` feature and
[Trunk](https://trunkrs.dev)). Web netplay itself uses
[matchbox](../architecture/rollback.md#the-web-netplay-trap-and-the-fix),
because browsers cannot open raw UDP sockets.

## Continuous integration

`.github/workflows/rust.yml` runs five steps on every push to `main` and every
pull request:

1. `cargo fmt --all --check`.
2. `cargo clippy --workspace --all-targets -- -D warnings`.
3. `cargo test --workspace`: the determinism gate. `pf_net`'s SyncTest fails
   on any checksum mismatch.
4. `cargo build -p pf_app --target wasm32-unknown-unknown`, so the web build
   cannot break silently.
5. `cargo clippy -p pf_app --target wasm32-unknown-unknown -- -D warnings`:
   the only lint pass that sees `wasm_entropy.rs`, which does not compile on
   the host.

macroquad needs no apt packages on the Linux runner. miniquad opens X11, GL,
and ALSA with `dlopen` at runtime, and its build script links a system library
only on Darwin and iOS, so a headless build links nothing.
`rust-toolchain.toml` pins stable and lists the wasm target; the workflow
repeats both so the action installs them up front instead of leaving rustup to
fetch them mid-build.

## The intended stack

| Concern | Crate | Covers | State |
| --- | --- | --- | --- |
| Fixed point | [`fixed`](https://docs.rs/fixed) | deterministic math | built |
| Rollback | [`ggrs`](https://github.com/gschup/ggrs) | the netcode engine | built |
| Window + rendering | [`macroquad`](https://macroquad.rs) | desktop · web | built, for now |
| GPU | [`wgpu`](https://wgpu.rs) | Vulkan · Metal · DX12 · WebGPU · WebGL2 | decided |
| Window + input | [`winit`](https://docs.rs/winit) | desktop · web · Android · iOS | decided |
| Audio | [`kira`](https://docs.rs/kira) | desktop · web (incl. wasm) | decided |
| Transport | [`matchbox`](https://github.com/johanhelsing/matchbox) | WebRTC, native + browser | Phase 3 |

!!! note "Decided, not built"

    macroquad is the prototyping renderer because it is dead simple and
    cross-platform including web. The end state swaps it for `wgpu` + `winit`
    for real control, and adds `kira` for audio. The swap never touches the
    simulation, which is engine-independent by construction: `pf_render` reads
    `World` and nothing in `pf_core` knows a renderer exists.

## Documentation site (this site)

This site is built with [Zensical](https://zensical.org). From `docs-site/`:

```bash
# one-time: create and activate a virtualenv, then:
pip install zensical

zensical serve        # live preview at http://localhost:8000
zensical build        # static output to docs-site/site/
```

`.github/workflows/docs.yml` deploys it to <https://3-hz.github.io/pfengine/>
on every push to `main` that touches `docs-site/`.
