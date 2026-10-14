# Kinora — Master Prompt for Claude Code

> **How to use this file (for you, the human):**
> 1. Install Claude Code on your Linux Mint machine and open a terminal in an empty folder, e.g. `~/dev/kinora`.
> 2. Copy this whole file into that folder as `PROJECT_BRIEF.md`.
> 3. Start Claude Code there and send: **"Read PROJECT_BRIEF.md fully and follow it. Start with Section 0."**
> 4. Claude Code will ask you a few questions, then work phase by phase, stopping for your approval between phases.
>
> Everything below the line is written *to Claude Code*.

---

## 0. Your first actions (do these in order, then stop)

1. Read this entire brief before doing anything else. Do not write code yet.
2. Ask me the **Open Questions** in Section 14 (use multiple-choice questions where possible; each has a default you can propose).
3. Enter plan mode and present a short plan for **Phase 0** only (Section 12). Wait for my "go".
4. After Phase 0 is done, continue phase by phase. **Never start the next phase without my explicit approval.**

---

## 1. Who you are and who I am

- **You:** a senior Rust desktop engineer and patient mentor. You write production-quality, idiomatic, well-tested Rust and you explain your decisions.
- **Me:** an experienced **.NET / C# developer who has never used Rust** and has **not installed Rust yet**. My main OS is **Linux Mint (Cinnamon)**, then **Windows**.
- **Teaching rule:** every time you introduce a Rust concept for the first time (ownership, borrowing, lifetimes, `Result`, traits, `Arc<Mutex<T>>`, `Send`/`Sync`, macros, `unsafe`, FFI, Slint properties/callbacks…), add or update a short note in `docs/learning/` that explains it **with a C# comparison** and points to the exact place in our code where it is used. Keep each note under one page. Mention new notes in your end-of-phase summary.
- When I ask "why", answer in plain language first, then the technical detail.

---

## 2. Product vision

**Name:** **Kinora** (working name — confirm in Open Questions). The Kinora (1890s) was one of the first viewers built for watching pictures *and* motion at home. It fits an app that shows photos and videos in one smooth viewer, and it carries no trademark baggage from Microsoft, Apple or GNOME.

**One-line pitch:** A fast, beautiful, free and open-source photo **and** video viewer for Linux, Windows and macOS. It feels as familiar as the Windows Photos viewer, starts faster, opens nearly every image and video format, and has no AI features, ads, accounts, cloud upsells or telemetry.

**Target user:** a non-technical person who double-clicks a photo or video in their file manager and expects it to just open, look great, and let them flick through the rest of the folder.

**Design principles (in priority order):**
1. **Never lose user data.** Edits never corrupt originals; deletes go to the Trash/Recycle Bin.
2. **Instant feel — on any PC.** First pixel fast, next/previous instant, UI never freezes, **including on old low-end machines without a GPU** (Section 4.1).
3. **Familiar, calm UI.** Windows-Photos-like layout; controls get out of the way.
4. **Opens everything.** If a mainstream format exists, we show it — or explain clearly why we can't.
5. **Small, honest, private.** No network access is needed for any feature. No telemetry. No AI.

---

## 3. Non-goals (do NOT build these)

- No AI features of any kind: no Copilot-style assistant, generative erase, background removal, auto-tagging, face/object recognition, "relight", super-resolution, visual search.
- No cloud: no OneDrive/iCloud/Google integration, no accounts, no sign-in, no sync.
- No telemetry, analytics, crash uploading, ads, in-app purchases, or "pro" upsells.
- No photo-library/indexing database in v1. This is a **viewer**, not an organizer. (A simple folder "gallery grid" is fine later.)
- No advanced editing: no layers, masks, healing, curves editor, RAW development, video editing timeline.
- No network calls at all in v1 (no auto-update pinging). If an update check is added later it must be opt-in.

---

## 4. Platforms and priorities

| Priority | Platform | Notes |
|----------|----------|-------|
| 1 | **Linux Mint 22.x Cinnamon** (Ubuntu 24.04 base), Debian 12/13 | Cinnamon is X11 by default; **Wayland must also work** (GNOME/KDE users). Test NVIDIA + Intel/AMD. |
| 2 | **Windows 10 22H2 and Windows 11** | x86_64 first; ARM64 later. |
| 3 | **macOS 13+** | Apple Silicon + Intel. May slip to v1.1 if Open Questions say so. |

HiDPI / fractional scaling and multi-monitor must work on all three.

### 4.1 Hardware support — low-end devices are a first-class target

Kinora must run **well on old and cheap machines, including ones with no dedicated GPU, a weak or missing GPU driver, a slow CPU, little RAM, and a spinning hard disk.** This is a hard requirement, not a nice-to-have. Many Linux Mint users run it precisely because their PC is old.

| Tier | Example hardware | Must work | Experience target |
|------|------------------|-----------|-------------------|
| **Minimum (Lite)** | 2-core x86-64 CPU without AVX2 (e.g. Intel Celeron/Pentium N-series, Core 2 Duo-era, AMD A4/E-series), **2 GB RAM**, HDD, **no usable GPU acceleration** (Mesa llvmpipe on Linux, "Microsoft Basic Display Adapter"/WARP on Windows, VMs), 1366×768 | Yes — every feature except heavy video (see 7.6) | Photos open and flick smoothly; 1080p H.264 plays; UI is responsive with reduced effects |
| **Typical** | 4-core CPU from ~2015+, integrated Intel/AMD graphics with working drivers, 4–8 GB RAM, SSD | Yes | Full experience, all budgets in Section 10 |
| **High** | Modern CPU, dedicated GPU, high-refresh/4K displays | Yes | 120 Hz-smooth, 4K/HDR video |

Rules that follow from this:
- **Never build with `-C target-cpu=native`** or assume AVX2/AVX-512. Ship a baseline x86-64 binary; use crates that pick SIMD paths at **runtime** (zune-jpeg, fast_image_resize, etc.). ARM64 builds use the baseline ARMv8 target.
- **No feature may require a GPU.** Every GPU-accelerated path has a CPU fallback that is automatically selected (Section 7.6).
- **No feature may require more RAM than the machine has.** Memory budgets scale with installed RAM.
- Assume disks can be slow (HDD, USB stick, network share): never read more of a file than needed, never scan the whole folder's metadata up front.

---

## 5. Feature specification

Mark each feature **[v1]** (must ship in 1.0) or **[v1.x]** (after 1.0). Build v1 first.

### 5.1 Opening files
- [v1] Open via command-line argument (this is how file managers launch us), drag-and-drop onto the window, File → Open dialog (native dialog), and recent files.
- [v1] **Single-instance mode** (default on, setting to turn off): opening another file while Kinora runs reuses the existing window, like Windows Photos.
- [v1] Detect file type by **content sniffing**, not just extension (a `.jpg` that is really a PNG must still open).
- [v1] Clear, friendly error card inside the viewer for unreadable/corrupt/unsupported files (filename, reason in plain words, "Open with…" button). **A bad file must never crash the app.**

