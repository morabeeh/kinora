# Performance log

Measurements against the budgets in PROJECT_BRIEF.md §10. Each phase adds a section, and missed budgets are reported, never hidden.

## Reference machines
| Tier | Machine | Status |
|---|---|---|
| Dev (High) | Windows 11 25H2, Core Ultra 9 275HX (24 cores), 128 GB, Intel Graphics + RTX 5090 Laptop (hybrid) | in use |
| Typical | Owner's Mint PC: Intel i5, integrated graphics, Mint 22.x | not measured yet |
| Lite | simulated (Phase 1: `taskset`/affinity, `systemd-run -p MemoryMax=2G`, forced software GL) | not set up yet |

## Phase 0 — empty window (2026-10-14, Windows dev machine)
| Metric | Value | Budget | Notes |
|---|---|---|---|
| Release binary size | **11.8 MB** | ≤ 25 MB | both renderers compiled in |
| RSS, empty window, FemtoVG/OpenGL, release, after resizing | **345 MB** (288 MB private) | ≤ 200 MB *with a 12 MP image* | ⚠️ way over for an empty window |
| RSS, empty window, FemtoVG/OpenGL, debug, at start | 124 MB | — | |
| RSS, empty window, software renderer, debug, at start | 41 MB | — | |
| Live resize, FemtoVG/OpenGL | **stutters** (owner's observation, debug and release) | smooth | ⚠️ |
| Live resize, software renderer | better, "not buttery smooth" | smooth | |

**Finding:** the OpenGL context was created on the **Intel iGPU** (`igxelpgicd64.dll`, Intel's Windows OpenGL driver, which runs on top of D3D12), not the NVIDIA GPU. Intel graphics is what most low-end and typical Windows PCs have, so this matters for the whole user base, not just this laptop.

**Phase 1 follow-up (input for ADR 0004, renderer fallback):**
1. Add frame-time tracing around resize/redraw so we have numbers, not impressions.
2. Repeat on the NVIDIA GPU (Windows Settings → Display → Graphics → per-app "High performance") to separate driver from toolkit.
3. Compare against a Direct3D-based renderer path on Windows (Slint's Skia renderer on D3D, or a wgpu-based option if Slint 1.18 offers a stable one), weighing binary size.
4. Measure the same build on the Mint PC (Intel iGPU, Mesa). Linux uses a different, generally stronger GL stack.
5. Check memory: how much of the 288 MB private memory comes from the GL driver versus Slint/our code.
