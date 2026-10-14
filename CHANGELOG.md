# Changelog

All notable changes to this project are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Phase 0 — environment, repository, skeleton
- Cargo workspace with seven crates (`kinora_app`, `kinora_core`, `kinora_decode`, `kinora_cache`, `kinora_video`, `kinora_edit`, `kinora_platform`) and `xtask`.
- Strict workspace lints, release/dev profiles, pinned toolchain (Rust 1.99.0, edition 2024).
- Empty Slint window titled "Kinora", with both the FemtoVG (OpenGL) and software (CPU) renderers compiled in.
- `cargo xtask check-all` (fmt, clippy, tests, cargo-deny) and `cargo xtask licenses` (cargo-about).
- GitHub Actions CI on Ubuntu 24.04, Windows and macOS.
- ADRs 0001–0003, architecture overview, setup guides, and learning notes for C# developers.

#### Known issues
- On Windows with Intel graphics, live window resize stutters with the FemtoVG/OpenGL renderer, and the empty window uses ~345 MB RSS. To be investigated in Phase 1 (see `docs/perf.md`).
