//! JPEG: DCT-domain scaled decoding for screen-sized views (brief §7.6 "fast first pixel",
//! ADR 0005) and the embedded EXIF thumbnail. Full-size JPEGs go through the `image` crate.

use jpeg_decoder::PixelFormat;
use kinora_core::viewport::Size;

use crate::error::DecodeError;
use crate::exif;
use crate::format::Format;
use crate::image_buf::{DecodedImage, Rgba8Image};
use crate::{DecodeOptions, Target, check_size, color, orient, raster};

/// Decodes a JPEG for `options.target`.
pub(crate) fn decode(bytes: &[u8], options: &DecodeOptions) -> Result<DecodedImage, DecodeError> {
    if let Target::Screen { width, height, .. } = options.target {
        match scaled(bytes, Size::new(width, height), options) {
            Ok(Some(image)) => return Ok(image),
            Ok(None) => {}
            // The full decoder is more forgiving (it shows truncated files partially).
            Err(err) => tracing::debug!(%err, "scaled JPEG decode failed; using the full decoder"),
        }
    }
    raster::decode_still(bytes, Format::Jpeg, options)
}

/// Decodes at 1/2, 1/4 or 1/8 size when that is still at least as big as the picture will
/// be on `screen` at "fit". `Ok(None)` when no reduction is possible or worthwhile.
fn scaled(
    bytes: &[u8],
    screen: Size,
    options: &DecodeOptions,
) -> Result<Option<DecodedImage>, DecodeError> {
    let corrupt = |err: jpeg_decoder::Error| DecodeError::corrupt(Format::Jpeg, err);
    let orientation = exif::jpeg_orientation(bytes);
    let swaps = exif::swaps_axes(orientation);
    let mut decoder = jpeg_decoder::Decoder::new(bytes);
    decoder.read_info().map_err(corrupt)?;
    let info = decoder
        .info()
        .ok_or_else(|| DecodeError::corrupt(Format::Jpeg, "no header"))?;
    if info.pixel_format == PixelFormat::CMYK32 {
        return Ok(None); // CMYK (often Adobe-inverted) is handled by the full decoder
    }
    let (stored_w, stored_h) = (u32::from(info.width), u32::from(info.height));
    let source = if swaps {
        Size::new(stored_h, stored_w)
    } else {
        Size::new(stored_w, stored_h)
    };
    // The decoder works on the stored (unrotated) picture.
    let (want_w, want_h) = if swaps {
        (screen.height, screen.width)
    } else {
        (screen.width, screen.height)
    };
    let clamp = |v: u32| u16::try_from(v.max(1)).unwrap_or(u16::MAX);
    let (out_w, out_h) = decoder
        .scale(clamp(want_w), clamp(want_h))
        .map_err(corrupt)?;
    if (out_w, out_h) == (info.width, info.height) {
        return Ok(None);
    }
    check_size(u32::from(out_w), u32::from(out_h), options)?;
    let pixels = decoder.decode().map_err(corrupt)?;
    let rgba = to_rgba(
        u32::from(out_w),
        u32::from(out_h),
        &pixels,
        info.pixel_format,
    )
    .ok_or_else(|| DecodeError::corrupt(Format::Jpeg, "pixel count"))?;
    let rgba = match decoder.icc_profile() {
        Some(profile) => color::to_srgb(rgba, &profile),
        None => rgba,
    };
    let rgba = orient(rgba, orientation)
        .ok_or_else(|| DecodeError::corrupt(Format::Jpeg, "orientation"))?;
    Ok(Some(DecodedImage::still(rgba, source, Format::Jpeg, false)))
}

fn to_rgba(width: u32, height: u32, pixels: &[u8], format: PixelFormat) -> Option<Rgba8Image> {
    let data: Vec<u8> = match format {
        PixelFormat::RGB24 => pixels
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|&[r, g, b]| [r, g, b, u8::MAX])
            .collect(),
        PixelFormat::L8 => pixels.iter().flat_map(|&v| [v, v, v, u8::MAX]).collect(),
        // 16-bit grey (lossless JPEG), big-endian: keep the high byte.
        PixelFormat::L16 => pixels
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|&[high, _]| [high, high, high, u8::MAX])
            .collect(),
        PixelFormat::CMYK32 => return None,
    };
    Rgba8Image::new(width, height, data)
}

/// The picture embedded in a JPEG's EXIF block, ready to show while the real decode runs.
/// `source_size` is the full picture's size. `None` if there is no thumbnail or its shape
/// doesn't match the photo (some cameras pad thumbnails with black bars).
#[must_use]
pub fn embedded_thumbnail(bytes: &[u8]) -> Option<DecodedImage> {
    let thumb = exif::jpeg_thumbnail_bytes(bytes)?;
    let orientation = exif::jpeg_orientation(bytes);
    let mut header = jpeg_decoder::Decoder::new(bytes);
    header.read_info().ok()?;
    let info = header.info()?;
    let mut decoder = jpeg_decoder::Decoder::new(thumb.as_slice());
    let pixels = decoder.decode().ok()?;
    let thumb_info = decoder.info()?;
    let image = to_rgba(
        u32::from(thumb_info.width),
        u32::from(thumb_info.height),
        &pixels,
        thumb_info.pixel_format,
    )?;
    let same_shape = {
        let photo = f64::from(info.width) / f64::from(info.height.max(1));
        let small = f64::from(thumb_info.width) / f64::from(thumb_info.height.max(1));
        (photo - small).abs() < 0.02 * photo
    };
    if !same_shape {
        return None;
    }
    let image = orient(image, orientation)?;
    let (w, h) = (u32::from(info.width), u32::from(info.height));
    let source = if exif::swaps_axes(orientation) {
        Size::new(h, w)
    } else {
        Size::new(w, h)
    };
    Some(DecodedImage::still(image, source, Format::Jpeg, false))
}
