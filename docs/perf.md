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

## Phase 1 / S1 — UI renderer bake-off (2026-10-15, Windows dev machine, release builds)
Setup: throwaway app `spike/s1-renderers` (branch `spike/phase1`), showing a generated **24 MP** image fitted to the window. Renderer chosen with `SLINT_BACKEND=winit-<name>`. Runner: `spike/s1-run.ps1`.
- **Auto-resize test:** a timer resizes the window 60×/s (800×500 ↔ 1600×1000). "Slint fps" is Slint's own counter (`SLINT_DEBUG_PERFORMANCE=refresh_lazy,console`); p50/p95 come from our frame-time notifier (only some renderers support it). Peak memory = peak working set.
- **Drag test:** the owner dragged the window edges by hand. That path goes through Windows' modal resize loop, which the timer test does not exercise.

| Renderer | Slint fps (all cores / 2 cores) | p95 frame ms | Peak RSS | Owner drag test | Binary size* |
|---|---|---|---|---|---|
| `femtovg` (OpenGL, Phase 0 default) | 74 / 86 | 18.9 | **664 MB** | ❌ **stutters** | 11.8 MB |
| `femtovg-wgpu` (ran on Intel Vulkan) | 36 / 34 | 39.3 | 494 MB | not tested (too slow) | — |
| `skia` (GPU, wgpu/D3D12 path) | 85 / 85 | n/a | 297 MB | ✅ smooth | 21.3 MB |
| `skia-opengl` | 89 / 84 | 18.2 | 763 MB | ⚠️ slight lag | 21.3 MB |
| `skia-software` (CPU) | 84 / 84 | n/a | 221 MB | ✅ smooth | 21.3 MB |
| `software` (Slint CPU renderer) | **90 / 91** | n/a | **121 MB** | ✅ smooth, same image quality | **11.1 MB** |

\* Binary size of the spike built with only that renderer family plus the software renderer (`software` = software alone).
"2 cores" = CPU affinity limited to cores 0–1 (Lite CPU simulation; the GPU is still available).

**Findings**
1. The Phase 0 stutter is **FemtoVG + Intel's Windows OpenGL driver during interactive resize**. Raw throughput is fine (75–90 fps in the timer test), so the problem is frame pacing inside Windows' modal resize loop. OpenGL through Skia is better but still lags slightly, so OpenGL on Intel/Windows is the weak spot.
2. The GPU renderers' high memory use is mostly the 24 MP image held as a GPU texture (Intel iGPU memory is counted as system RAM), plus every GPU driver that wgpu loads. Phase 2 will upload screen-sized images, not full-resolution ones, which should cut this a lot. That still needs measuring.
3. On this CPU, Slint's **software renderer is smooth, the lightest (121 MB) and the smallest**, even when limited to 2 cores. It still needs measuring on a real low-end CPU (Mint PC / VM).
4. Skia costs **+10 MB** of binary size. 21.3 MB leaves about 4 MB of the 25 MB budget for decoders.

**Still pending:** NVIDIA-forced OpenGL run (optional, since the problem reproduces on Intel, which is the common case); Mesa llvmpipe "no GPU" run; all Linux runs (Mint PC: X11, Wayland, `LIBGL_ALWAYS_SOFTWARE=1`).