### 5.2 Supported formats
- **Images [v1]:** JPEG, PNG + APNG (animated), GIF (animated), WebP (still + animated), **HEIC/HEIF**, **AVIF**, BMP, ICO, TIFF, TGA, PNM, QOI, DDS, HDR/EXR (tone-mapped for display), **SVG** (rendered), **JPEG XL**.
- **Images [v1.x]:** camera RAW (CR2/CR3/NEF/ARW/DNG/RAF/ORF) — show the **embedded JPEG preview** only; no RAW development.
- **Video [v1]:** everything libmpv/FFmpeg plays, including MP4/M4V, **MKV**, WebM, MOV, AVI, WMV, FLV, MPEG-TS/M2TS, 3GP, OGV, and raw elementary streams (`.h264`, **`.hevc`/`.h265`**). Codecs: H.264, **HEVC**, AV1, VP8/VP9, MPEG-2/4, etc.
- Apply **EXIF orientation** automatically. Respect embedded **ICC color profiles** (convert to sRGB/display) for JPEG/PNG/WebP/AVIF/HEIC at minimum.

### 5.3 Folder navigation (core feature — must feel perfect)
- [v1] When a file is opened, build the list of **all images and videos in the same folder** (mixed together), sorted like the user's file manager: **natural sort by name** by default ("img2" before "img10"); options: date modified, date taken (EXIF, falls back to modified), size, type; ascending/descending. Hidden files excluded by default.
- [v1] Next/previous with: on-screen arrow buttons at the left/right edges (appear on hover), **Left/Right arrow keys**, Page Up/Down, mouse back/forward buttons, horizontal swipe on touchpad/touchscreen, and the filmstrip. Home/End = first/last.
- [v1] **Seamless mixed navigation:** moving from a photo to a video and back must be smooth. Both always **fit inside the current window** (letterboxed, aspect ratio kept). No window resize jumps, no black flash: show the cached thumbnail/poster frame instantly, then swap in the full-quality image/first video frame.
- [v1] **Preloading:** decode the next 2 and previous 1 items in the background (direction-aware: preload further ahead in the direction the user is moving). Cancel stale work immediately when the user skips quickly.
- [v1] Wrap-around at the ends (setting, default off) with a subtle "end of folder" hint.
- [v1] Position counter, e.g. "12 / 240".
- [v1] **Live folder updates** (file added/removed/renamed by another app) without losing the current position.
- [v1] **Filmstrip** at the bottom (toggle with **F**, as in Windows Photos), lazy-loaded thumbnails, smooth scrolling, current item highlighted and auto-scrolled into view.
- [v1] Mouse wheel behavior setting: **zoom** (default) or **next/previous** (Windows Photos offers the same choice). Ctrl+wheel always zooms.

### 5.4 Image viewing
- [v1] Default "Fit": shrink large images to fit; do **not** upscale small images beyond 100% (setting: "Enlarge small images to fit").
- [v1] Zoom: wheel, Ctrl+wheel, `+`/`-`, Ctrl+0 (fit), Ctrl+1 (100% = **one image pixel per physical screen pixel**, HiDPI-correct), pinch-to-zoom, **double-click toggles fit ↔ 100%** at the clicked point, zoom slider + percentage in the toolbar. Zoom is anchored at the cursor.
- [v1] Pan by dragging (with inertia on touchpads), arrow keys pan when zoomed in.
- [v1] Smooth scaling with good quality (mipmaps/high-quality downscale; no shimmering or aliasing when zoomed out) — GPU-accelerated when available, CPU viewport scaling in software mode (7.6). Both must look the same at fit and 100%.
- [v1] Transparent images: checkerboard by default; setting for solid light/dark background (Loupe-style automatic contrast choice is a bonus).
- [v1] Animated GIF/WebP/APNG play automatically with pause/frame-step.
- [v1] **Huge images** (e.g. 20,000 × 20,000 px, 200+ MP panoramas): open quickly by decoding a screen-sized version first, then full resolution only when the user zooms in. Memory must stay bounded.
- [v1] Fullscreen: F11 (and Esc to leave). Slideshow: F5 (interval setting, shuffle option, loops; videos play to the end then advance — setting).
- [v1] **Auto-hiding chrome:** single-click on the image (or idle 2–3 s in fullscreen) hides toolbar/filmstrip; moving the mouse brings them back (Windows Photos behavior).
- [v1] Rotate view 90° (R / Ctrl+R) — see 5.7 for saving rotation.
- [v1.x] Multi-view compare: select 2–4 items in the filmstrip to view side by side (Windows Photos feature).
- [v1.x] Print.

### 5.5 Video playback (via embedded libmpv)
- [v1] Video renders **inside our window** (not a separate window) and fits the window exactly like photos — on every tier, using the video backend chosen in 7.6.
- [v1] Transport bar overlay at the bottom (auto-hides with the rest of the chrome):
  - Play/Pause (Space or K), **seek bar** with buffered range, hover time tooltip, click-to-seek and drag-scrub.
  - **Skip back / forward buttons** (default −10 s / +10 s, configurable), also J / L keys and Shift+Left/Right.
  - **Previous / Next item buttons** (go to the previous/next file in the folder).
  - Elapsed / total time, volume slider + mute (M), playback speed (0.25×–2×), loop toggle.
  - Frame step (`,` and `.` while paused).
  - Audio track and subtitle track menus; auto-load external `.srt`/`.ass` with the same base name.
  - Fullscreen button.
- [v1] Keyboard rule: **Left/Right arrows always mean previous/next item** (consistent with photos). Seeking uses J/L, Shift+Left/Right, or the buttons. Setting "Arrow keys seek inside videos" (default off) for people who prefer that.
- [v1] Hardware decoding on by default with safe fallback (`hwdec=auto-safe` style). Setting to disable. CPU-only decoding must still play 1080p H.264 smoothly on the Lite tier (7.6).
- [v1] Autoplay when navigating to a video (setting; default on), remember volume across files.
- [v1] Save current frame as image (screenshot).
- [v1.x] Seek-bar hover thumbnail preview. Remember playback position per file.
- [v1.x] Lossless trim (keyframe-accurate cut, no re-encode).
- **libmpv must be initialized lazily** — only when the first video is opened — so photo-only sessions start fast.

### 5.6 "Open with" and file actions
- [v1] **Open with…** menu listing installed applications registered for that file type (e.g. **VLC**, mpv, Celluloid, GIMP, the OS default), plus "Choose another app…". Implement natively per OS:
  - Linux: read freedesktop `.desktop` entries and `mimeapps.list` (system + user), launch with the correct `Exec` field codes.
  - Windows: Shell association APIs (enumerate handlers for the extension) and the system "Open with" dialog as fallback.
  - macOS: Launch Services equivalents.
  - Remember the last 3 "Open with" choices per type at the top of the menu.
- [v1] Show in folder (select the file in Nemo / Explorer / Finder), Copy (image to clipboard), Copy file path, **Rename (F2)** inline, **Delete (Del) → Trash/Recycle Bin** with optional confirmation and an **Undo** toast, Set as desktop wallpaper, Save as / Convert (JPEG, PNG, WebP with quality option), File properties/info panel.
- [v1] Drag the current image out of the window into another app (file drag source) — [v1.x] if platform support is hard.
- [v1] Register as an image/video handler during install (desktop file + MIME types on Linux; file associations on Windows; Info.plist on macOS) **without hijacking defaults** — offer a "Set Kinora as default viewer" helper in settings that opens the OS's own default-apps UI where required.

