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

## Phase 1 / S2–S3 — video backends on Windows (2026-10-15, release builds)
Test files are generated with FFmpeg 9.0 (testsrc2 pattern + 440 Hz tone, kept outside the repo): 1080p30 and 1080p60 H.264; 1080p30 HEVC (8- and 10-bit); 2160p30 HEVC.
libmpv: shinchiro build `20261015` (baseline x86-64, mpv git `492bae0768`); `mpv.lib` generated from the DLL exports with MSVC `lib.exe`.
Runners: `spike/s2-run.ps1`, `spike/s3-run.ps1`. "2 cores" = CPU affinity on cores 0–1. CPU % is relative to the cores the process may use.

### S2 — backend A (mpv render API → OpenGL texture → Slint image, controls drawn on top)
| Case | Renderer | Hwdec used | Video fps | Frame p95 | mpv render call p50/p95 | Drops | Peak RSS | CPU |
|---|---|---|---|---|---|---|---|---|
| 1080p30 H.264 | femtovg | d3d11va-copy | 30.2 | 49.8 ms | 0.37 / 0.44 ms | 0 | 277 MB | <1% |
| 1080p30 H.264 | skia-opengl | d3d11va-copy | 30.2 | 50.4 ms | 0.52 / 0.77 ms | 0 | 359 MB | <1% |
| 1080p60 H.264 | femtovg / skia-opengl | d3d11va-copy | 59.9 / 60.0 | 17.8 ms | 0.37 / 0.47 ms | 0 | 289 / 366 MB | — |
| 2160p30 HEVC | femtovg / skia-opengl | d3d11va-copy | 30.1 / 30.2 | 50 ms | 0.95 / 1.28 ms | ≤1 | 440 / 679 MB | — |
| 2160p30 HEVC, CPU decode | femtovg / skia-opengl | no | 30.1 / 30.2 | 50 ms | 1.1 / 1.2 ms | ≤1 | 725 / 974 MB | — |
| 1080p30 H.264, CPU decode, 2 cores | femtovg / skia-opengl | no | 30.1 / 30.2 | 50 ms | 0.4 / 0.5 ms | ≤1 | 240 / 321 MB | 8.7 / 9.8% |
| 1080p30 HEVC, CPU decode, 2 cores | femtovg / skia-opengl | no | 30.2 / 30.2 | 50 ms | 0.4 / 0.5 ms | 0 | 247 / 328 MB | 9.4 / 9.9% |
| 1080p30 auto-resize, **no debounce** | femtovg / skia-opengl | d3d11va-copy | **21.3 / 23.4** | 225 / 62 ms | **17.4 / 3.9 ms** | **62 / 48** | 722 / 1074 MB | — |
| 1080p30 auto-resize, **150 ms debounce** | femtovg / skia-opengl | d3d11va-copy | **30.2 / 30.2** | 46 / 48 ms | 0.44 / 0.55 ms | **0** | 618 / 754 MB | — |

First frame after `loadfile`: 150–170 ms (warm) and up to ~1 s on the very first run (cold DLL/driver load). Budget: ≤ 300 ms warm, ≤ 700 ms first video.

Owner's eye test: playback **smooth** in both; controls and the translucent overlay bar work; **drag-resize stutters in both** (the same OpenGL-on-Intel/Windows weakness as S1).

