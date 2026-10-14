# ADR 0002 — Video engine: embedded libmpv with three output backends

- **Status:** **Proposed** (2026-10-14). The Phase 1 spike will add measurements and a go/no-go, and then it becomes Accepted or is revised.
- **Deciders:** project owner, Claude Code

## Context
Video must play inside our window and fit it exactly like photos. It must cover the formats in PROJECT_BRIEF.md §5.2 and work on every hardware tier, including machines without usable OpenGL (§7.6).

## Proposed decision
Use **libmpv**, dynamically linked through the `libmpv2` crate, all inside `kinora_video`. It is created **lazily** on the first video and reused via `loadfile`. There are three output backends behind one trait, chosen per session with automatic downgrade:

| Backend | When | How |
|---|---|---|
| **A — GL render API** | working hardware OpenGL | mpv renders into an OpenGL texture/FBO, which is composited into the Slint scene via the rendering notifier. Our controls overlay the video. |
| **B — `wid` child window** | no hardware GL, but a native window handle (X11, Windows) | mpv draws into a child window with its own lightweight `vo`. Controls go **below** the video in this mode. Not available on Wayland. |
| **C — software render API** | nothing else works | mpv renders into a CPU buffer at reduced size (≤ 854×480), with the `sw-fast` option. Last resort. |

mpv is configured not to load remote URLs, scripts, user config, ytdl or its OSC.

## Alternatives (only if the Phase 1 spike fails; see §7.3)
- Make backend B the default where it's available.
- Use Slint + GStreamer (Slint ships an example).
- Each would need a new ADR with measurements, and the owner's approval.

## To be filled in Phase 1
- Measurements on Typical and simulated Lite (first frame, dropped frames, CPU usage, resize smoothness) on Windows and Linux Mint X11 (plus Wayland if available).
- Which `vo` outputs the Windows and Linux libmpv builds actually include.
- Go/no-go recommendation.
