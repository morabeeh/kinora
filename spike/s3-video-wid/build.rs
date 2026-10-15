//! Compiles the UI and tells the linker where `mpv.lib` lives (Windows dev setup:
//! `KINORA_MPV_DIR`, default `%USERPROFILE%\dev\kinora-tools\mpv`). On Linux, libmpv
//! is found through the system library path instead.

fn main() -> Result<(), slint_build::CompileError> {
    println!("cargo:rerun-if-env-changed=KINORA_MPV_DIR");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let dir = std::env::var("KINORA_MPV_DIR").unwrap_or_else(|_| {
            let home = std::env::var("USERPROFILE").unwrap_or_default();
            format!("{home}\\dev\\kinora-tools\\mpv")
        });
        // Build scripts talk to Cargo through stdout; this is not logging.
        #[allow(clippy::print_stdout)]
        {
            println!("cargo:rustc-link-search=native={dir}");
        }
    }
    slint_build::compile("ui/video.slint")
}
