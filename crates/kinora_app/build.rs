//! Build script: compiles the `.slint` UI files into Rust code at build time (similar to how
//! XAML is compiled into generated C# partial classes), and on Windows marks `heif.dll` as
//! delay-loaded.

fn main() -> Result<(), slint_build::CompileError> {
    // Windows + MSVC: load heif.dll only when the first HEIC file is opened, so Kinora starts
    // even if the DLL is missing (HEIC then shows "not available" instead of the whole app
    // failing to start). `kinora_platform::library_available` checks it before first use.
    let msvc = std::env::var("CARGO_CFG_TARGET_ENV").is_ok_and(|env| env == "msvc");
    if msvc && std::env::var_os("CARGO_FEATURE_HEIF").is_some() {
        println!("cargo:rustc-link-arg-bins=/DELAYLOAD:heif.dll");
        println!("cargo:rustc-link-arg-bins=delayimp.lib");
    }
    slint_build::compile("ui/app_window.slint")
}