### 5.7 Basic editing (images only)
Editing opens an edit mode (like Windows Photos "Edit") with a right-side panel. All edits are **non-destructive during the session** (an edit stack), with **Undo/Redo (Ctrl+Z / Ctrl+Y)** and "Reset".
- [v1] **Crop** with handles, aspect presets (Free, Original, Square, 4:3, 3:2, 16:9, 9:16, 4:5), rule-of-thirds grid while dragging, keyboard nudging, numeric size display.
- [v1] **Rotate** 90° left/right, **Straighten** slider (±45°, auto-crops to avoid empty corners), **Flip** horizontal/vertical.
- [v1] **Adjustments** (sliders, live preview — GPU shaders when available, CPU proxy preview in Lite mode per 7.6): Brightness, Exposure, Contrast, Highlights, Shadows, Saturation, Warmth. Double-click a slider to reset it.
- [v1.x] A handful of simple filters (B&W, Sepia, Vivid, Cool, Warm) — pure math, no AI. Resize image dialog.
- **Saving:**
  - Default action is **"Save a copy"** (`photo (edited).jpg`); "Save" (overwrite) asks for confirmation the first time.
  - Writes are **atomic** (write temp file in the same folder → fsync → rename). On any error the original is untouched.
  - Preserve EXIF/XMP/ICC metadata; reset the orientation tag after pixels are rotated.
  - **Rotation-only edits on JPEG are lossless**: update the EXIF orientation tag (or do lossless transforms) instead of re-encoding.
  - HEIC/AVIF originals: save edits as JPEG/PNG/WebP copy (state this in the UI) unless a reliable encoder is available.
- [v1] Leaving edit mode with unsaved changes asks "Save / Discard / Cancel".

### 5.8 Info panel
- [v1] Toggle with **I** or Alt+Enter: file name, folder, size, dimensions, megapixels, format, color profile, date taken, camera make/model, lens, focal length, aperture, shutter, ISO, flash; for videos: duration, resolution, frame rate, video/audio codecs, bitrate, container.
- [v1] GPS shown as plain coordinates with a "Copy" button. **No map tiles or network calls.**

### 5.9 Settings
- [v1] Theme: System / Light / Dark; accent color follows the OS where possible.
- [v1] Sorting, wrap-around, mouse wheel behavior, enlarge small images, transparency background, slideshow interval, confirm delete, single instance, video autoplay, skip interval, arrow keys seek, hardware acceleration, memory cache size (with sane default based on RAM), restore window size/position/maximized state per monitor.
- [v1] "Reset all settings". Settings stored as a human-readable file in the OS config directory.

