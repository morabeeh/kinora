//! Hardware tiers and the budgets that follow from them (brief §4.1, §7.6).
//!
//! Detection itself (asking the OS) lives in `kinora_platform`; this module only holds the
//! pure rules, so they can be unit-tested. Pool sizes, cache sizes and preload distances
//! always come from here, never from constants scattered through the app.

use crate::preload::PreloadPolicy;

/// One mebibyte.
pub const MIB: u64 = 1024 * 1024;
/// One gibibyte.
pub const GIB: u64 = 1024 * MIB;

/// RAM assumed when the OS doesn't tell us.
const UNKNOWN_RAM: u64 = 4 * GIB;

/// The three hardware tiers from the brief.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    /// Old/cheap PCs: 2 cores, ~2 GB RAM, or no usable GPU.
    Lite,
    /// A normal PC from ~2015 on with working integrated graphics.
    Typical,
    /// A fast PC with plenty of RAM and a working GPU.
    High,
}

/// Whether the GPU can be used for drawing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Gpu {
    /// A real (hardware) GPU, with its name if known.
    Hardware(Option<String>),
    /// Only a software rasteriser (Mesa llvmpipe, Microsoft Basic Render Driver, …).
    SoftwareOnly(Option<String>),
    /// Not known yet (e.g. Linux before the first OpenGL frame).
    Unknown,
}

/// What `kinora_platform` found out about this PC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HardwareInfo {
    /// Logical CPU cores (threads) available to us.
    pub logical_cores: usize,
    /// Installed RAM, if known.
    pub total_ram_bytes: Option<u64>,
    /// Whether the CPU has AVX2 (informational; we never require it).
    pub avx2: bool,
    /// GPU status.
    pub gpu: Gpu,
}

impl HardwareInfo {
    /// Installed RAM, or a conservative guess.
    #[must_use]
    pub fn ram_or_default(&self) -> u64 {
        self.total_ram_bytes.unwrap_or(UNKNOWN_RAM)
    }
}

/// Classifies a PC into a tier.
///
/// ```
/// use kinora_core::hardware::{classify, Gpu, HardwareInfo, Tier, GIB};
/// let celeron = HardwareInfo { logical_cores: 2, total_ram_bytes: Some(2 * GIB), avx2: false, gpu: Gpu::Unknown };
/// assert_eq!(classify(&celeron), Tier::Lite);
/// ```
#[must_use]
pub fn classify(info: &HardwareInfo) -> Tier {
    let ram = info.ram_or_default();
    let no_gpu = matches!(info.gpu, Gpu::SoftwareOnly(_));
    // "2 GB" machines report a little less than 2 GiB usable; anything under 3.5 GiB is Lite.
    if info.logical_cores <= 2 || ram < 3 * GIB + GIB / 2 || no_gpu {
        Tier::Lite
    } else if info.logical_cores >= 8 && ram >= 16 * GIB && matches!(info.gpu, Gpu::Hardware(_)) {
        Tier::High
    } else {
        Tier::Typical
    }
}

/// Resource budgets for this session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budgets {
    /// Lite mode: fewer threads, smaller caches, no effects.
    pub lite_mode: bool,
    /// Decode worker threads.
    pub decode_workers: usize,
    /// How many neighbours to preload.
    pub preload: PreloadPolicy,
    /// Decoded-image cache size in bytes.
    pub image_cache_bytes: u64,
    /// In-memory thumbnail cache size in bytes.
    pub thumbnail_cache_bytes: u64,
    /// Largest RGBA buffer a single full-resolution decode may allocate. Bigger images
    /// are shown from a screen-sized decode with a friendly note (brief §7.6).
    pub max_image_bytes: u64,
}

/// Hard limit for any single decode, whatever the RAM (brief §11).
pub const MAX_IMAGE_BYTES_LIMIT: u64 = 1_500_000_000;

