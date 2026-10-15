//! SVG, rendered with resvg (brief §5.2). The picture is rendered at the size it will be
//! shown (vector graphics have no "full resolution"), never loads files or URLs referenced
//! by the SVG (privacy, brief §3/§11), and loads system fonts only for SVGs with text.

use std::sync::{Arc, OnceLock};

use kinora_core::viewport::{Size, to_px};
use resvg::{tiny_skia, usvg};

use crate::error::DecodeError;
use crate::format::Format;
use crate::image_buf::{DecodedImage, Rgba8Image};
use crate::{DecodeOptions, Target};

/// Longest side we render, whatever the request.
const MAX_SIDE: f64 = 8_192.0;

pub(crate) fn decode(bytes: &[u8], options: &DecodeOptions) -> Result<DecodedImage, DecodeError> {
    let mut opts = usvg::Options {
        // Only embedded (data:) images; never read local files named inside the SVG.
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_data: usvg::ImageHrefResolver::default_data_resolver(),
            resolve_string: Box::new(|_, _| None),
        },
        ..usvg::Options::default()
    };
    if has_text(bytes) {
        opts.fontdb = system_fonts();
    }
    let tree = usvg::Tree::from_data(bytes, &opts)
        .map_err(|err| DecodeError::corrupt(Format::Svg, err))?;
    let natural = tree.size();
    let (nw, nh) = (f64::from(natural.width()), f64::from(natural.height()));
    let source = Size::new(to_px(nw).max(1), to_px(nh).max(1));
    // Screen: exactly what "fit" needs (crisp even when enlarged). Full: 2× the natural
    // size, so zooming to 200% stays sharp.
    let wanted = match options.target {
        Target::Screen { width, height, .. } => {
            (f64::from(width.max(1)) / nw).min(f64::from(height.max(1)) / nh)
        }
        Target::Full => 2.0,
    };
    let scale = limit_scale(wanted, nw, nh, options.max_rgba_bytes);
    let (width, height) = (to_px(nw * scale).max(1), to_px(nh * scale).max(1));
    let mut pixmap =
        tiny_skia::Pixmap::new(width, height).ok_or_else(|| DecodeError::TooLarge {
            width: u64::from(width),
            height: u64::from(height),
        })?;
    let transform = tiny_skia::Transform::from_scale(f32_of(scale), f32_of(scale));
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    // tiny-skia stores premultiplied alpha; our buffers are straight alpha.
    let data: Vec<u8> = pixmap
        .pixels()
        .iter()
        .flat_map(|px| {
            let c = px.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect();
    let rgba = Rgba8Image::new(width, height, data)
        .ok_or_else(|| DecodeError::corrupt(Format::Svg, "pixel count"))?;
    Ok(DecodedImage::still(rgba, source, Format::Svg, true))
}

/// Caps the render scale by the longest side and the memory limit.
fn limit_scale(wanted: f64, width: f64, height: f64, max_bytes: u64) -> f64 {
    let wanted = if wanted.is_finite() && wanted > 0.0 {
        wanted
    } else {
        1.0
    };
    let side_cap = MAX_SIDE / width.max(height).max(1.0);
    let max_pixels = f64::from(u32::try_from(max_bytes / 4).unwrap_or(u32::MAX));
    let memory_cap = (max_pixels / (width * height).max(1.0)).sqrt();
    wanted.min(side_cap).min(memory_cap).max(1e-4)
}

/// `f64` → `f32` for a render scale. Rust has no lossless conversion for this; the value is
/// clamped to a small range first, where `as` only rounds (it cannot overflow), so this is
/// the one audited `as` in this crate.
#[allow(clippy::as_conversions, clippy::cast_possible_truncation)]
fn f32_of(value: f64) -> f32 {
    value.clamp(1e-4, 1e4) as f32
}

fn has_text(bytes: &[u8]) -> bool {
    bytes.windows(5).any(|w| w == b"<text") || bytes.starts_with(&[0x1F, 0x8B])
}

/// System fonts, loaded once (it takes tens of milliseconds) and only when needed.
fn system_fonts() -> Arc<usvg::fontdb::Database> {
    static FONTS: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    Arc::clone(FONTS.get_or_init(|| {
        let mut db = usvg::fontdb::Database::new();
        db.load_system_fonts();
        Arc::new(db)
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_is_capped_by_side_and_memory() {
        assert!((limit_scale(2.0, 100.0, 100.0, 1 << 30) - 2.0).abs() < 1e-9);
        assert!(limit_scale(100.0, 1000.0, 1000.0, 1 << 30) <= 8.193);
        let s = limit_scale(1.0, 100_000.0, 100_000.0, 64 << 20);
        let bytes = (100_000.0 * s) * (100_000.0 * s) * 4.0;
        assert!(bytes <= f64::from(64u32 << 20) * 1.001, "{bytes}");
        assert!((limit_scale(f64::NAN, 10.0, 10.0, 1 << 20) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn byte_len_guard_matches() {
        assert!(crate::image_buf::byte_len(16_384, 16_384).is_some());
    }

    #[test]
    fn f32_conversion() {
        assert!((f32_of(0.5) - 0.5).abs() < f32::EPSILON);
        assert!((f32_of(2.0) - 2.0).abs() < f32::EPSILON);
    }
}