Findings:
1. mpv rebuilds its scaler for every new target size (~15 ms per resize step). **Debouncing** (keep rendering at the old size and let Slint scale until the size has been stable for 150 ms) fixes the timer-driven resize test completely. Interactive drag still stutters because of the GL driver (S1).
2. On Windows, backend A can only use hardware decoding in **copy-back** mode (`d3d11va-copy`: GPU → RAM → OpenGL), which costs memory bandwidth.
3. The frame interval p95 of about 50 ms for 30 fps content means some frames are shown for 3 display refreshes instead of 2 (`video-timing-offset=0` + Slint's redraw scheduling). It isn't visible to the owner on test patterns; Phase 3 should use mpv's frame timing (`report_swap` / target time) for even pacing.
4. Memory with GPU renderers is high (277–974 MB), and it grows during resize (texture churn).

### S3 — backend B (mpv's own `vo=gpu-next`/D3D11 in a native child window, controls below)
| Case | UI renderer | Hwdec used | Video fps | VO drops | Peak RSS | CPU | First frame |
|---|---|---|---|---|---|---|---|
| 1080p30 H.264 | skia / software | **d3d11va (zero-copy)** | 30.0 | 0 | 378 / **224 MB** | 0.7 / 0.4% | 430 / 345 ms |
| 1080p30 H.264, auto-resize | skia / software | d3d11va | 30.0 | **0** | 474 / 336 MB | 1.8 / 2.0% | 347 / 367 ms |
| 2160p30 HEVC | skia / software | d3d11va | 30.0 | 0 | 561 / 422 MB | 0.6 / 0.4% | 298 / 324 ms |
| **No-GPU sim** (`d3d11-warp`), CPU decode, 2 cores, 1080p30 H.264, default `gpu-next` | software | no | 30.0 | **200 of 300** | 195 MB | **90%** | 683 ms |
| same, `gpu-next` + `profile=fast` | software | no | 30.0 | 187 | 175 MB | 94% | 193 ms |
| same, **`vo=gpu` + `profile=fast`** | software | no | 30.0 | **0** | 158 MB | **33%** | 169 ms |
| same, `vo=gpu` + `profile=fast`, 1080p30 HEVC | software | no | 30.0 | 0 | 163 MB | 36% | 140 ms |
| same, `vo=gpu` + `profile=fast`, 1080p60 H.264 | software | no | 60.0 | 0 | 160 MB | 65% | 172 ms |
| same, `vo=gpu` + `profile=fast`, 2160p30 HEVC | software | no | 30.0 | 0 | 299 MB | 46% | 195 ms |

(`vo=direct3d` measured 9% CPU, but `d3d11-warp` does not apply to Direct3D 9, so that run still used the real GPU. It is not a valid no-GPU result.)

Owner's eye test: playback **smooth**, **drag-resize smooth** with both UI renderers, controls work, nothing covers the controls.

Findings:
1. Backend B is the best Windows path measured: zero-copy hardware decoding, no drops even while resizing, lowest CPU, and the UI renderer can be the smooth one from S1.
2. Its cost is the **airspace rule**: nothing can be drawn over the video, so controls go below it. Overlay chrome (fullscreen auto-hide bar) would need a separate transparent top-level window over the video. Not tested yet.
3. Lite mode on Windows must switch mpv to **`vo=gpu` + `profile=fast`**. The default `gpu-next` is far too heavy when Direct3D falls back to the CPU.
4. Still pending: a real old CPU (this CPU's cores are roughly 3× faster than a Lite-tier Celeron), and Linux (X11 `wid`; Wayland has none, so backend C).

### S4 — backend C (mpv software render API → CPU buffer ≤ 854×480 → Slint image, on a render thread)
UI renderer: Slint `software`. CPU decoding (`hwdec=no`), `sw-fast=yes`.

| Case | Video fps | mpv SW render p50 / p95 | Peak RSS | CPU | First frame |
|---|---|---|---|---|---|
| 1080p30 H.264, all cores | 30.1 | 0.71 / 0.98 ms | 200 MB | 1.2% | 82 ms |
| 1080p30 H.264, 2 cores | 30.0 | 1.60 / 1.80 ms | **102 MB** | **14%** | 84 ms |
| 1080p30 HEVC, 2 cores | 30.0 | 1.69 / 2.02 ms | 107 MB | 15% | 75 ms |
| 1080p60 H.264, 2 cores | 59.8 | 1.65 / 2.89 ms | 104 MB | 25% | 75 ms |

Owner's eye test (2 cores, maximized window): motion **smooth**, picture softer but **acceptable for a fallback**, resize smooth, controls work (drawn on top of the video).

Findings:
1. Backend C is far cheaper than expected, and cheaper than backend B under WARP (14% vs 33% of 2 cores at 1080p30). Rendering at ≤ 854×480 and letting Slint's software renderer scale up is the cheapest path measured. It works on every platform (including Wayland) and allows overlay controls.
2. Its cost is picture quality (upscaled from 480p) and CPU-side YUV→RGB conversion, which matters more on a real Celeron.
3. Measuring on a real old CPU is still pending.
