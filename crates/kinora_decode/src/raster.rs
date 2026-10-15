//! Formats decoded by the `image` crate: JPEG (full size), PNG/APNG, GIF, WebP, TIFF, BMP,
//! ICO, TGA, PNM, QOI, DDS, HDR and EXR.

use std::io::Cursor;
use std::time::Duration;

use image::{AnimationDecoder, DynamicImage, ImageDecoder, ImageFormat, ImageReader};

use crate::error::DecodeError;
use crate::format::Format;
use crate::image_buf::{DecodedImage, Frame, Rgba8Image};
use crate::{DecodeOptions, check_size, color, finish_still, orient, tonemap};

/// Decodes a still or animated image with the `image` crate.
pub(crate) fn decode(
    bytes: &[u8],
    format: Format,
    options: &DecodeOptions,
) -> Result<DecodedImage, DecodeError> {
    match format {
        Format::Gif => {
            let decoder = image::codecs::gif::GifDecoder::new(Cursor::new(bytes))
                .map_err(|err| map_error(format, err))?;
            let header = header_of(&decoder);
            animation(decoder, header, format, options)
        }
        Format::WebP => {
            let decoder = image::codecs::webp::WebPDecoder::new(Cursor::new(bytes))
                .map_err(|err| map_error(format, err))?;
            if decoder.has_animation() {
                let header = header_of(&decoder);
                animation(decoder, header, format, options)
            } else {
                still(bytes, format, options)
            }
        }
        Format::Png => {
            let decoder = image::codecs::png::PngDecoder::new(Cursor::new(bytes))
                .map_err(|err| map_error(format, err))?;
            if decoder.is_apng().map_err(|err| map_error(format, err))? {
                let header = header_of(&decoder);
                let apng = decoder.apng().map_err(|err| map_error(format, err))?;
                animation(apng, header, format, options)
            } else {
                still(bytes, format, options)
            }
        }
        _ => still(bytes, format, options),
    }
}

/// A still image: decode, apply EXIF orientation and the ICC profile, tone-map HDR.
pub(crate) fn still(
    bytes: &[u8],
    format: Format,
    options: &DecodeOptions,
) -> Result<DecodedImage, DecodeError> {
    let image_format = image_format(format).ok_or(DecodeError::UnknownFormat)?;
    let mut reader = ImageReader::with_format(Cursor::new(bytes), image_format);
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(options.max_rgba_bytes.saturating_mul(2));
    reader.limits(limits);
    let mut decoder = reader
        .into_decoder()
        .map_err(|err| map_error(format, err))?;
    let (width, height) = decoder.dimensions();
    check_size(width, height, options)?;
    let orientation = decoder
        .orientation()
        .unwrap_or(image::metadata::Orientation::NoTransforms);
    let icc = decoder.icc_profile().ok().flatten();
    let has_alpha = decoder.color_type().has_alpha();
    let image = DynamicImage::from_decoder(decoder).map_err(|err| map_error(format, err))?;
    let rgba = match image {
        DynamicImage::ImageRgb32F(hdr) => {
            tonemap::tonemap(hdr.width(), hdr.height(), hdr.as_raw(), 3)
                .ok_or_else(|| DecodeError::corrupt(format, "HDR sample count"))?
        }
        DynamicImage::ImageRgba32F(hdr) => {
            tonemap::tonemap(hdr.width(), hdr.height(), hdr.as_raw(), 4)
                .ok_or_else(|| DecodeError::corrupt(format, "HDR sample count"))?
        }
        other => {
            let rgba = other.into_rgba8();
            let (w, h) = rgba.dimensions();
            let rgba = Rgba8Image::new(w, h, rgba.into_raw())
                .ok_or_else(|| DecodeError::corrupt(format, "pixel count"))?;
            match &icc {
                Some(profile) => color::to_srgb(rgba, profile),
                None => rgba,
            }
        }
    };
    let rgba =
        orient(rgba, orientation).ok_or_else(|| DecodeError::corrupt(format, "orientation"))?;
    let size = rgba.size();
    Ok(finish_still(
        DecodedImage::still(rgba, size, format, has_alpha),
        options,
    ))
}

/// Shortest frame delay we honour; browsers treat smaller GIF delays as "as fast as
/// possible" and show them at 100 ms, so do we.
const MIN_DELAY: Duration = Duration::from_millis(20);
const DEFAULT_DELAY: Duration = Duration::from_millis(100);

