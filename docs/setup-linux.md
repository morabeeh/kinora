# Linux Mint / Ubuntu / Debian development setup

For **Linux Mint 22.x** (Ubuntu 24.04 "noble" base). The package names below were checked against packages.ubuntu.com for noble on 2026-10-14. CI installs the same UI set on `ubuntu-24.04`.

Check your Mint version with `cat /etc/linuxmint/info` (or *Menu → System Info*).

## 1. System packages
```bash
sudo apt update
# compiler, tools
sudo apt install build-essential pkg-config git curl cmake
# Slint's winit backend (X11 + Wayland, fonts, keyboard)
sudo apt install libfontconfig1-dev libxkbcommon-dev libxkbcommon-x11-dev \
                 libxcb-shape0-dev libxcb-xfixes0-dev libwayland-dev
# media libraries (needed from Phase 1: video and HEIC)
sudo apt install libmpv-dev libheif-dev libheif-plugin-libde265 nasm
#   libheif-plugin-libde265: libheif's HEVC decoder (HEIC photos). apt normally installs it
#   as a "recommended" package; it's listed so a --no-install-recommends setup still works.
#   nasm: build-time assembler for the AVIF decoder (rav1d)
# optional: a much faster linker for dev builds
sudo apt install mold
```
Noble ships `libmpv-dev` 0.37 and `libheif-dev` 1.17.

## 2. Rust
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# restart the terminal, then:
rustup component add clippy rustfmt rust-src
rustc --version && cargo --version
cargo install --locked cargo-deny
cargo install --locked cargo-about --features cli   # the CLI is behind a feature flag
```
`rust-toolchain.toml` pins the exact version, and rustup fetches it on the first `cargo` command in the repo.

### Optional: faster linking with mold (local machine only, not committed)
Add to `~/.cargo/config.toml`:
```toml
[target.x86_64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=mold"]
```
(needs `sudo apt install clang`). **Never** add `target-cpu=native`, because release builds must run on old CPUs.

## 3. VS Code
Install VS Code from code.visualstudio.com (the `.deb`), then accept the extension recommendations the repo offers (rust-analyzer, Slint, CodeLLDB, Even Better TOML, Dependi).

## 4. Five-minute check on your Mint PC (Phase 0)
```bash
git clone <repo-url> kinora && cd kinora
cargo xtask check-all                               # should end with "all checks passed"
cargo run -p kinora_app                             # dark window titled "Kinora" (OpenGL)
SLINT_BACKEND=winit-software cargo run -p kinora_app   # same window, CPU renderer
LIBGL_ALWAYS_SOFTWARE=1 cargo run -p kinora_app        # OpenGL via Mesa llvmpipe (no GPU)
```
- [ ] All three windows open, are titled "Kinora", resize smoothly and close cleanly.
- [ ] If you can log into a Wayland session (Cinnamon has an experimental one), repeat `cargo run` there.

## 5. Phase 1 spike checklist on the Mint PC (≈ 30 min, when you have time)
The spike lives on branch `spike/phase1`, in its own workspace under `spike/`:
```bash
git switch spike/phase1 && cd spike
cargo build --release
# S1 renderers (drag each window edge, note which stutter)
for r in femtovg skia skia-opengl software; do SLINT_BACKEND=winit-$r ./target/release/s1-renderers & done
# same under Mesa's software OpenGL ("no GPU") and on 2 cores
LIBGL_ALWAYS_SOFTWARE=1 SLINT_BACKEND=winit-femtovg taskset -c 0,1 ./target/release/s1-renderers
# S2 video in an OpenGL texture (backend A) — needs test videos (copy them from the Windows PC)
SLINT_BACKEND=winit-femtovg ./target/release/s2-video-gl ~/Videos/h264-1080p30.mkv
SLINT_BACKEND=winit-software ./target/release/s4-video-sw ~/Videos/h264-1080p30.mkv
# Lite simulation: 2 cores + 2 GB cap
systemd-run --user --scope -p MemoryMax=2G taskset -c 0,1 ./target/release/s4-video-sw ~/Videos/h264-1080p30.mkv
```
Note your Mint version (`cat /etc/linuxmint/info`), X11 or Wayland (`echo $XDG_SESSION_TYPE`), and the GL renderer (`glxinfo -B`, package `mesa-utils`). Then tell Claude Code what felt smooth. Those numbers decide the Linux defaults in ADR 0002/0004. (`s3-video-wid` is Windows-only.)
