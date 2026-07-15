# Contributing to Stackr

Thanks for your interest! Stackr is a Tauri 2 + React + TypeScript + Rust desktop
app that targets **Windows only**.

## Prerequisites

- Windows 10 / 11
- Node.js 20+
- Rust (stable) — <https://rustup.rs>
- MSVC build tools (the Tauri toolchain); WebView2 is bootstrapped at runtime

## Getting started

```bash
npm ci
npm run tauri dev      # hot-reloading dev build
```

## Before opening a PR

Run exactly what CI runs (see [`.github/workflows/ci.yml`](.github/workflows/ci.yml)):

```bash
npm run build                                              # tsc + vite
cargo test  --manifest-path src-tauri/Cargo.toml --lib     # unit tests
cargo build --manifest-path src-tauri/Cargo.toml --release # release compile
```

Live/network tests are marked `#[ignore]` and skipped in CI — run them explicitly
when your change touches that path.

## Guidelines

- Keep PRs focused: one logical change, with a clear description of the why.
- Match the surrounding code style (TypeScript/React on the frontend, Rust on the
  backend).
- For anything security-related, follow [`SECURITY.md`](SECURITY.md) — do not open a
  public issue.