/// Decodes animation frames. `image`'s frame iterators **never end on damaged files** (they
/// repeat the same error forever — ADR 0005), so we stop at the first error and also cap
/// the number of frames and the memory they use.
fn animation<'a>(
    decoder: impl AnimationDecoder<'a>,
    header: Header,
    format: Format,
    options: &DecodeOptions,
) -> Result<DecodedImage, DecodeError> {
    check_size(header.width, header.height, options)?;
    let mut frames: Vec<Frame> = Vec::new();
    let mut total_bytes = 0u64;
    let mut truncated = false;
    for result in decoder.into_frames() {
        let frame = match result {
            Ok(frame) => frame,
            Err(err) if frames.is_empty() => return Err(map_error(format, err)),
            Err(err) => {
                tracing::debug!(%err, frames = frames.len(), "animation stops at a damaged frame");
                truncated = true;
                break;
            }
        };
        let (numerator, denominator) = frame.delay().numer_denom_ms();
        let millis = u64::from(numerator) / u64::from(denominator.max(1));
        let delay = Duration::from_millis(millis);
        let delay = if delay < MIN_DELAY {
            DEFAULT_DELAY
        } else {
            delay
        };
        let buffer = frame.into_buffer();
        let (w, h) = buffer.dimensions();
        let Some(image) = Rgba8Image::new(w, h, buffer.into_raw()) else {
            truncated = true;
            break;
        };
        total_bytes = total_bytes.saturating_add(image.byte_size());
        if !frames.is_empty()
            && (total_bytes > options.max_animation_bytes || frames.len() >= options.max_frames)
        {
            truncated = true;
            break;
        }
        frames.push(Frame { image, delay });
    }
    let mut picture = DecodedImage::animated(frames, format, header.has_alpha)
        .ok_or_else(|| DecodeError::corrupt(format, "no frames"))?;
    picture.animation_truncated = truncated;
    if !picture.is_animated() {
        picture = finish_still(picture, options);
    }
    Ok(picture)
}

/// Size and transparency, read before the frames are decoded.
#[derive(Debug, Clone, Copy)]
struct Header {
    width: u32,
    height: u32,
    has_alpha: bool,
}

fn header_of(decoder: &impl ImageDecoder) -> Header {
    let (width, height) = decoder.dimensions();
    Header {
        width,
        height,
        has_alpha: decoder.color_type().has_alpha(),
    }
}

fn image_format(format: Format) -> Option<ImageFormat> {
    Some(match format {
        Format::Jpeg => ImageFormat::Jpeg,
        Format::Png => ImageFormat::Png,
        Format::Gif => ImageFormat::Gif,
        Format::WebP => ImageFormat::WebP,
        Format::Bmp => ImageFormat::Bmp,
        Format::Ico => ImageFormat::Ico,
        Format::Tiff => ImageFormat::Tiff,
        Format::Tga => ImageFormat::Tga,
        Format::Pnm => ImageFormat::Pnm,
        Format::Qoi => ImageFormat::Qoi,
        Format::Dds => ImageFormat::Dds,
        Format::Hdr => ImageFormat::Hdr,
        Format::Exr => ImageFormat::OpenExr,
        Format::Heif | Format::Avif | Format::Jxl | Format::Svg => return None,
    })
}

fn map_error(format: Format, err: image::ImageError) -> DecodeError {
    match err {
        image::ImageError::Limits(_) => DecodeError::TooLarge {
            width: 0,
            height: 0,
        },
        image::ImageError::IoError(io) => DecodeError::corrupt(format, io),
        other => DecodeError::corrupt(format, other),
    }
}

/// Re-export for `jpeg.rs`, which falls back to the full decoder.
pub(crate) use still as decode_still;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_image_crate_format_has_a_mapping() {
        for format in [
            Format::Jpeg,
            Format::Png,
            Format::Gif,
            Format::WebP,
            Format::Bmp,
            Format::Ico,
            Format::Tiff,
            Format::Tga,
            Format::Pnm,
            Format::Qoi,
            Format::Dds,
            Format::Hdr,
            Format::Exr,
        ] {
            assert!(image_format(format).is_some(), "{format}");
        }
        assert!(image_format(Format::Heif).is_none());
    }
}