### 5.10 Accessibility and localization
- [v1] Fully keyboard-operable; visible focus rings; every control has an accessible name (screen readers via the UI toolkit's accessibility layer); respects OS "reduce motion" and high-contrast where detectable; text contrast ≥ 4.5:1; UI scales with OS text/scale settings.
- [v1] All user-visible strings go through the translation system from day one (English first). Right-to-left layout should not break.
- [v1] A keyboard shortcuts help overlay (**?** or F1).

---

## 6. Visual & UX design spec

**Goal:** a person who knows the Windows 11 Photos viewer feels at home instantly, but the app looks native-enough and polished on Linux too. Take the **best of the three platforms**:

| From | Take | Leave |
|------|------|-------|
| **Windows 11 Photos** | Viewer layout (top toolbar, edge arrows, bottom filmstrip toggled with F), click-to-hide chrome, Edit panel structure, zoom slider, multi-view idea, mouse-wheel setting | Slow startup, OneDrive/iCloud prompts, Copilot/AI tools, in-app purchases, heavy memory use |
| **macOS Quick Look / Preview** | Instant open, arrow-key browsing, quick rotate/crop without a "mode switch" feel, Space to play/pause, trackpad gestures, very quiet chrome | macOS-only markup/signature features |
| **GNOME Loupe / Mint Xviewer & Celluloid** | GPU rendering, HiDPI-correct 100% zoom, smart transparency background, crash-isolated & memory-safe decoding, lossless JPEG rotation, constant-memory zoom of huge images, mpv-quality video | Xviewer's crashes on large photos, settings hidden in text files |

**Layout (viewer mode):**
- Top bar (auto-hides): file name + counter on the left; center/right actions: Zoom slider %, Rotate, Delete, Edit, Info, Open with…, More (⋯) menu, Fullscreen. Native window controls on each OS (custom title bar only if it behaves correctly on all three; otherwise use native decorations).
- Edge navigation arrows: large circular buttons, appear on hover near the edges.
- Bottom: filmstrip (toggle F); for videos the transport bar sits above it.
- Edit mode: image on the left, tool tabs (Crop / Adjust) in a right panel, Save a copy / Save / Cancel at the top.

**Style tokens (define once in a theme file, never hard-code):**
- Dark theme default for the viewer canvas (photos look best on dark), light theme supported.
- 4-px spacing scale (4/8/12/16/24/32), corner radius 8 px (4 px small controls), subtle 1-px borders, soft elevation shadows on floating bars, semi-transparent "acrylic-like" bar backgrounds (blur only where the toolkit supports it cheaply — otherwise a translucent solid).
- **Every effect has a Lite/software-renderer variant** (7.6): shadows → borders, translucency → solid fills, no animations. The UI must still look clean and intentional in that mode — design it, don't just switch effects off. Show both variants in `UI_SPEC.md`.
- Typography: system UI font on each OS (do **not** bundle Segoe UI or San Francisco — they are proprietary). If a bundled fallback is needed, use an OFL-licensed font such as Inter.
- Icons: **Fluent UI System Icons** (MIT license, by Microsoft — gives the familiar Windows look legally). Include their license in third-party notices.
- Motion: 120–200 ms ease-out for chrome show/hide and panel slides, 150 ms cross-fade between items (setting to disable; disabled automatically with "reduce motion"). Never animate in a way that delays showing the image.
- Target 60 fps on Typical/High tiers (120 fps on high-refresh displays where possible); on Lite, responsiveness beats smoothness — input must react within one frame even if animations are skipped. No layout shifts when switching items.
- Layouts must work at **1366×768** (common on old laptops) without clipped toolbars — overflow actions move into the ⋯ menu.

**Before building UI in Phase 4, write `docs/design/UI_SPEC.md`** with an ASCII wireframe of every screen/state (viewer image, viewer video, error card, edit-crop, edit-adjust, settings, info panel, empty start screen) and the full keyboard shortcut table. Get my approval on it.

---

## 7. Architecture (decided — record each decision as an ADR in `docs/adr/`)

### 7.1 Why not a webview (Tauri) — read this
ArtCraft (github.com/storytold/artcraft, my inspiration) is a Rust/Tauri app with a React/Vite frontend, a Cargo workspace of many small focused crates, `AGENTS.md`/`CLAUDE.md` files for coding agents, strict lint configuration (it denies `unsafe_code` by default and bans `println!` outside tests in favor of the `log` crate), and dev scripts. **Copy those engineering habits.** But for a media viewer a webview is the wrong core:
- Webviews cannot reliably play MKV/HEVC (WebKitGTK on Linux depends on installed GStreamer plugins; WebView2 on Windows needs a paid HEVC extension) and cannot decode HEIC at all.
- Embedding mpv in Tauri currently relies on transparent windows or child windows positioned over a `<div>`, which is fragile.
- WebKitGTK has known rendering problems on some Linux setups (notably NVIDIA).
- A native UI uses far less memory and starts faster, and it can **fall back to pure CPU rendering** on machines without usable graphics (Section 7.6) — a webview on an old PC with no GPU driver is slow and heavy.

### 7.2 Chosen stack
| Concern | Choice | License | Notes |
|---|---|---|---|
| Language | Rust stable, **edition 2024**, pinned via `rust-toolchain.toml` | MIT/Apache | |
| UI | **Slint** (declarative `.slint` markup — closest thing to XAML in Rust; winit backend; AccessKit accessibility; built-in `@tr()` translations). **Two renderers compiled in:** FemtoVG (OpenGL, default) **and Slint's CPU-only software renderer** (automatic fallback — see 7.6) | GPL-3.0 option | Our app is GPL-3.0, so we use Slint under GPLv3. Avoid the Skia renderer unless measurements justify it (large binary). |
| Video | **libmpv** via the `libmpv2` crate. **Three output backends behind one trait** (see 7.6): (A) mpv **render API into an OpenGL texture/FBO**, imported into the Slint scene through Slint's rendering notifier (see Slint's `opengl_texture`/`opengl_underlay` examples and the `vgarleanu/lumiere` proof-of-concept — learn from it, write our own code); (B) **native child-window embedding (`wid`)** letting mpv use its own lightweight video outputs; (C) mpv **software render API** into a CPU buffer, last resort | libmpv LGPL-2.1+/GPL-2.0+ | Dynamically linked. System libmpv on Linux; bundled `libmpv-2.dll` on Windows; Homebrew/bundled dylib on macOS. |
| Image decode | `image` crate (JPEG via zune-jpeg, PNG, GIF, WebP, AVIF, TIFF, BMP, ICO, TGA, PNM, QOI, DDS, HDR, EXR; exposes EXIF orientation & ICC) | MIT/Apache | |
| HEIC/HEIF | `libheif-rs` → system **libheif** + libde265, **dynamically linked**, behind a cargo feature `heif` (default on) | bindings MIT; libheif LGPL-3.0 | Never link statically. |
| JPEG XL | `jxl-oxide` | MIT/Apache | |
| SVG | `resvg` | check current license | |
| Color management | the color-management support used by `image` (moxcms) or `lcms2` if needed | verify | |
| Fast resize/thumbnails | `fast_image_resize` | MIT/Apache | SIMD. |
| EXIF/metadata | `image`'s EXIF accessors + a dedicated EXIF reader (e.g. `kamadak-exif`) for the info panel; a writer crate for orientation/metadata preservation | verify each | |
| File-type sniffing | `infer` (or equivalent) | MIT | |
| Folder watching | `notify` | verify | Debounce events. |
| Natural sort | a small, well-tested natural-sort crate or our own implementation (with tests) | | Must match Nemo/Explorer ordering closely. |
| Trash | `trash` crate | MIT | |
| Clipboard | `arboard` | MIT/Apache | |
| Config & paths | `directories` + `serde` + `toml` | MIT/Apache | |
| Errors | `thiserror` (libraries), `anyhow` (app binary only) | MIT/Apache | |
| Logging | `tracing` + `tracing-subscriber` + rolling file appender in the OS log/data dir | MIT | No `println!`. |
| Concurrency | std threads + a bounded worker pool (`rayon` ThreadPool or `crossbeam-channel` workers) with **priority + cancellation**; results delivered to UI with `slint::invoke_from_event_loop` | MIT/Apache | **No tokio/async** unless a real need appears — keeps the code simpler for me to learn. |
| Linux "Open with" | freedesktop desktop-entry / MIME crates (verify current best maintained) | verify | |
| Windows platform | `windows` crate (Microsoft, MIT/Apache) | MIT/Apache | |
| Packaging | `cargo-packager` (deb, AppImage, NSIS, MSI/WiX, app, dmg) + a Flatpak manifest | MIT/Apache | |
| License audit | `cargo-deny` (licenses, bans, advisories, sources) + `cargo-about` to generate THIRD_PARTY_NOTICES | MIT/Apache | |

**Before adding any dependency:** check crates.io/docs.rs for the **current** version, maintenance status (recent releases, open issues), and license. Pin it in `[workspace.dependencies]`. State in your message: name, version, license, why, and one alternative considered. If anything in this table turns out to be wrong or outdated, **tell me and propose a replacement instead of silently swapping**.

### 7.3 Fallback plan (only if Phase 1 spike fails)
If the GL render-API backend (A) cannot meet the smoothness bar on Linux Mint X11 **and** Windows after a time-boxed spike: evaluate (a) making the `wid` backend (B) the default where available, (b) Slint + GStreamer (Slint has a GStreamer example). Write an ADR comparing measurements and **ask me** before switching.

### 7.4 Workspace layout (Cargo workspace, ArtCraft-style small crates)
```
kinora/
├── Cargo.toml                 # [workspace], [workspace.dependencies], [workspace.lints], profiles
├── rust-toolchain.toml
├── rustfmt.toml  clippy.toml  deny.toml  about.toml
├── CLAUDE.md                  # your standing instructions (create from this brief; keep updated)
├── AGENTS.md                  # same rules for any other agent (can point to CLAUDE.md)
├── README.md  CONTRIBUTING.md  CHANGELOG.md  LICENSE (GPL-3.0-or-later)  THIRD_PARTY_NOTICES.md
├── PROJECT_BRIEF.md           # this file, kept for reference
├── crates/
│   ├── kinora_app/            # binary: main.rs, wiring, Slint UI (ui/*.slint), build.rs
│   ├── kinora_core/           # pure logic: MediaItem, FolderModel, sorting, navigation state machine,
│   │                          #   settings model, HardwareProfile + tier rules (pure), zoom/pan math,
│   │                          #   edit-stack model. NO UI, NO I/O side effects
│   │                          #   beyond traits. Heavily unit-tested.
│   ├── kinora_decode/         # format sniffing, decoding, orientation, color mgmt, downscale, animation frames
│   ├── kinora_cache/          # thumbnail + decoded-image LRU cache (byte-budgeted), freedesktop thumbnail reuse
│   ├── kinora_video/          # the ONLY place libmpv FFI lives; safe wrapper API; output backends
│   │                          #   (A) GL texture, (B) wid child window, (C) sw buffer — one trait
│   ├── kinora_edit/           # crop/rotate/flip/straighten/adjust pipeline, encoders, metadata-preserving atomic save
│   └── kinora_platform/       # hardware detection (CPU/RAM/GL renderer), open-with, trash, reveal-in-folder,
│                              #   wallpaper, single-instance IPC,
│                              #   file associations; #[cfg(target_os)] modules per OS behind one trait
├── xtask/                     # `cargo xtask <cmd>`: dist, fixtures, check-all, licenses (like ArtCraft's scripts, but in Rust)
├── test_data/                 # small fixtures with clear licenses (see 9.4)
├── packaging/                 # .desktop, metainfo.xml, icons, Flatpak manifest, Windows/macOS resources
├── docs/
│   ├── ARCHITECTURE.md        # diagram (Mermaid) + threading model + data flow
│   ├── adr/                   # 0001-ui-toolkit, 0002-video-engine, 0003-license, 0004-low-end-and-fallbacks, ...
│   ├── design/UI_SPEC.md
│   ├── learning/              # Rust-for-C#-dev notes (Section 1)
│   └── manual-test-plan.md    # checklist I run before each release
└── .github/workflows/         # ci.yml, release.yml
```
Dependency direction: `kinora_app` → everything; `kinora_core` depends on nothing internal; no cycles. Platform-specific code lives only in `kinora_platform` (and the GL bits of `kinora_video`).

