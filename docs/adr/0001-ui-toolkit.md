# ADR 0001 — UI toolkit: Slint (native), not a webview

- **Status:** Accepted (2026-10-14)
- **Deciders:** project owner, Claude Code

## Context
Kinora must show photos and play video (MKV, HEVC, AV1…) inside one window. It must start fast and stay light on memory, and it must still work on PCs with no usable GPU (PROJECT_BRIEF.md §4.1, §7.6). The owner comes from C#/XAML.

## Options considered
1. **Tauri (webview + Rust backend).** The engineering habits of ArtCraft (our inspiration) are good, but a webview is the wrong core here:
   - Webviews can't reliably play MKV or HEVC: WebKitGTK depends on GStreamer plugins, and WebView2 needs a paid HEVC extension. They can't decode HEIC at all.
   - Embedding mpv relies on transparent windows or child windows positioned over a `<div>`, which is fragile.
   - WebKitGTK has rendering problems on some NVIDIA setups.
   - A webview with no GPU driver is slow and heavy.
2. **egui.** Immediate-mode and easy to embed GL in, but there's no declarative markup, its accessibility is weaker, and it doesn't feel like a native app.
3. **GTK4 (gtk-rs).** Excellent on Linux, but second-class on Windows and macOS, and its threading model and widget tree are less approachable for us.
4. **Slint.** Declarative `.slint` markup with properties, bindings and callbacks (the closest thing to XAML in Rust). It has a winit backend, AccessKit accessibility, built-in `@tr()` translations, and **two renderers in one binary**: FemtoVG (OpenGL) and a CPU-only software renderer. Its rendering notifier lets us draw mpv's OpenGL output into the scene.

## Decision
Use **Slint 1.18.x** under its **GPL-3.0** license option. Configuration: `default-features = false` with `std`, `compat-1-2`, `backend-winit`, `renderer-femtovg`, `renderer-software`, `accessibility`.
- We don't use the Qt backend. Slint would pick it automatically on Linux if `qmake` is installed, and we want the same backend everywhere.
- We avoid the Skia renderer unless measurements justify its binary size.
- Runtime renderer fallback (FemtoVG → software) uses `slint::BackendSelector` and is implemented in Phase 2. Users can also force it with `SLINT_BACKEND=winit-software`.

## Consequences
- The software renderer has limits: no element rotation/scaling, no drop shadows, no rounded corners combined with clipping, and western scripts only. So the image canvas must not rely on element transforms (we scale on the CPU ourselves), and every visual effect needs a software-safe variant.
- Slint is GPL-3.0-only under the option we use. That's compatible with Kinora's GPL-3.0-or-later (see ADR 0003).
