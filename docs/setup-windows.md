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

## 6. Media libraries (added in Phase 1)
libmpv (`libmpv-2.dll` + import library, from a reputable open-source build) and libheif (vcpkg or prebuilt) get documented here once the Phase 1 spike needs them.
