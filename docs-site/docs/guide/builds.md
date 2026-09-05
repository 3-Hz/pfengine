---
icon: lucide/package
---

# Building everywhere

One Rust codebase targets every platform. The discipline that makes this
painless: keep all OS-specific code in `pf_app` behind `#[cfg(...)]`, so the
lower crates stay fully portable.

## Rendering & platform layer

| Concern | Crate | Covers |
| --- | --- | --- |
| GPU | [`wgpu`](https://wgpu.rs) | Vulkan · Metal · DX12 · WebGPU · WebGL2 |
| Window + input | [`winit`](https://docs.rs/winit) | desktop · web · Android · iOS |
| Audio | [`kira`](https://docs.rs/kira) | desktop · web (incl. wasm) |
| Rollback | [`ggrs`](https://github.com/gschup/ggrs) | the netcode engine |
| Transport | [`matchbox`](https://github.com/johanhelsing/matchbox) | WebRTC, native + browser |
| Fixed point | [`fixed`](https://docs.rs/fixed) | deterministic math |

!!! tip "Prototype faster with macroquad"

    Because `pf_core` is engine-independent, you can put early visuals on screen
    with [`macroquad`](https://macroquad.rs) (dead simple, cross-platform incl.
    web) and swap to `wgpu` + `winit` for real control later — without touching
    the simulation.

## Per-platform builds

=== "Desktop (Mac / Linux / Win)"

    ```bash
    cargo run -p pf_app
    ```

    Just works — the default target.

=== "Web (WASM)"

    Compile to `wasm32-unknown-unknown` and bundle with
    [Trunk](https://trunkrs.dev). `wgpu` serves WebGPU with a WebGL2 fallback.

    ```bash
    rustup target add wasm32-unknown-unknown
    trunk serve            # local preview
    trunk build --release  # static site output
    ```

    !!! note "What's actually wired today"

        The Trunk setup above is the intended end state. While the renderer is
        still macroquad, the web build is a manual copy — no Trunk in the repo
        yet:

        ```bash
        cargo build -p pf_app --target wasm32-unknown-unknown
        cp target/wasm32-unknown-unknown/debug/pf_app.wasm crates/pf_app/web/
        # then serve crates/pf_app/web/ over http and open index.html
        ```

        `crates/pf_app/web/` also holds `mq_js_bundle.js` — macroquad's JS glue,
        vendored from the pinned crate, so the page fetches no third-party
        script at runtime. Delete the copied `.wasm` when you are done; it is
        not gitignored.

    Web netplay uses [matchbox](../architecture/rollback.md#the-web-netplay-trap-and-the-fix)
    (WebRTC), since browsers can't open raw UDP sockets.

=== "Android"

    ```bash
    cargo install cargo-ndk
    rustup target add aarch64-linux-android
    cargo ndk -t arm64-v8a build --release
    ```

    Packaged with `cargo-apk` / `xbuild`.

=== "iOS"

    ```bash
    cargo install cargo-mobile2
    cargo mobile init      # generates the Xcode project
    cargo apple open       # build & run via Xcode
    ```

## Keeping platform code contained

```rust title="pf_app — platform seams behind cfg"
#[cfg(target_arch = "wasm32")]
fn entry() { /* trunk / wasm-bindgen startup */ }

#[cfg(not(target_arch = "wasm32"))]
fn entry() { /* native winit event loop */ }
```

Only `pf_app` is allowed to contain `#[cfg(target_os = ...)]` /
`#[cfg(target_arch = ...)]`. `pf_core`, `pf_net`, and `pf_render` stay
platform-neutral — which is also what keeps the
[determinism wall](../architecture/overview.md#why-the-boundary-is-enforced-by-the-compiler)
intact.

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
