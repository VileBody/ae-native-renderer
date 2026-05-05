# Local Rust Toolchain

Status date: 2026-05-05

This repo now builds on the local macOS host without Docker.

Installed local toolchain:

```text
rustup stable / 1.95.0
cargo 1.95.0
rustfmt
clippy
Homebrew pkgconf
Homebrew gstreamer 1.28.0
```

The repo pins the expected Rust version in `rust-toolchain.toml`, so agents can
run `cargo` directly from the project root.

## First-Time Setup

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
. "$HOME/.cargo/env"
brew install pkgconf gstreamer
```

Verify:

```bash
cargo --version
rustc --version
rustfmt --version
pkg-config --modversion gstreamer-1.0 gstreamer-app-1.0 gstreamer-video-1.0 gstreamer-base-1.0
```

Expected result on this machine:

```text
cargo 1.95.0
rustc 1.95.0
gstreamer 1.28.0
```

## Common Commands

```bash
cargo fmt --all
cargo test -p testkit
cargo test -p render-cli
cargo run -p render-cli -- doctor
```

Hypothesis experiment wrapper:

```bash
cargo run -p render-cli -- hypothesis-pack \
  --module M10 \
  --candidate shadow_blur_floor \
  --status instrumented \
  --gate isolated \
  --case EFF_010 \
  --out target/ae_agents/m10_shadow_blur_floor
```

Docker is still valid for hermetic CI-like checks, but local development should
prefer the native toolchain unless a task explicitly needs the Docker image.
