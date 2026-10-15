//! Linux system information from `/proc`, `/dev/dri` and Mesa environment variables.

use std::path::Path;

use kinora_core::hardware::Gpu;

use crate::memory::ProcessMemory;
use crate::proc_text::kib_field;

/// Installed RAM from `/proc/meminfo`.
pub fn total_ram_bytes() -> Option<u64> {
    kib_field(&std::fs::read_to_string("/proc/meminfo").ok()?, "MemTotal")
}

/// Resident set size and its peak from `/proc/self/status`.
pub fn process_memory() -> Option<ProcessMemory> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    Some(ProcessMemory {
        resident_bytes: kib_field(&status, "VmRSS")?,
        peak_resident_bytes: kib_field(&status, "VmHWM").unwrap_or(0),
    })
}

/// A quick guess before any window exists. The definitive answer (the OpenGL renderer
/// name) comes from the first frame and is cached in the settings for the next start.
pub fn gpu() -> Gpu {
    let env_is = |name: &str, values: &[&str]| {
        std::env::var(name).is_ok_and(|value| {
            values
                .iter()
                .any(|wanted| value.trim().eq_ignore_ascii_case(wanted))
        })
    };
    if env_is("LIBGL_ALWAYS_SOFTWARE", &["1", "true", "yes"]) {
        return Gpu::SoftwareOnly(Some(String::from("LIBGL_ALWAYS_SOFTWARE is set")));
    }
    if env_is("GALLIUM_DRIVER", &["llvmpipe", "softpipe", "swrast"]) {
        return Gpu::SoftwareOnly(Some(String::from("GALLIUM_DRIVER selects software")));
    }
    if !has_dri_device(Path::new("/dev/dri")) {
        return Gpu::SoftwareOnly(Some(String::from("no /dev/dri device")));
    }
    Gpu::Unknown
}

/// `true` if the directory has a DRM render or card node (a kernel GPU driver is loaded).
fn has_dri_device(dir: &Path) -> bool {
    std::fs::read_dir(dir).is_ok_and(|entries| {
        entries.filter_map(Result::ok).any(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name.starts_with("renderD") || name.starts_with("card")
        })
    })
}
