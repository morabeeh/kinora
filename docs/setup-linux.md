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
sudo apt install libmpv-dev libheif-dev
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
