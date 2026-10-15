# Changelog

All notable changes to this project are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Phase 2 — core image viewer (Windows verified; Linux via CI only)
- **Viewer:** opens a file from the command line, drag-and-drop or Ctrl+O; lists its folder (natural sort like Explorer/Nemo, hidden files skipped) and flicks through it with Left/Right, PageUp/PageDown, Home/End, mouse back/forward, horizontal touchpad swipes and the edge buttons. Wrap-around setting with an "end of folder" hint.
- **Zoom/pan:** fit (no enlarging by default), wheel/`+`/`-` zoom anchored at the cursor with stops at 100% and fit, Ctrl+0 / Ctrl+1, double-click toggles fit ↔ 100%, drag and arrow-key panning, F11 fullscreen. 100% is one image pixel per physical screen pixel.
- **Formats:** JPEG, PNG/APNG, GIF, WebP, HEIC (libheif), AVIF (rav1d), JPEG XL, BMP, ICO, TIFF, TGA, PNM, QOI, DDS, HDR/EXR (tone-mapped), SVG; EXIF orientation, ICC → sRGB, transparency on a checkerboard, animations with pause and frame step.
- **Speed:** background decode pool with priorities and cancellation, direction-aware preloading, byte-budgeted cache, DCT-scaled JPEG decoding, embedded-thumbnail first picture in Lite mode, viewport render thread with a half-size pyramid.
- **Low-end PCs:** hardware probe (cores incl. affinity, RAM, no-GPU detection via DXGI / Mesa hints), Lite/Typical/High budgets, automatic software renderer without a GPU, `--lite`, `--renderer`, `KINORA_SIMULATE=lite`.
- **Robustness:** a damaged file shows an error card, never a crash; size checks before allocating; panics in decoders caught; container completeness check (fixes an `avif-parse` panic on truncated files); `heif.dll` delay-loaded on Windows.
- **Platform:** single instance (named pipe / Unix socket), settings in `settings.toml` (atomic save), daily log files, `--bench-startup`.
- **Tooling:** `cargo xtask run` and `gen-folder`; 205 KB of generated CC0 fixtures; CI installs NASM and libheif on all three OSes.
- Measurements in `docs/perf.md` (Phase 2). Budgets missed with the Skia renderer (start-up ~350 ms, ~300 MB) and binary size (34.1 MB vs 30 MB): see the proposals there.
- Learning notes 09–16.

### Phase 1 — technical spike (Windows; Linux pending)
- Throwaway spike workspace on branch `spike/phase1` (S1 renderers, S2–S4 video backends A/B/C, S5–S7 images/100%/CPU viewport) with runner scripts.
- Measurements in `docs/perf.md`. ADR 0002 (video engine) revised, plus new ADR 0004 (renderer fallback & Lite mode) and ADR 0005 (image decoders), **accepted** for Windows (ADR 0005 fully accepted). Release-binary budget raised from 25 MB to 30 MB.
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
