//! Tone mapping for high-dynamic-range files (Radiance HDR, EXR; brief §5.2).
//!
//! Their pixels are linear light values that can be far brighter than a screen's white.
//! We use the classic global Reinhard operator: scale the picture so its average brightness
//! lands at middle grey, compress highlights with `L / (1 + L)`, then encode to sRGB.
//! The maths runs in `f64` (every `f32` converts to it exactly).

use kinora_core::viewport::to_px;

use crate::image_buf::Rgba8Image;

/// Converts linear float RGB(A) samples (`channels` = 3 or 4) to displayable sRGB RGBA8.
#[must_use]
pub fn tonemap(width: u32, height: u32, samples: &[f32], channels: usize) -> Option<Rgba8Image> {
    if !(3..=4).contains(&channels) {
        return None;
    }
    let expected = usize::try_from(width)
        .ok()?
        .checked_mul(usize::try_from(height).ok()?)?;
    if samples.len() != expected.checked_mul(channels)? {
        return None;
    }
    let scale = exposure_scale(samples, channels);
    let mut out = Vec::with_capacity(expected.checked_mul(4)?);
    for px in samples.chunks_exact(channels) {
        let [r, g, b] = rgb(px);
        let luminance = luma(r, g, b);
        let factor = if luminance > 0.0 {
            let scaled = luminance * scale;
            scaled / (1.0 + scaled) / luminance
        } else {
            0.0
        };
        for channel in [r, g, b] {
            out.push(encode_srgb(channel * factor));
        }
        let alpha = px.get(3).map_or(1.0, |a| finite(f64::from(*a)));
        out.push(unit_to_u8(alpha));
    }
    Rgba8Image::new(width, height, out)
}

fn rgb(px: &[f32]) -> [f64; 3] {
    [0, 1, 2].map(|i| finite(px.get(i).copied().map_or(0.0, f64::from)))
}

fn luma(r: f64, g: f64, b: f64) -> f64 {
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// Maps the log-average luminance to middle grey (0.18). Samples at most ~64k pixels.
fn exposure_scale(samples: &[f32], channels: usize) -> f64 {
    const KEY: f64 = 0.18;
    let pixels = samples.len() / channels;
    let step = (pixels / 65_536).max(1);
    let (sum, count) = samples
        .chunks_exact(channels)
        .step_by(step)
        .map(|px| {
            let [r, g, b] = rgb(px);
            (1e-4 + luma(r, g, b)).ln()
        })
        .fold((0.0f64, 0u32), |(sum, n), v| (sum + v, n.saturating_add(1)));
    if count == 0 {
        return 1.0;
    }
    let average = (sum / f64::from(count)).exp();
    (KEY / average.max(1e-6)).clamp(1e-6, 1e6)
}

/// Linear light → sRGB-encoded 8-bit value.
fn encode_srgb(linear: f64) -> u8 {
    let c = linear.clamp(0.0, 1.0);
    let encoded = if c <= 0.003_130_8 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    unit_to_u8(encoded)
}

fn unit_to_u8(value: f64) -> u8 {
    u8::try_from(to_px(value.clamp(0.0, 1.0) * 255.0).min(255)).unwrap_or(u8::MAX)
}

fn finite(value: f64) -> f64 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn black_stays_black_and_alpha_is_kept() {
        let img = tonemap(2, 1, &[0.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 1.0], 4).unwrap();
        assert_eq!(img.pixel(0, 0), Some([0, 0, 0, 128]));
    }

    #[test]
    fn very_bright_values_do_not_clip_to_flat_white() {
        // A gradient from dim to 1000× brighter must stay ordered after tone mapping.
        let samples: Vec<f32> = (0u8..16)
            .flat_map(|i| {
                let v = 10f32.powf(f32::from(i) / 5.0);
                [v, v, v]
            })
            .collect();
        let img = tonemap(16, 1, &samples, 3).unwrap();
        let values: Vec<u8> = (0..16).map(|x| img.pixel(x, 0).unwrap()[0]).collect();
        assert!(values.windows(2).all(|w| w[0] <= w[1]), "{values:?}");
        assert!(values[15] < 255, "{values:?}");
    }

    #[test]
    fn nan_and_wrong_sizes_are_safe() {
        assert!(tonemap(1, 1, &[f32::NAN, f32::INFINITY, -1.0], 3).is_some());
        assert!(tonemap(2, 2, &[0.0; 3], 3).is_none());
        assert!(tonemap(1, 1, &[0.0; 5], 5).is_none());
    }
}
