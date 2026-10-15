# CLAUDE.md — standing instructions for Kinora

Full spec: `PROJECT_BRIEF.md` (read it when starting a session). This file is the short version.

## Current phase
**Phase 1 (technical spike) done on Windows (2026-10-15)**: ADR 0002/0004/0005 are *proposed*, waiting for the owner's approval; Linux runs pending (`docs/setup-linux.md` §5). Spike code: branch `spike/phase1` (throwaway, never merged). Never start the next phase without the owner's explicit "go".
Key Phase 1 results: Windows UI = Skia GPU → software fallback (FemtoVG stutters on Intel); Windows video = mpv child window (backend B), `vo=gpu`+`profile=fast` without a GPU; AVIF = avif-decode/rav1d (needs NASM); see `docs/perf.md`.

## Owner & decisions (2026-10-14)
- Owner is an experienced C#/.NET dev, new to Rust → explain in C# terms; add a `docs/learning/` note (≤ 1 page, C# comparison, pointer into our code) for every Rust concept introduced.
- Develops on **Windows 11** now; **Linux Mint 22 is the #1 target** (covered by CI `ubuntu-24.04` + `docs/setup-linux.md`). Owner's Mint PC: Intel i5, integrated graphics.
- Name Kinora · GPL-3.0-or-later · macOS deferred to v1.1 · wheel default = zoom · VS Code · Flatpak later · **public** GitHub repo `morabeeh/kinora` (made public 2026-10-15 → free CI minutes) · Lite tier simulated (no old hardware yet).

## Commands
```
cargo run -p kinora_app                    # run (binary name: kinora)
$env:SLINT_BACKEND="winit-software"        # PowerShell: force the CPU renderer (bash: SLINT_BACKEND=winit-software)
cargo xtask check-all                      # fmt --check + clippy -D warnings + tests + cargo deny
cargo xtask licenses                       # regenerate THIRD_PARTY_NOTICES.md
cargo test -p kinora_core                  # one crate
```

## Crate map (dependency direction: app → everything; core → nothing internal)
| Crate | Role |
|---|---|
| `kinora_app` | binary, wiring, Slint UI (`ui/*.slint`), `build.rs` |
| `kinora_core` | pure logic, no UI/IO; heavily unit-tested |
| `kinora_decode` | sniffing, decoding, orientation, colour, downscale |
| `kinora_cache` | byte-budgeted LRU caches, thumbnails |
| `kinora_video` | **only** place for libmpv FFI; output backends A/B/C |
| `kinora_edit` | edit pipeline, encoders, atomic metadata-preserving save |
| `kinora_platform` | OS integration, hardware detection |
| `xtask` | dev tasks (`cargo xtask`) — never shipped |

## Coding rules (PROJECT_BRIEF.md §8)
- `cargo fmt` clean; `clippy --workspace --all-targets -D warnings` clean. Lints live in `[workspace.lints]` — every crate has `[lints] workspace = true`.
- No `unwrap/expect/panic/todo/unimplemented/dbg!/println!/eprintln!`, no slice indexing (`.get()`), no `as` casts (use `try_from`) in non-test code.
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
