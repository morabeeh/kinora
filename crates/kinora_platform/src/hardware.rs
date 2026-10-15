//! Cheap hardware detection at startup (brief §7.6, budget < 20 ms).

use kinora_core::hardware::{Gpu, HardwareInfo};

/// Detects CPU cores (respecting CPU affinity), RAM, AVX2 and whether a hardware GPU
/// exists. Never fails: unknown values get conservative defaults in the tier rules.
#[must_use]
pub fn probe() -> HardwareInfo {
    HardwareInfo {
        logical_cores: logical_cores(),
        total_ram_bytes: crate::memory::total_ram_bytes(),
        avx2: has_avx2(),
        gpu: gpu(),
    }
}

fn logical_cores() -> usize {
    let std_count = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    #[cfg(windows)]
    {
        crate::sys_windows::affinity_cores().map_or(std_count, |cores| cores.min(std_count))
    }
    #[cfg(not(windows))]
    {
        // On Linux the standard library already honours `taskset` (sched_getaffinity).
        std_count
    }
}

fn has_avx2() -> bool {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        std::arch::is_x86_feature_detected!("avx2")
    }
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    {
        false
    }
}

fn gpu() -> Gpu {
    #[cfg(windows)]
    {
        crate::sys_windows::gpu()
    }
    #[cfg(target_os = "linux")]
    {
        crate::sys_linux::gpu()
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        Gpu::Hardware(None) // every supported Mac has a Metal GPU
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_reports_this_machine() {
        let started = std::time::Instant::now();
        let info = probe();
        let elapsed = started.elapsed();
        assert!(info.logical_cores >= 1);
        // Generous bound for slow CI runners; the real budget (20 ms) is logged by the app.
        assert!(elapsed.as_millis() < 2000, "probe took {elapsed:?}");
    }
}
