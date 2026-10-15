# Manual test plan

Run before each release, and the section for a phase at its end. Tick each box on **Windows** and on **Linux Mint** (X11 and, if available, Wayland). Setup: `cargo xtask gen-folder <dir>` makes a 500-photo test folder (`photo 1.jpg` … `photo 500.*`, mixed formats, 10 deliberately broken files).

## Phase 2 — image viewer

Start: `cargo xtask run --release -- "<dir>\photo 1.jpg"` (normal) and again with `--lite --renderer software` (Lite). For the simulated Lite PC on Windows: `$env:KINORA_SIMULATE="lite"; start /affinity 3 target\release\kinora.exe "<dir>\photo 1.jpg"`.

**Opening**
- [ ] Double-click a photo in Explorer/Nemo after "Open with → Choose another app → kinora.exe" opens it.
- [ ] With Kinora already open, opening a second photo reuses the same window and brings it to the front.
- [ ] Dragging a photo (or a folder) onto the window opens it; a blue frame shows while dragging over.
- [ ] Ctrl+O and the "Open…" button open the system dialog in the current folder.
- [ ] Starting without a file shows "Open a photo".

**Flicking** (Right/Left, PageDown/PageUp, Home/End, mouse back/forward buttons, edge buttons on hover, touchpad two-finger swipe)
- [ ] Holding Right flicks through the 500 photos with no visible delay, in Explorer's order (photo 2 before photo 10).
- [ ] At the last photo, Right shows "End of folder" (wrap-around off).
- [ ] Broken files (`photo 14.jpg`, `photo 38.png`, …) show the error card or a partial picture — never a crash; the next Right continues.
- [ ] "12 / 500" counter and the file name in the title and top bar are right.

**Zoom and pan**
- [ ] Mouse wheel zooms at the cursor (the point under the mouse stays put); it stops briefly at 100% and at fit.
- [ ] Ctrl+wheel zooms; on a Windows touchpad, pinch zooms.
- [ ] `+` / `-`, Ctrl+0 (fit), Ctrl+1 (100%); double-click toggles fit ↔ 100% at the clicked point.
- [ ] At 100% on a 125%/150% display, a 1-pixel checkerboard (`test_data/images` has none; use any detailed photo) looks sharp, no moiré.
- [ ] Dragging pans when zoomed in (grab cursor); arrow keys pan when zoomed in and change the photo at fit.
- [ ] Zooming into a 24 MP photo becomes sharp after a moment (full resolution loads).
- [ ] Small photos are not enlarged at fit.

**Formats and looks** (`test_data/images`)
- [ ] `orient-1.jpg` … `orient-8.jpg`: red square always top-left.
- [ ] `rgba.png`: checkerboard behind transparent parts.
- [ ] `anim.gif`, `anim.webp`, `anim-apng.png` animate; Space pauses, `,` / `.` step frames.
- [ ] HEIC, AVIF, JPEG XL, SVG, HDR, EXR, TIFF, BMP, TGA, QOI, DDS, ICO all open.
- [ ] A `.mp4` in the folder shows "Videos can't be played yet".
- [ ] F11 toggles fullscreen; Esc leaves it.

**Lite / software renderer**
- [ ] With `--lite --renderer software` everything above works; bars are solid (no translucency).
- [ ] On the simulated Lite PC, flicking still feels instant.

**Logs and settings**
- [ ] Logs: `%LOCALAPPDATA%\Kinora\data\logs\` (Windows), `~/.local/state/kinora/logs/` (Linux).
- [ ] `settings.toml` (Windows `%APPDATA%\Kinora\config\`, Linux `~/.config/kinora/`): set `[viewer] wheel = "navigate"`, restart → the wheel changes photos; a typo in the file is ignored (defaults used) and not overwritten.
