//! HEIC/HEIF through libheif (dynamically linked, LGPL; cargo feature `heif`, brief §7.2).
//!
//! On Windows the app delay-loads `heif.dll` so Kinora starts even without it; before the
//! first HEIC decode it asks the availability check (see [`set_availability_check`]) whether
//! the library can be loaded, and shows a friendly "not available" error if not.

use std::sync::OnceLock;

use libheif_rs::{ColorSpace, HeifContext, LibHeif, RgbChroma};

use crate::error::DecodeError;
use crate::format::Format;
use crate::image_buf::{DecodedImage, Rgba8Image};
use crate::{DecodeOptions, check_size, color, finish_still};

static CHECK: OnceLock<fn() -> bool> = OnceLock::new();
static AVAILABLE: OnceLock<bool> = OnceLock::new();

/// Registers a function that reports whether libheif can be loaded (called once, on the
/// first HEIC file). Without one, libheif is assumed to be present.
pub fn set_availability_check(check: fn() -> bool) {
    // Only the first registration counts; later calls are ignored on purpose.
    let _ = CHECK.set(check);
}

fn available() -> bool {
    *AVAILABLE.get_or_init(|| CHECK.get().is_none_or(|check| check()))
}

pub(crate) fn decode(bytes: &[u8], options: &DecodeOptions) -> Result<DecodedImage, DecodeError> {
    if !available() {
        return Err(DecodeError::Unavailable(Format::Heif));
    }
    // Keep obviously incomplete files away from the C library.
    if !crate::isobmff::boxes_complete(bytes) {
        return Err(DecodeError::corrupt(Format::Heif, "the file is incomplete"));
    }
    let corrupt = |err: libheif_rs::HeifError| DecodeError::corrupt(Format::Heif, err);
    let context = HeifContext::read_from_bytes(bytes).map_err(corrupt)?;
    let handle = context.primary_image_handle().map_err(corrupt)?;
    check_size(handle.width(), handle.height(), options)?;
    let has_alpha = handle.has_alpha_channel();
    let image = LibHeif::new()
        .decode(&handle, ColorSpace::Rgb(RgbChroma::Rgba), None)
        .map_err(corrupt)?;
    let planes = image.planes();
    let plane = planes
        .interleaved
        .ok_or_else(|| DecodeError::corrupt(Format::Heif, "no interleaved plane"))?;
    let row_bytes = usize::try_from(plane.width)
        .ok()
        .and_then(|w| w.checked_mul(4))
        .ok_or_else(|| DecodeError::corrupt(Format::Heif, "width"))?;
    let rows = usize::try_from(plane.height).unwrap_or(0);
    let mut rgba = Vec::with_capacity(row_bytes.saturating_mul(rows));
    for row in plane.data.chunks(plane.stride.max(1)).take(rows) {
        let row = row
            .get(..row_bytes)
            .ok_or_else(|| DecodeError::corrupt(Format::Heif, "short row"))?;
        rgba.extend_from_slice(row);
    }
    let rgba = Rgba8Image::new(plane.width, plane.height, rgba)
        .ok_or_else(|| DecodeError::corrupt(Format::Heif, "pixel count"))?;
    let rgba = match handle.color_profile_raw() {
        Some(profile) => color::to_srgb(rgba, &profile.data),
        None => rgba,
    };
    let size = rgba.size();
    Ok(finish_still(
        DecodedImage::still(rgba, size, Format::Heif, has_alpha),
        options,
    ))
}
