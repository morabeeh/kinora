//! Operating-system integration for Kinora.
//!
//! One cross-platform API with `#[cfg(target_os = ...)]` modules behind it:
//! - [`hardware`]: CPU cores, RAM and GPU detection for the hardware tier (brief §7.6)
//! - [`memory`]: installed RAM and this process's memory use (for `--bench-startup`)
//! - [`single_instance`]: forwarding files to an already running Kinora
//! - [`paths`] and [`fs`]: config/log locations and atomic file writes
//! - [`gl`]: reading the OpenGL renderer name from a live context
//!
//! Later phases add "Open with…", Trash, reveal in folder and wallpaper.
//!
//! `unsafe` is allowed **only** in the OS-binding modules below (`sys_windows`, `gl`), never
//! crate-wide; every block carries a `// SAFETY:` comment.

pub mod fs;
// OpenGL is a C API; reading the renderer name needs one FFI call (see the module docs).
#[allow(unsafe_code)]
pub mod gl;
pub mod hardware;
pub mod memory;
pub mod paths;
pub mod single_instance;

#[cfg(target_os = "linux")]
mod sys_linux;
// Windows APIs are C APIs; each call is wrapped in a safe function in this module.
#[cfg(windows)]
#[allow(unsafe_code)]
mod sys_windows;

/// `true` if the shared library `name` can be loaded. On Windows the library is loaded and
/// kept (Kinora delay-loads `heif.dll`, so it starts even when the DLL is missing); on other
/// systems shared libraries are linked normally, so this always returns `true`.
#[must_use]
pub fn library_available(name: &str) -> bool {
    #[cfg(windows)]
    {
        sys_windows::load_library(name)
    }
    #[cfg(not(windows))]
    {
        let _ = name;
        true
    }
}

/// Parsers for Linux `/proc` files. They are plain text functions, compiled and tested on
/// every OS so Windows CI checks them too.
mod proc_text;
