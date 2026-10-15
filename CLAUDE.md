# CLAUDE.md — standing instructions for Kinora

Full spec: `PROJECT_BRIEF.md` (read it when starting a session). This file is the short version.

## Current phase
**Phase 2 (core image viewer) implemented on Windows (2026-10-15)** on branch `phase2/image-viewer`, awaiting the owner's review, push and merge. Open owner decisions (see `docs/perf.md` Phase 2): Windows default renderer (Skia misses the start-up/memory budgets, software meets them) and the 30 MB binary budget (now 34.1 MB). Linux defaults still pending (`docs/setup-linux.md` §5; Linux renderer = FemtoVG, provisional). Never start the next phase without the owner's explicit "go".
Earlier: Phase 1 spike (branch `spike/phase1`, never merged): ADRs 0002/0004/0005; Windows video = mpv child window (backend B); AVIF = rav1d (needs NASM).

## Owner & decisions (2026-10-14)
- Owner is an experienced C#/.NET dev, new to Rust → explain in C# terms; add a `docs/learning/` note (≤ 1 page, C# comparison, pointer into our code) for every Rust concept introduced.
- Develops on **Windows 11** now; **Linux Mint 22 is the #1 target** (covered by CI `ubuntu-24.04` + `docs/setup-linux.md`). Owner's Mint PC: Intel i5, integrated graphics.
- Name Kinora · GPL-3.0-or-later · macOS deferred to v1.1 · wheel default = zoom · VS Code · Flatpak later · **public** GitHub repo `morabeeh/kinora` (made public 2026-10-15 → free CI minutes) · Lite tier simulated (no old hardware yet).

## Commands
```
cargo xtask run [--release] -- <file> [--lite] [--renderer software] [--new-window] [--bench-startup]
cargo xtask check-all                      # fmt --check + clippy -D warnings + tests + cargo deny
cargo xtask gen-folder <dir> [--count N]   # synthetic mixed-format photos for flick tests
cargo xtask licenses                       # regenerate THIRD_PARTY_NOTICES.md
cargo test -p kinora_core                  # one crate
$env:KINORA_SIMULATE="lite"                # simulate a Lite PC (2 cores, 2 GB, no GPU); add CPU affinity for real 2-core limits
$env:SLINT_BACKEND="winit-software"        # force the CPU renderer (bash: SLINT_BACKEND=winit-software)
```
Windows: xtask adds vcpkg (libheif) and NASM to the environment; plain `cargo` needs `VCPKG_ROOT` + PATH (see `docs/setup-windows.md` §7). Fixtures: `test_data/generator` (outside the workspace).

## Crate map (dependency direction: app → everything; core → nothing internal)
| Crate | Role |
|---|---|
| `kinora_app` | binary: startup, renderer/Lite choice, viewer controller (`src/viewer/`), Slint UI (`ui/*.slint`), `build.rs` (delay-loads heif.dll) |
| `kinora_core` | pure logic: natural sort, folder model, navigation, preload plan, viewport maths, hardware tiers, settings; heavily unit/property-tested |
| `kinora_decode` | sniffing, all v1 image decoders, orientation, ICC, tone mapping, CPU viewport renderer + pyramid |
| `kinora_cache` | byte-budgeted LRU (pinning), priority work pool with cancellation |
| `kinora_video` | **only** place for libmpv FFI; output backends A/B/C (Phase 3) |
| `kinora_edit` | edit pipeline, encoders, atomic metadata-preserving save (Phase 6) |
| `kinora_platform` | OS integration: hardware probe, single instance, paths, atomic write, memory |
| `xtask` | dev tasks (`cargo xtask`) — never shipped |

## Coding rules (PROJECT_BRIEF.md §8)
- `cargo fmt` clean; `clippy --workspace --all-targets -D warnings` clean. Lints live in `[workspace.lints]` — every crate has `[lints] workspace = true`.
- No `unwrap/expect/panic/todo/unimplemented/dbg!/println!/eprintln!`, no slice indexing (`.get()`), no `as` casts (use `try_from`) in non-test code. Audited exceptions: `kinora_core::viewport::to_px`, `kinora_decode::svg::f32_of`.
- `unsafe` only in `kinora_video` / `kinora_platform`, each block with `// SAFETY:`.
- Libraries: `thiserror` typed errors. App binary: `anyhow`. Logging: `tracing` only.
- Never silence a lint / skip a test to get green. Any `#[allow]` needs a comment with the reason, and must be mentioned to the owner.
- Before adding a dependency: verify current version, maintenance and license on crates.io; pin in `[workspace.dependencies]`; report name, version, license, why, one alternative.
- Never `target-cpu=native` / AVX2 assumptions. No tokio/async.
- Conventional Commits, small, each builds and passes tests. Update `CHANGELOG.md` per phase.
- Ask before: installing software, sudo/admin, deleting files, pushing to GitHub.
- Never claim something works unless it was run. Say when something was verified.

## Never do (§3, §9)
No AI features, cloud, accounts, telemetry, ads, network calls, photo-library database, advanced editing. Never copy code/assets from proprietary products. No static linking of LGPL C libraries (libmpv, libheif, libde265). No "Windows/Photos/Microsoft/Apple" in our name, icon or store listings. Deletes go to Trash only; writes are atomic.
