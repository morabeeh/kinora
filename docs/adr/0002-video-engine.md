# ADR 0002 — Video engine: embedded libmpv with three output backends

- **Status:** **Accepted for Windows** (owner, 2026-10-15): backend B is the Windows default. Linux/macOS backend order is still *proposed* until the Mint PC measurements exist.
- **Deciders:** project owner, Claude Code
- **Evidence:** `docs/perf.md` (Phase 1 S2–S4), spike code on branch `spike/phase1`.

## Context
Video must play inside our window, fit it like photos, and work on every hardware tier (PROJECT_BRIEF.md §5.5, §7.6). The brief made backend **A** (mpv render API → OpenGL texture inside the Slint scene) the default whenever hardware OpenGL works. §7.3 says: if A can't meet the smoothness bar, evaluate making **B** (`wid` child window) the default, and **ask the owner** before switching.

## What Phase 1 measured (Windows 11, Intel iGPU, libmpv git 20261015)
| | A — GL texture (S2) | B — child window, `vo=gpu-next`/D3D11 (S3) | C — software buffer ≤ 854×480 (S4) |
|---|---|---|---|
| 1080p/4K playback | ✅ 30/60 fps, render call 0.4–1.3 ms | ✅ 30/60 fps, 0 drops | ✅ 30/60 fps |
| Hardware decoding | ⚠️ `d3d11va-copy` only (GPU→RAM→GL) | ✅ `d3d11va` **zero-copy** | CPU decoding by design |
| Resize while playing | ⚠️ needs a 150 ms **debounce** (else 15 ms per frame, 50+ drops); **owner's drag test: stutters** (GL on Intel) | ✅ **smooth** (owner), 0 drops | ✅ smooth (owner) |
| Our controls over the video | ✅ yes (overlay) | ❌ **no** (airspace): controls go below; fullscreen overlay needs an extra transparent window | ✅ yes |
| Memory (1080p) | 277–359 MB | 224 MB (software UI) / 378 MB (Skia UI) | **102 MB** |
| CPU, no GPU (WARP / software), 2 cores, 1080p30 | — | 90% + 200 drops with `gpu-next`; **33%, 0 drops with `vo=gpu` + `profile=fast`** | **14%**, 0 drops (480p quality) |
| Works with the smooth UI renderers (Skia, software) | ❌ needs an OpenGL UI renderer | ✅ any | ✅ any |
| Wayland | ✅ | ❌ (no `wid`) | ✅ |

Owner's eye tests: playback smooth in A, B and C; drag-resize smooth in **B and C**, stuttering in **A**; controls worked everywhere; C's picture "acceptable for a fallback".

## Proposed decision (needs the owner's OK — it changes the brief's default on Windows)
**libmpv: GO.** One lazily created instance, reused with `loadfile`, all FFI inside `kinora_video`, with the hardening options (no ytdl/scripts/config/OSC/default bindings) confirmed working.

Per-platform backend order (automatic downgrade on failure):
- **Windows:** **B** (`vo=gpu-next`, `gpu-api=d3d11`, `hwdec=auto-safe`) → on a no-GPU/Lite PC, **B with `vo=gpu` + `profile=fast`** → **C**.
  - Windowed layout: transport bar **below** the video.
  - Fullscreen auto-hide chrome: prototype a small transparent top-level window over the video in Phase 3. If that fails, the chrome uses a bottom strip that slides in.
- **Linux X11:** A or B, decided on the Mint PC. A gives overlays; B gives zero-copy VA-API and mpv's own pacing.
- **Linux Wayland:** **A** when hardware GL works, else **C**.
- **macOS (v1.1):** A or C (no `wid` on recent macOS without extra work); measure later.

Rules that come from the measurements:
- Backend A must **debounce resizes** (render at the old size, let the UI scale, re-render at the exact size after ~150 ms).
- Backend A should use mpv's frame timing (`report_swap` / target time) for even pacing.
- Backend C renders on its **own thread** and hands finished buffers to the UI. It never blocks the UI thread.
- Lite mode on Windows switches mpv to `vo=gpu` + `profile=fast`; the default `gpu-next` is far too heavy without a GPU.

## Bindings
`libmpv2` 6.0.0 works (verified: GL render API, events, properties). Caveats:
- It's LGPL-2.1-**only**. That's compatible with our GPL-3.0 via LGPL-2.1 §3, but needs a `deny.toml` entry when added in Phase 3.
- Its safe render wrapper only supports OpenGL, and `RenderContext::get_info` contains a `panic!`.
- `RenderContext` borrows `Mpv`, which forces awkward ownership.

**Proposal:** in Phase 3, use only `libmpv2-sys` (raw bindings), or generate our own from mpv's ISC-licensed headers, and write our own small safe wrapper in `kinora_video` that owns both the handle and the render context.

## Windows packaging notes (for Phase 7)
- Use the **baseline** `mpv-dev-x86_64` build. Never use the `x86_64-v3` build, which requires AVX2 and would break the Lite tier.
- `libmpv-2.dll` is about 118 MB (FFmpeg included). The MSVC import library is generated with `lib /def` from the DLL's exports.

## Still pending (not hidden)
- Linux X11/Wayland measurements on the Mint PC; VA-API on Intel.
- A real low-end CPU / 2-core VM. This CPU's cores are roughly 3× faster than a Lite Celeron.
- The fullscreen-overlay prototype for backend B.

## Go / no-go
**GO** with libmpv, **if the owner approves B as the Windows default**. The alternative is to keep A as the brief says and accept the resize stutter on Intel/Windows.