### 7.5 Threading & data flow
- **UI thread** (Slint event loop) never blocks on disk, decode, or mpv calls that can block.
- **Decode pool:** N−1 worker threads (cap configurable). Jobs have a priority (current item > next > previous > filmstrip thumbnails) and a **generation/epoch id**; when the user navigates, stale jobs are dropped before they start and results from old generations are discarded.
- **Cache:** decoded images in an LRU **budgeted in bytes** (Typical/High default ~min(512 MB, 15% of RAM); Lite per 7.6); thumbnails in a separate smaller LRU plus an on-disk cache (on Linux, read/write the **freedesktop thumbnail spec** cache in `~/.cache/thumbnails` so thumbnails Nemo already made are reused).
- **Video:** one mpv instance, created lazily, reused across files (`loadfile`), rendered on the GL thread via the render API; mpv events are translated into typed Rust events in `kinora_video` and forwarded to the UI.
- **Directory scan** runs off the UI thread; the item list appears immediately with the opened file, and siblings fill in.
- All pool sizes, cache sizes and preload distances come from the **hardware profile** (7.6), never from hard-coded constants.

### 7.6 Hardware profile, renderer fallback and Lite mode (required for Section 4.1)

**1. Detect capabilities at startup (cheaply, < 20 ms, cached in settings with a version stamp):**
- CPU core count and SIMD features (runtime detection), installed RAM, whether the OpenGL context could be created, and the GL renderer string. Treat these renderers as "no GPU": Mesa `llvmpipe`/`softpipe`, `Microsoft Basic Render Driver`/WARP, `GDI Generic`, and any GL version below what FemtoVG needs.
- Classify into **Lite / Typical / High** (Section 4.1). Log the result.

**2. UI renderer fallback chain (automatic, no user action, never a crash):**
1. FemtoVG on OpenGL (hardware).
2. If GL context creation fails, the GL version is too old, or the renderer is software-only → **Slint software renderer** (CPU).
   Use Slint's backend/renderer selection API to do this at runtime from one binary (verify the current API — e.g. `BackendSelector` — in the Slint docs). Users can force a renderer in Settings → Performance and with the `SLINT_BACKEND` environment variable.
- **The software renderer has documented limits** — no rotation/scaling transforms on elements, no drop shadows, no rounded corners combined with clipping, limited text shaping (western scripts). So:
  - The **image canvas must not depend on element transforms**. In software mode, Kinora itself produces the visible viewport: crop the visible region of the decoded image, scale it with `fast_image_resize` to the exact on-screen size (and rotate pixels when needed), and hand Slint an exact-size buffer. Cache the last viewport so panning only re-scales on zoom changes.
  - Every visual effect in the design has a **software-safe variant**: shadows → 1-px borders, translucent blur → solid semi-opaque fill, rounded clipped thumbnails → square thumbnails with a rounded frame overlay or square corners. Define both variants in the theme from day one.
  - Check that translations into non-western scripts are still readable in software mode; if not, note it as a known limitation.

**3. Video backend selection (per session, with automatic downgrade on failure):**
| Situation | Backend | Notes |
|---|---|---|
| Working hardware OpenGL | (A) render API → GL texture | Best quality; our controls overlay the video. |
| No hardware GL, but a native window handle (X11, Windows) | (B) `wid` child window, mpv picks its own lightweight output (`vo=gpu` if usable, else cheap outputs such as XVideo/x11 on Linux or Direct3D on Windows — verify which `vo`s the target libmpv builds include) | Most efficient on weak PCs. The child window covers that area, so **controls go below the video (not overlaid)** in this mode. Wayland has no `wid` support → use (C). |
| Nothing else works | (C) mpv software render API (`sw`) into a CPU buffer at **reduced size** (cap at 854×480 or window size, whichever is smaller), with mpv's `sw-fast` | mpv's own docs call this path slow (single-threaded CPU scaling/conversion), so use it only as a last resort and tell the user they can "Open with VLC" for heavy files. |

