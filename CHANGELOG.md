# Changelog

All notable changes to this project are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Phase 1 — technical spike (Windows; Linux pending)
- Throwaway spike workspace on branch `spike/phase1` (S1 renderers, S2–S4 video backends A/B/C, S5–S7 images/100%/CPU viewport) with runner scripts.
- Measurements in `docs/perf.md`. ADR 0002 (video engine) revised, plus new ADR 0004 (renderer fallback & Lite mode) and ADR 0005 (image decoders), all **proposed, awaiting approval**.
- Findings: FemtoVG/OpenGL resize stutter on Intel/Windows; Skia-GPU and the software renderer are smooth; mpv in a child window (B) is the smoothest Windows video path; `vo=gpu` + `profile=fast` for no-GPU PCs; resize debounce for GL video; AVIF via rav1d (NASM needed); scaled JPEG via jpeg-decoder; `image` animation iterators never end on truncated files.
- Setup docs: libmpv / vcpkg libheif / NASM / FFmpeg on Windows; Mint spike checklist. Learning notes 05–08.
- `.gitignore`: ignore `target/` at any depth.

### Phase 0 — environment, repository, skeleton
- Cargo workspace with seven crates (`kinora_app`, `kinora_core`, `kinora_decode`, `kinora_cache`, `kinora_video`, `kinora_edit`, `kinora_platform`) and `xtask`.
- Strict workspace lints, release/dev profiles, pinned toolchain (Rust 1.99.0, edition 2024).
- Empty Slint window titled "Kinora", with both the FemtoVG (OpenGL) and software (CPU) renderers compiled in.
- `cargo xtask check-all` (fmt, clippy, tests, cargo-deny) and `cargo xtask licenses` (cargo-about).
- GitHub Actions CI on Ubuntu 24.04, Windows and macOS.
- ADRs 0001–0003, architecture overview, setup guides, and learning notes for C# developers.

#### Known issues
- On Windows with Intel graphics, live window resize stutters with the FemtoVG/OpenGL renderer, and the empty window uses ~345 MB RSS. To be investigated in Phase 1 (see `docs/perf.md`).
