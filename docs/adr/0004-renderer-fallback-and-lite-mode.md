# ADR 0004 — UI renderer choice, fallback chain and Lite mode

- **Status:** **Accepted for Windows** (owner, 2026-10-15): Skia (GPU) → Slint software, FemtoVG dropped on Windows; **release-binary budget raised from 25 MB to 30 MB**. The Linux default renderer is still *proposed* until the Mint PC run.
- **Deciders:** project owner, Claude Code
- **Evidence:** `docs/perf.md` (Phase 0, Phase 1 S1, S5–S7), spike code on branch `spike/phase1`.

## Context
PROJECT_BRIEF.md §7.2 chose FemtoVG (OpenGL) as the default renderer and Slint's software renderer as the fallback. In Phase 0 the owner saw FemtoVG **stutter during window resize** on this laptop's Intel GPU, with high memory use. Phase 1 compared every Slint 1.18 renderer on Windows 11: Intel Graphics + RTX 5090 laptop, 150% scaling, 24 MP test image, with timer-driven resize numbers plus the owner's drag test.

## Measurements (Windows; details in perf.md)
| Renderer | Drag resize (owner) | Timer resize fps | Peak RSS (24 MP texture) | Binary |
|---|---|---|---|---|
| FemtoVG / OpenGL | ❌ stutters | 74–86 | 664 MB | 11.8 MB |
| FemtoVG / wgpu | — (too slow) | 34–36 | 494 MB | — |
| Skia / OpenGL | ⚠️ slight lag | 84–89 | 763 MB | 21.3 MB |
| **Skia (GPU, default surface)** | ✅ smooth | 85 | 297 MB | 21.3 MB |
| Skia / software | ✅ smooth | 84 | 221 MB | 21.3 MB |
| **Slint software** | ✅ smooth, same quality | 90–91 (even on 2 cores) | **121 MB** | **11.1 MB** |

Plus: the software renderer with our CPU viewport (crop + `fast_image_resize` to the exact window size) gave smooth zoom/pan of a 24 MP photo on 2 cores, and pixel-exact 100% at 150% scaling (S6/S7).

## Proposed decision
1. **Renderer fallback chain, chosen at startup with `slint::BackendSelector`:**
   - **Windows:** Skia (GPU) → Slint software.
   - **Linux:** to be decided after the Mint run (candidates: Skia-OpenGL or FemtoVG on Mesa) → Slint software.
   - **macOS:** Skia (Metal) → software (v1.1).
   - In every case, `SLINT_BACKEND` and Settings → Performance can force one.
2. **Drop FemtoVG on Windows.** It's the only renderer that stuttered, and OpenGL via Intel's Windows driver is the weak spot (Skia-OpenGL lagged too).
3. **Lite mode = Slint software renderer + CPU viewport**, exactly as §7.6 describes. It was the smoothest and lightest renderer on this machine.
4. **The image canvas always uses our CPU viewport output**, shown 1:1 (`phx` units, `pixelated`), in *all* renderers. This keeps fit/100% identical across renderers (§5.4) and avoids uploading full-resolution textures (the source of most GPU memory use).

## Consequences
- ⚠️ **Binary size:** Skia adds about 10 MB (21.3 MB with both renderers, against a 25 MB target). That leaves about 4 MB for decoders, HEIF glue, etc. **Owner decision (2026-10-15): the budget is raised to 30 MB** (PROJECT_BRIEF.md §10 row "Release binary").
- Skia's default surface on Windows went through wgpu: Vulkan was rejected and it fell back to another surface. Phase 2 must **log which surface was selected** and test a machine without Vulkan.
- The software renderer's limits (no shadows, no rounded clipping, no element rotation, western text only) stay real for the *chrome*. UI_SPEC.md must design for them (§6).
- Lite also needs: an image **pyramid** (½, ¼ levels) for zoomed-out views of huge images (fit on 24 MP costs ≈ 20 ms here, so ≈ 60 ms on a Celeron), scaled JPEG decoding for first pixel, and the mpv Lite settings from ADR 0002.

## Still pending (not hidden)
- Linux (Mint PC, Intel iGPU, Mesa): X11 + Wayland, FemtoVG vs Skia-OpenGL vs software, and `LIBGL_ALWAYS_SOFTWARE=1`.
- A real low-end CPU (or a 2-core/2 GB VM). This CPU is roughly 3× faster per core than a Lite-tier Celeron.
- NVIDIA-forced OpenGL run (optional), 100%/125% scaling, GPU-renderer 100% check.

## Go / no-go
**GO** for Phase 2 with this chain on Windows. The Linux default renderer gets decided from the Mint PC numbers before Phase 2 builds the fallback code.
