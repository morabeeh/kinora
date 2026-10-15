# Windows development setup

Tested on Windows 11 25H2 (x86_64). Run the commands in PowerShell.

## 1. C++ build tools (Rust uses Microsoft's linker)
In **Visual Studio Installer**, choose Modify on your Visual Studio (2022 or 2026, Community is fine). Tick **Desktop development with C++**, keeping the default components, which include **MSVC** and the **Windows 11 SDK**.
If you have no Visual Studio: `winget install --id Microsoft.VisualStudio.2022.BuildTools -e`, then add the same workload.
Rust ≥ 1.93 detects Visual Studio 2026.

## 2. Rust
```powershell
winget install --id Rustlang.Rustup -e
# open a NEW terminal so PATH includes %USERPROFILE%\.cargo\bin
rustup default stable
rustup component add clippy rustfmt rust-src
rustc -vV   # host should be x86_64-pc-windows-msvc
```
The repository pins the exact toolchain in `rust-toolchain.toml`. rustup installs that version automatically the first time you run `cargo` in the repo.

## 3. Tools
```powershell
winget install --id GitHub.cli -e      # then: gh auth login
cargo install --locked cargo-deny
cargo install --locked cargo-about --features cli   # the CLI is behind a feature flag
```
Later phases add `cargo-packager`, `cargo-fuzz` and (optionally) `cargo-nextest`.

## 4. VS Code
```powershell
code --install-extension rust-lang.rust-analyzer
code --install-extension slint.slint
code --install-extension vadimcn.vscode-lldb
code --install-extension tamasfe.even-better-toml
code --install-extension fill-labs.dependi
```
The repo's `.vscode/extensions.json` recommends the same set.

## 5. Build and run
```powershell
cargo run -p kinora_app                                   # FemtoVG (OpenGL) renderer
$env:SLINT_BACKEND="winit-software"; cargo run -p kinora_app; Remove-Item Env:SLINT_BACKEND   # CPU renderer
cargo xtask check-all                                     # everything CI checks
```

## 6. Media libraries and tools (Phase 1)
Everything lives outside the repo, e.g. `%USERPROFILE%\dev\kinora-tools\`.

**libmpv**: from the shinchiro/mpv-winbuild-cmake GitHub releases, download **`mpv-dev-x86_64-<date>-git-<hash>.7z`**. ⚠️ **Not** the `x86_64-v3` build, which requires AVX2 and breaks old PCs. Check its SHA-256 against the release page, extract it to `kinora-tools\mpv`, then create the MSVC import library (Developer PowerShell, or use the full path to `dumpbin`/`lib` under `VC\Tools\MSVC\<ver>\bin\Hostx64\x64`):
```powershell
cd $env:USERPROFILE\dev\kinora-tools\mpv
$exports = dumpbin /exports libmpv-2.dll | Select-String '^\s+\d+\s+[0-9A-F]+\s+[0-9A-F]{8}\s+(mpv_\w+)' | % { $_.Matches[0].Groups[1].Value }
Set-Content mpv.def -Encoding ascii -Value (@('LIBRARY libmpv-2.dll','EXPORTS') + $exports)
lib /nologo /def:mpv.def /out:mpv.lib /machine:x64
```
Build scripts look in `KINORA_MPV_DIR` (default `%USERPROFILE%\dev\kinora-tools\mpv`). At run time, put that folder on `PATH`.

**libheif** (HEIC) via vcpkg, as DLLs (dynamic linking only):
```powershell
git clone https://github.com/microsoft/vcpkg.git $env:USERPROFILE\dev\kinora-tools\vcpkg
cd $env:USERPROFILE\dev\kinora-tools\vcpkg
.\bootstrap-vcpkg.bat -disableMetrics
.\vcpkg.exe install "libheif[core,hevc]:x64-windows" --disable-metrics   # ~3 min; `hevc` = x265 encoder, only to generate test files
$env:VCPKG_ROOT = "$env:USERPROFILE\dev\kinora-tools\vcpkg"; $env:VCPKGRS_DYNAMIC = "1"
# run time: add vcpkg\installed\x64-windows\bin to PATH
```
(`libheif[aom]` failed to build here because a Perl download timed out. We don't need it: AVIF uses rav1d, see ADR 0005.)

**NASM** (rav1d's AVIF decoder assembles SIMD code at build time): `winget install --id NASM.NASM -e`, then add `%LOCALAPPDATA%\bin\NASM` to `PATH`.

**FFmpeg** (only to generate test files): the BtbN/FFmpeg-Builds `ffmpeg-n9.0-latest-win64-gpl-9.0.zip` (≈ 194 MB, verify its SHA-256), extracted to `kinora-tools`. Spike tools read it from `KINORA_FFMPEG`.