/// The budgets for `info`. `cache_override_mb` is the user's "memory cache size" setting.
#[must_use]
pub fn budgets(info: &HardwareInfo, lite_mode: bool, cache_override_mb: Option<u32>) -> Budgets {
    let ram = info.ram_or_default();
    let cores = info.logical_cores.max(1);
    let decode_workers = if lite_mode {
        if cores <= 2 { 1 } else { (cores - 1).min(4) }
    } else {
        cores.saturating_sub(1).clamp(1, 8)
    };
    let automatic_cache = if lite_mode {
        (ram / 10).clamp(64 * MIB, 256 * MIB)
    } else {
        (ram / 100 * 15).min(512 * MIB)
    };
    let image_cache_bytes = cache_override_mb.map_or(automatic_cache, |mb| {
        (u64::from(mb) * MIB).clamp(32 * MIB, (ram / 2).max(32 * MIB))
    });
    let max_image_bytes = if lite_mode {
        image_cache_bytes.max(128 * MIB)
    } else {
        (ram / 4).clamp(256 * MIB, MAX_IMAGE_BYTES_LIMIT)
    };
    Budgets {
        lite_mode,
        decode_workers,
        preload: if lite_mode {
            PreloadPolicy {
                ahead: 1,
                behind: 0,
            }
        } else {
            PreloadPolicy {
                ahead: 2,
                behind: 1,
            }
        },
        image_cache_bytes,
        thumbnail_cache_bytes: if lite_mode { 32 * MIB } else { 96 * MIB },
        max_image_bytes: max_image_bytes.min(MAX_IMAGE_BYTES_LIMIT),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pc(cores: usize, ram_gib: u64, gpu: Gpu) -> HardwareInfo {
        HardwareInfo {
            logical_cores: cores,
            total_ram_bytes: Some(ram_gib * GIB),
            avx2: true,
            gpu,
        }
    }

    #[test]
    fn tiers() {
        assert_eq!(classify(&pc(2, 8, Gpu::Hardware(None))), Tier::Lite);
        assert_eq!(classify(&pc(4, 2, Gpu::Hardware(None))), Tier::Lite);
        assert_eq!(classify(&pc(8, 16, Gpu::SoftwareOnly(None))), Tier::Lite);
        assert_eq!(classify(&pc(4, 8, Gpu::Hardware(None))), Tier::Typical);
        assert_eq!(classify(&pc(8, 16, Gpu::Unknown)), Tier::Typical);
        assert_eq!(classify(&pc(24, 128, Gpu::Hardware(None))), Tier::High);
    }

    #[test]
    fn lite_budgets_match_the_brief() {
        let celeron = pc(2, 2, Gpu::SoftwareOnly(None));
        let b = budgets(&celeron, true, None);
        assert_eq!(b.decode_workers, 1);
        assert_eq!(
            b.preload,
            PreloadPolicy {
                ahead: 1,
                behind: 0
            }
        );
        // clamp(10% of 2 GiB, 64 MB, 256 MB) = 204.8 MiB
        assert_eq!(b.image_cache_bytes, 2 * GIB / 10);
        assert_eq!(b.thumbnail_cache_bytes, 32 * MIB);
        assert_eq!(b.max_image_bytes, 2 * GIB / 10);
        // 4-core machine in Lite mode: cores - 1, capped at 4.
        assert_eq!(
            budgets(&pc(8, 4, Gpu::Unknown), true, None).decode_workers,
            4
        );
    }

    #[test]
    fn typical_budgets_match_the_brief() {
        let b = budgets(&pc(4, 8, Gpu::Hardware(None)), false, None);
        assert_eq!(b.decode_workers, 3);
        // min(512 MB, 15% of 8 GiB ≈ 1.2 GB)
        assert_eq!(b.image_cache_bytes, 512 * MIB);
        let small = budgets(&pc(4, 2, Gpu::Hardware(None)), false, None);
        assert_eq!(small.image_cache_bytes, 2 * GIB / 100 * 15);
        assert_eq!(b.max_image_bytes, MAX_IMAGE_BYTES_LIMIT.min(2 * GIB));
        let big = budgets(&pc(32, 128, Gpu::Hardware(None)), false, None);
        assert_eq!(big.decode_workers, 8);
        assert_eq!(big.image_cache_bytes, 512 * MIB);
        assert_eq!(big.max_image_bytes, MAX_IMAGE_BYTES_LIMIT);
    }

    #[test]
    fn cache_override_is_clamped() {
        let info = pc(4, 4, Gpu::Hardware(None));
        assert_eq!(budgets(&info, false, Some(1)).image_cache_bytes, 32 * MIB);
        assert_eq!(
            budgets(&info, false, Some(300)).image_cache_bytes,
            300 * MIB
        );
        assert_eq!(
            budgets(&info, false, Some(100_000)).image_cache_bytes,
            2 * GIB
        );
    }

    #[test]
    fn unknown_ram_and_zero_cores_are_safe() {
        let info = HardwareInfo {
            logical_cores: 0,
            total_ram_bytes: None,
            avx2: false,
            gpu: Gpu::Unknown,
        };
        assert_eq!(classify(&info), Tier::Lite);
        let b = budgets(&info, false, None);
        assert_eq!(b.decode_workers, 1);
        assert!(b.image_cache_bytes > 0);
    }
}
