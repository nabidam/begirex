# Contributing to BegireX

Thanks for helping improve BegireX.

## Before opening an issue

Search existing issues first. Include the operating system, BegireX version,
yt-dlp and FFmpeg versions, steps to reproduce, expected behavior, and actual
behavior. Do not include private URLs, cookies, credentials, or download logs
that contain personal information.

## Development setup

Install a current Node.js LTS release, Rust stable, and the system prerequisites
for [Tauri v2](https://v2.tauri.app/start/prerequisites/). Then run:

```bash
npm ci
npx tauri dev
```

Before submitting a pull request, run:

```bash
npm run build
cd src-tauri
cargo fmt --check
cargo test
```

Keep pull requests focused, explain the user-facing change, and update relevant
documentation. Follow the repository conventions in `CONVENTIONS.md`.

## Conduct

By participating, you agree to follow the [Code of Conduct](CODE_OF_CONDUCT.md).