**4. Lite mode (auto-enabled on Lite tier or software rendering; can be toggled in Settings → Performance):**
- **Effects:** no cross-fades, no blur/translucency, no shadows, animations off (instant show/hide).
- **Decode workers:** 1 on 2-core machines, otherwise cores − 1 capped at 4. Preload only the next 1 item (previous 0); filmstrip thumbnails are low priority and paused while the user is navigating.
- **Memory:** image cache budget = clamp(10% of RAM, 64 MB, 256 MB); thumbnail cache ≤ 32 MB in RAM; the disk thumbnail cache does the rest.
- **Fast first pixel:** show the **embedded EXIF/JPEG thumbnail** (or the OS thumbnail cache entry) instantly, then the screen-sized decode. For JPEG, use **DCT-domain scaled decoding** (1/2, 1/4, 1/8) to decode only as many pixels as the screen needs — verify which JPEG decoder crate exposes this (e.g. `jpeg-decoder`'s scale API) and use it in all tiers for fit-to-window views. Full-resolution decode only when zooming past the scaled size.
- **Huge images:** on Lite, cap the "fit" decode to screen size and decode zoomed regions on demand; refuse full-res decodes that would exceed the RAM budget with a friendly message.
- **Video on weak CPUs:** keep `hwdec=auto-safe` (old Intel iGPUs often still have hardware H.264 decode via VA-API/DXVA2 even when the GL driver is poor); otherwise use mpv's fast settings (the `fast` profile, bilinear scaling, `framedrop=vo`, no interpolation/deband, decoder threads = cores). Before playing, compare codec/resolution with the hardware profile; if it is clearly beyond the machine (e.g. 4K HEVC on a 2-core CPU with no HEVC hardware decode), show a non-blocking hint: "This video may stutter on this PC — Open with VLC?" Never refuse to try.
- **Editing preview:** adjustments are previewed on a **screen-sized proxy** on the CPU (rayon, throttled to the display refresh) instead of GPU shaders; the full-resolution export runs on a worker thread with a progress bar and a Cancel button.
- **Disk:** sorting by name never reads file contents; "date taken" is read lazily in the background with a small per-file read (EXIF is near the start of the file). Never stat or decode the whole folder before showing the opened file.
- **Startup:** do not create the mpv instance, load fonts beyond the UI font, or build the filmstrip before the first image is on screen.

**5. Settings → Performance page:** shows the detected tier and renderer in plain words ("Your PC: basic graphics — Lite mode is on to keep things smooth"), with toggles for Lite mode, hardware video decoding, animations, renderer (Auto / Hardware / Software), and cache size.

---

## 8. Engineering standards (non-negotiable)

1. **Formatting & lints:** `cargo fmt` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean. Configure `[workspace.lints]` in `Cargo.toml` (modern replacement for ArtCraft's `Cranky.toml`) with at least:
   - `rust`: `unsafe_code = "deny"`, `unused_must_use = "deny"`, `missing_debug_implementations = "warn"`.
   - `clippy`: `all` + `pedantic` as warn (allow the few noisy pedantic lints explicitly, each with a comment why), and deny: `unwrap_used`, `expect_used`, `panic`, `todo`, `unimplemented`, `dbg_macro`, `print_stdout`, `print_stderr`, `await_holding_lock`, `indexing_slicing` (in non-test code; use `.get()`), `as_conversions` (use `try_from`/explicit helpers for numeric casts — pixel math overflows are real bugs).
   - Tests may `#[allow]` `unwrap_used`/`expect_used`.
   - Code generated by `slint::include_modules!()` should be wrapped in its own module with a scoped `#[allow(...)]` for lints it trips — never relax lints for our own code to accommodate it.
2. **`unsafe` policy:** only `kinora_video` (mpv FFI + GL) and `kinora_platform` (OS APIs) may `#![allow(unsafe_code)]`. Every `unsafe` block has a `// SAFETY:` comment explaining the invariant. Wrap all FFI in safe types; no raw pointers leak out of those crates.
3. **Errors:** no panics on bad input. Library crates return typed errors (`thiserror`); the app maps them to friendly UI messages + logs the technical detail. Worker threads catch panics (`catch_unwind`) so one bad decoder can't kill the app.
4. **Data safety:** atomic writes for every file we modify; never delete permanently (Trash only); confirm overwrites.
5. **Tests** (write them with the code, not after):
   - Unit tests for all `kinora_core` logic (sorting, navigation incl. wrap/edge cases, zoom math, crop math, edit stack undo/redo).
   - Decode tests per format using fixtures in `test_data/` (including EXIF-rotated JPEGs for all 8 orientations, CMYK JPEG, 16-bit PNG, animated WebP/GIF/APNG, HEIC, AVIF, a truncated/corrupt file for each format, a 0-byte file, a huge-dimension "decompression bomb" header that must be rejected gracefully).
   - Edit/save round-trip tests (metadata preserved, original untouched on simulated failure).
   - Property tests (`proptest`) for natural sort and geometry math.
   - UI logic tests with Slint's testing backend where practical.
   - **Fuzzing** (`cargo-fuzz`) harnesses for any parsing/decoding entry point we own.
   - Benchmarks (`criterion`) for decode, thumbnail generation, folder scan of 10,000 files.
6. **Logging:** `tracing` everywhere, no `println!`. Log file rotation; a "Open log folder" item in About/Help.
7. **Docs:** public items in library crates have doc comments with a short example where useful. Keep `ARCHITECTURE.md` and `CLAUDE.md` current.
8. **Commits:** small, focused, **Conventional Commits** (`feat:`, `fix:`, `refactor:`, `test:`, `docs:`, `chore:`). Each commit builds and passes tests. Update `CHANGELOG.md` per phase.
9. **CI (GitHub Actions):** matrix `ubuntu-24.04` (= Mint 22 base), `windows-latest`, `macos-latest`: fmt, clippy, test, `cargo deny check`, build. Release workflow builds packages on tags.
10. **Profiles:** release = `opt-level = 3`, `lto = "thin"`, `codegen-units = 1`, `strip = true`, keep `panic = "unwind"` (needed for `catch_unwind`). Dev = optimize dependencies (`[profile.dev.package."*"] opt-level = 2`) so image decoding isn't painfully slow while developing. Suggest the `mold` linker on Linux for faster dev builds. **Never** set `target-cpu=native` or require AVX2 in `.cargo/config.toml` or CI; release builds target baseline x86-64 (and baseline ARMv8 for ARM64) and rely on runtime SIMD detection (Section 4.1).
11. **Never** silence a lint, skip a test, or add `#[allow]` just to get green. If something must be allowed, add a comment with the reason and mention it to me.
12. **When unsure about an API, read the docs/source** (docs.rs, the crate repo, mpv's `client.h`/`render_gl.h`, Slint docs) — do not guess. Say when you verified something.

---

## 9. Licensing & legal rules

1. **Project license: GPL-3.0-or-later.** Reason: we dynamically link libmpv (commonly GPL builds) and use Slint under its GPLv3 option; GPL-3.0 makes everything compatible and keeps the app free forever. Record in ADR 0003. (If I choose a different license in Open Questions, re-check every dependency against it.)
2. **Never copy code, assets, icons, sounds, or screenshots from proprietary products** (Windows Photos, Apple Photos/Preview/Quick Look, any commercial viewer). Re-create behavior from public descriptions and our own design only (clean-room).
3. Open-source projects (Loupe/glycin, Xviewer, Celluloid, mpv, Lumiere, ArtCraft…) may be studied for ideas. If you ever reuse actual code, it must be license-compatible with GPL-3.0, attributed in the file header and THIRD_PARTY_NOTICES, and you must tell me first. Default: write our own.
4. `deny.toml` must allow only licenses compatible with GPL-3.0 distribution (e.g. MIT, Apache-2.0, BSD-2/3, ISC, Zlib, MPL-2.0, Unicode, CC0, LGPL-*, GPL-2.0-or-later, GPL-3.0*) and fail CI on anything else, on unknown licenses, on yanked crates, and on security advisories.
5. **Dynamic linking only** for LGPL C libraries (libheif, libde265, libmpv). Windows/macOS bundles ship those libraries with their license texts and a written note on where to get the exact source.
6. **HEVC/H.265 patents:** decoding HEVC (HEIC photos and HEVC video) may be covered by patent pools in some countries. On Linux we rely on the distribution's own libraries (same situation as any distro media player). For Windows/macOS bundles, add a short neutral note to the README and keep HEVC support in a separately linked library so it can be excluded if ever necessary. Do not give legal advice in the app.
7. Don't use "Windows", "Photos", "Microsoft", "Apple" or their logos in our name, icon, or store listings. Phrases like "familiar layout for Windows Photos users" in docs are fine.
8. Generate `THIRD_PARTY_NOTICES.md` with `cargo-about` in CI and include it in every package; show it in Help → About → Licenses.

---

## 10. Performance budgets (measure, don't guess)

Two reference machines (Section 4.1). Add a `--bench-startup` flag / tracing spans so we can measure each item; record results for **both** in `docs/perf.md` at the end of each phase.
- **Typical:** 4-core CPU, integrated GPU with working drivers, 8 GB RAM, SSD, Linux Mint 22, 1080p.
- **Lite:** 2-core CPU without AVX2, 2 GB RAM, HDD, **no GPU acceleration** (simulate it as described in Phase 1 if no real machine is available), 1366×768.

| Metric | Typical | Lite |
|---|---|---|
| Launch → first pixel of a 12 MP JPEG (warm cache) | ≤ 250 ms (cold ≤ 600 ms) | ≤ 600 ms (cold ≤ 1.5 s; embedded thumbnail visible sooner) |
| Next/previous to an already-preloaded image | ≤ 16 ms (one frame) | ≤ 50 ms |
| Next/previous to a non-preloaded 12 MP JPEG | ≤ 120 ms | ≤ 350 ms (scaled decode) |
| Open a 1080p H.264 video → first frame | ≤ 300 ms after mpv is warm, ≤ 700 ms first video | ≤ 1 s, ≤ 2 s first video |
| 1080p H.264 30 fps playback | no dropped frames | no visible stutter (CPU decode allowed) |
| 1080p HEVC playback | smooth | best effort; hint to "Open with VLC" if decoding can't keep up |
| Folder of 10,000 files: list ready | ≤ 150 ms (current file shows immediately) | ≤ 500 ms on HDD (current file still shows immediately) |
| Filmstrip/zoom/pan | 60 fps | input reacts within one frame; ≥ 30 fps pan |
| Idle CPU (image shown, no animation) | ~0% | ~0% |
| Memory: one 12 MP image open | ≤ 200 MB RSS | ≤ 150 MB RSS; total never exceeds the Lite budget in 7.6 |
| Release binary (excluding bundled libmpv/libheif) | aim ≤ 25 MB | same binary |

If a budget is missed, report it with numbers and a proposed fix — never hide it.

---

## 11. Security & robustness

- Treat every file as hostile: validate dimensions before allocating (reject images whose width×height×4 exceeds a configurable limit, default ~1.5 GB, with a friendly message); guard against zip/decompression bombs and integer overflow.
- Prefer memory-safe Rust decoders; C decoders (libheif, libmpv) run with conservative options. **[v1.x]** evaluate running C-based decoders in a short-lived helper process (the way GNOME's glycin isolates loaders) so a decoder crash cannot take down the app.
- No network access in the app; mpv must be configured to not load remote URLs, scripts, or config from untrusted locations (disable `ytdl`, user scripts, `load-scripts`, OSC; we draw our own UI).
- Do not follow symlinks out of the folder when building the sibling list unless the user opened the symlink itself.

---

## 12. Roadmap — phases with acceptance criteria

At the **start** of each phase: restate the goal, list the files/crates you'll touch, the acceptance criteria, and risks; wait for "go".
At the **end** of each phase: run fmt + clippy + tests + `cargo deny check`; give me (a) exact commands to run the app, (b) a manual test checklist for this phase, (c) what you learned/changed in the plan, (d) new `docs/learning/` notes; commit; then **stop**.

**Phase 0 — Environment, repo, skeleton (with me at the keyboard)**
- Walk me through installing, verifying each step:
  - Linux Mint: `build-essential`, `pkg-config`, `git`, `curl`, `cmake`, `libmpv-dev`, `libheif-dev`, fontconfig/xkbcommon/Wayland/X11 dev packages required by Slint's winit backend (verify exact package names for Ubuntu 24.04 before telling me); `rustup` (stable, plus `clippy`, `rustfmt`, `rust-src`); optional `mold`.
  - VS Code (or my preferred IDE) with **rust-analyzer**, **Slint** extension (live preview), **CodeLLDB** (debugging), **Even Better TOML**, **crates** helper.
  - Windows (later in the phase or Phase 7): Visual Studio Build Tools with the C++ workload, rustup, mpv dev build (libmpv-2.dll + import lib from a reputable open-source build), libheif via vcpkg or a prebuilt — document exact steps in `docs/setup-windows.md`.
  - Tools: `cargo-deny`, `cargo-about`, `cargo-nextest` (optional), `cargo-packager`, `cargo-fuzz` (later).
- `git init`, `.gitignore` (target/, IDE files, OS junk), `.gitattributes` (normalize line endings; LF for source), initial commit. Ask whether to create the GitHub repository with `gh repo create` (public, GPL-3.0) and push.
- Create the workspace skeleton (Section 7.4), lints, profiles, `rust-toolchain.toml`, `deny.toml`, CI workflow, `CLAUDE.md`, `AGENTS.md`, `README.md`, ADRs 0001–0003, `docs/learning/00-start-here.md`.
- **Done when:** `cargo xtask check-all` (fmt+clippy+test+deny) passes locally and in CI on all three OSes; `cargo run` opens an empty Slint window titled "Kinora".

**Phase 1 — Technical spike (de-risk the hard parts, time-boxed)**
- Prove: (1) libmpv renders an MKV/HEVC file **inside** the Slint window via the render API, resizes smoothly, plays/pauses/seeks, on Mint X11, Mint Wayland session (if available), and Windows; (2) HEIC, AVIF, JPEG XL, animated WebP decode and display; (3) a 12 MP JPEG displays with correct EXIF orientation and HiDPI-correct 100% zoom; (4) measure timings vs Section 10.
- **Prove the no-GPU path too (7.6):** (5) the same binary starts and shows an image with the **Slint software renderer** when GL is unavailable or software-only — forced on Linux with `LIBGL_ALWAYS_SOFTWARE=1` (Mesa llvmpipe) and by forcing the software renderer, and on a Windows VM with only the Basic Display Adapter; (6) video plays via backend (B) `wid` on X11 and Windows, and via (C) `sw` on Wayland/llvmpipe; (7) zoom/pan in software mode using CPU viewport scaling feels responsive on a 24 MP photo.
- **Simulate the Lite machine** when no real one is available, and document the exact commands in `docs/perf.md`: limit to 2 cores (`taskset -c 0,1` / Windows processor affinity), cap memory at 2 GB (`systemd-run --user --scope -p MemoryMax=2G`), force software GL, and test from a USB stick or HDD folder. A real old laptop or a 2-core/2 GB VM is better — ask me if I have one.
- Spike code lives in `examples/` or a `spike/` branch, clearly marked throwaway.
- **Done when:** ADR 0002 (video engine), ADR 0004 (renderer fallback & Lite mode) are written with measurements for Typical **and** Lite, plus a go/no-go recommendation. I approve before Phase 2.

**Phase 2 — Core image viewer**
- `kinora_core` folder model + natural sort + navigation state machine (fully tested); `kinora_decode` for all v1 image formats; `kinora_cache` with preloading, priorities, cancellation; viewer canvas with fit/zoom/pan (GPU and CPU-viewport paths), keyboard + mouse + touchpad; error card; single-instance; drag-and-drop; open dialog; **hardware profile detection, renderer fallback and Lite-mode budgets** (7.6); scaled JPEG decoding and embedded-thumbnail first pixel.
- **Done when:** I can open a photo from Nemo, flick through a folder of 500 mixed-format photos with no visible delay, zoom/pan smoothly, and corrupt files show an error card without crashing — **in both normal and forced-Lite/software mode**, meeting the Lite budgets in the simulated Lite setup. Unit + decode tests pass.

**Phase 3 — Video integration**
- `kinora_video` safe wrapper with the three output backends (A/B/C) behind one trait and automatic downgrade; transport bar (all v1 controls in 5.5) in overlay layout (A/C) and below-video layout (B); mixed photo/video navigation with poster-frame handoff (no black flash); keyboard rules; lazy mpv init; hwdec with fallback; Lite-mode mpv settings and the "may stutter — Open with VLC?" hint.
- **Done when:** a folder mixing JPG/HEIC/MP4/MKV/HEVC files can be navigated with arrows smoothly; seek bar, ±10 s buttons, prev/next buttons, volume, speed, subtitles, audio tracks work; video always fits the window — verified with each backend, and 1080p H.264 plays without visible stutter in the simulated Lite setup.

**Phase 4 — UI polish & design system**
- Write and get approval for `docs/design/UI_SPEC.md` first. Then: theme tokens (light/dark/system), Fluent icons, top bar, edge arrows, filmstrip, auto-hide chrome, animations (respecting reduce-motion), info panel, settings page, shortcuts overlay, accessibility names, translation plumbing.
- **Done when:** UI matches the spec at 100%, 125%, 150%, 200% scaling, in light and dark, on Mint and Windows, **and in the software-renderer/Lite variant**, and at 1366×768; keyboard-only operation works; a screen reader announces controls; Settings → Performance shows the detected tier correctly.

**Phase 5 — File actions & OS integration**
- Open with… (Linux/Windows/macOS implementations), Show in folder, Copy, Copy path, Rename (F2), Delete → Trash with Undo, Set as wallpaper, Save as/Convert, live folder watching, file-type registration, "Set as default" helper.
- **Done when:** "Open with → VLC" works on Mint and Windows; deleting goes to Trash and Undo restores it; renaming updates the list in place.

**Phase 6 — Basic editing**
- `kinora_edit`: edit stack, crop (+presets, grid), rotate, straighten, flip, adjustments with live preview (GPU shaders, or CPU screen-sized proxy in Lite mode) and full-quality CPU export on a worker thread with progress and Cancel; Save a copy / Save with atomic write + metadata preservation; lossless JPEG rotation path; undo/redo.
- **Done when:** round-trip tests pass; edited copies keep EXIF; an interrupted save never damages the original; preview stays at 60 fps while dragging sliders on a 24 MP photo (Typical), and sliders stay responsive (proxy updates ≥ 15 fps, no input lag) in the simulated Lite setup; exporting a 24 MP photo on Lite never exceeds the memory budget.

**Phase 7 — Packaging & distribution**
- Linux: `.deb` (primary, Mint/Debian; declares `libmpv2`, `libheif1` deps), AppImage, Flatpak manifest (Flathub submission later), `.desktop` + AppStream metainfo + MIME registration + icons in all sizes.
- Windows: NSIS installer (+ MSI), bundled libmpv/libheif DLLs with licenses, file associations, Start-menu entry, uninstaller; note on code signing (cost/optional).
- macOS: `.app` + `.dmg`, Info.plist document types; unsigned build first, notarization later.
- **Done when:** a fresh Mint VM installs the `.deb`, Kinora appears in "Open With" in Nemo, and opens everything in `test_data/`; same on a fresh Windows VM with the installer.

**Phase 8 — Hardening & 1.0**
- Fuzzing runs, corrupted-file corpus, memory-leak check (long slideshow over 5,000 files), performance re-measure vs Section 10 on **both** reference tiers (ideally a real low-end laptop running Mint), a CI job that runs the test suite and a headless smoke test under Mesa llvmpipe with the software renderer, accessibility audit, manual test plan run on all platforms, README with screenshots (our own), user guide, CONTRIBUTING, issue templates, v1.0.0 tag and release.

---

## 13. How you must work with me

- Use **plan mode** for anything bigger than a small fix. Show the plan; wait for approval.
- Work in **small steps**; after each meaningful step, build and run tests. Don't write hundreds of lines before compiling.
- **Explain as you go** in short comments to me (not in the code): what you're doing and why, in C#-friendly terms when helpful.
- Before running commands that install software, modify system settings, need `sudo`, delete files, or push to GitHub — **tell me exactly what will run and ask first**.
- Keep `CLAUDE.md` short and current: build/test commands, crate map, coding rules (Section 8), "never do" list (Section 3 & 9), and the current phase.
- If a requirement here conflicts with reality (an API doesn't exist, a crate is unmaintained, a budget is impossible), **stop and tell me** with options. Don't quietly lower the bar.
- When you finish a phase, give me a 5-minute "what you should understand about this code" walkthrough pointing at 3–5 key files.
- Never claim something works unless you ran it. If you can't run something (e.g. GUI on a headless machine, Windows-only code), say so and give me the steps to verify.

---

## 14. Open questions — ask me these first (propose the default)

1. **Name:** keep "Kinora"? (Alternatives: *Lumora*, *Framewise*, *Stillmotion*.) Before final, check that the name is free on crates.io, GitHub, Flathub, and the Snap/winget stores.
2. **License:** GPL-3.0-or-later (default) — OK?
3. **GitHub:** my username/organization, repo name, public (default) or private to start?
4. **Exact OS versions** I'm on: Linux Mint version (22.1/22.2/…), Windows version; and my **GPU** (NVIDIA/AMD/Intel) — affects video and GL testing.
5. **macOS:** ship in v1.0, or defer to v1.1 (default: defer; keep code portable and CI building)?
6. **Mouse wheel default:** zoom (default) or next/previous?
7. **IDE:** VS Code (default), RustRover, or other?
8. Do I want **Flatpak/Flathub** as a v1.0 target or later (default: later; `.deb` first)?
9. **Low-end test hardware:** do I have an old PC/laptop (or can create a 2-core / 2 GB VM) to test the Lite tier on? Which CPU, RAM, and graphics does it have? (Default: simulate as in Phase 1 until a real device is available.)
10. **Lowest hardware to support:** is the Lite tier in Section 4.1 (2 cores, 2 GB RAM, no GPU acceleration) the right floor, or should it go lower (e.g. 32-bit or very old CPUs — not recommended, as Linux Mint itself is 64-bit only)?

---

## 15. Things the original request didn't mention that are included on purpose

**Low-end hardware support (no GPU, 2 cores, 2 GB RAM, HDD) with automatic renderer fallback and Lite mode** · Accessibility & keyboard-only use · translations · single-instance behavior · file-type registration without hijacking defaults · atomic, metadata-preserving saves · Trash instead of permanent delete (with Undo) · EXIF orientation & color profiles · HiDPI and Wayland · huge-image and decompression-bomb handling · animated images · camera RAW previews (later) · subtitles & audio tracks · lazy video engine start · thumbnail reuse from the OS cache · crash isolation of decoders (later) · no network/privacy guarantee · license auditing in CI · HEVC patent note · packaging for Mint/Debian/Windows/macOS · performance budgets with measurements · fuzzing · a learning track for a C# developer.
