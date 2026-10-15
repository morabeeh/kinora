# 15 — `#[cfg(...)]`: per-OS code and features (≈ `#if WINDOWS`, target frameworks, NuGet conditions)

**In plain words:** `#[cfg(windows)]` keeps a piece of code only when compiling for Windows, like `#if WINDOWS` in C#. Cargo features (`heif`) work like optional build flags that switch whole dependencies on or off.

| C# / MSBuild | Rust / Cargo | In Kinora |
|---|---|---|
| `#if WINDOWS … #endif` | `#[cfg(windows)]` on an item or block | `memory.rs`, `hardware.rs` in [`kinora_platform`](../../crates/kinora_platform/src/) |
| `RuntimeInformation.IsOSPlatform(...)` at run time | `cfg!(windows)` — a compile-time `true/false` you can use in `if` | `startup.rs`: `GPU_RENDERER` |
| `<PackageReference Condition="'$(OS)' == 'Windows_NT'">` | `[target.'cfg(windows)'.dependencies]` | Skia only on Windows, FemtoVG elsewhere (`kinora_app/Cargo.toml`) |
| `<DefineConstants>HEIF</DefineConstants>` | `[features] heif = ["dep:libheif-rs"]` + `#[cfg(feature = "heif")]` | `kinora_decode` (HEIC) |
| MSBuild target that adds linker options | `build.rs` printing `cargo:rustc-link-arg-bins=…` | `/DELAYLOAD:heif.dll` in [`kinora_app/build.rs`](../../crates/kinora_app/build.rs) |

## One API, several implementations
`kinora_platform::memory::total_ram_bytes()` has the same signature everywhere; inside, `#[cfg]` blocks pick `sys_windows` (Win32 API), `sys_linux` (`/proc/meminfo`) or a fallback. Callers never see `#[cfg]`. It's the same idea as an interface with per-OS implementations, but the choice is made by the compiler, so there is no run-time cost and no dead code in the binary.

## Testing code for "the other OS"
Code inside `#[cfg(target_os = "linux")]` isn't even compiled on Windows, so a typo there only shows up on Linux CI. That's why pure helpers (like the `/proc` text parser in `proc_text.rs`) are compiled and tested on **every** OS, and only the thin OS calls are behind `#[cfg]`.
