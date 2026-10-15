# Kinora

A fast, calm, free and open-source **photo and video viewer** for Linux, Windows and macOS.

- Opens nearly every image and video format in one smooth viewer, and lets you flick through the folder.
- A familiar layout for people coming from the Windows Photos viewer.
- Runs well on old PCs without a usable GPU, thanks to a CPU-rendering fallback and Lite mode.
- **No AI, no cloud, no accounts, no ads, no telemetry, no network access.**

> **Status:** early development (Phase 2 — the image viewer works; video, polish, file actions and editing come next). Nothing to download yet.

## Building from source

- Windows: see [docs/setup-windows.md](docs/setup-windows.md)
- Linux Mint / Ubuntu / Debian: see [docs/setup-linux.md](docs/setup-linux.md)

```bash
cargo xtask run -- path/to/photo.jpg   # Windows: finds vcpkg/NASM for you
cargo run -p kinora_app -- path/to/photo.jpg
cargo xtask check-all
```

## Documentation

- [Architecture](docs/ARCHITECTURE.md) · [Decisions (ADRs)](docs/adr/) · [Learning notes for C# developers](docs/learning/)

## License

Kinora is free software, licensed under the [GNU General Public License v3.0 or later](LICENSE).
Third-party components are listed in `THIRD_PARTY_NOTICES.md` (generated with `cargo xtask licenses`).
