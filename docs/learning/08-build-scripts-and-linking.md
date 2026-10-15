# 08 — Build scripts and linking native libraries (≈ NativeLibrary, `.lib` import libraries)

**In plain words:** to call `libmpv-2.dll` the linker needs an *import library* (`mpv.lib`) at build time, and Windows needs the DLL at run time. `build.rs` tells Cargo where the import library lives.

| .NET | Rust / Cargo | In the spikes |
|---|---|---|
| `[DllImport]` resolves the DLL at run time | the linker needs `mpv.lib` at **build** time; the DLL is loaded at **start-up** | `spike/s2-video-gl/build.rs` |
| `<NativeLibrary>` / runtimes folder | `println!("cargo:rustc-link-search=native=…")` in `build.rs` | `KINORA_MPV_DIR` |
| NuGet package with native assets | crate + system package (Linux `libmpv-dev`) or a vcpkg/download (Windows) | `docs/setup-windows.md` |
| `DllNotFoundException` at run time | Windows error 0xc0000135 "DLL not found" at start-up | put the DLL folder on `PATH` or next to the exe |

## Making `mpv.lib` from a DLL (Windows)
Some builds ship only a MinGW import library (`libmpv.dll.a`). With Visual Studio's tools:
```powershell
dumpbin /exports libmpv-2.dll   # list the exported functions (mpv_*)
# write them into mpv.def:  LIBRARY libmpv-2.dll / EXPORTS / one name per line
lib /def:mpv.def /out:mpv.lib /machine:x64
```

## vcpkg (≈ NuGet for C/C++ libraries)
`libheif-rs` finds libheif through the `vcpkg` crate. Set `VCPKG_ROOT` to the vcpkg folder and `VCPKGRS_DYNAMIC=1` (use the DLL, never static, because LGPL rules forbid static linking for us), and put `vcpkg\installed\x64-windows\bin` on `PATH` at run time.

## Build-time tools
`rav1d` (AVIF) assembles SIMD code with **NASM** during the build, like a build-time NuGet tool package. It picks the right SIMD version **at run time**, so the binary still runs on old CPUs without AVX2.

## Linux is simpler
`sudo apt install libmpv-dev libheif-dev nasm`. pkg-config tells the build where everything is, and the system loader finds the `.so` files.
