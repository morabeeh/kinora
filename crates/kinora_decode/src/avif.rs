//! AVIF through `avif-decode` → `rav1d`, a memory-safe Rust port of dav1d (ADR 0005).

use kinora_core::viewport::Size;

use crate::error::DecodeError;
use crate::format::Format;
use crate::image_buf::{DecodedImage, Rgba8Image};
use crate::{DecodeOptions, check_size, finish_still, isobmff, transform};

pub(crate) fn decode(bytes: &[u8], options: &DecodeOptions) -> Result<DecodedImage, DecodeError> {
    if !isobmff::boxes_complete(bytes) {
        return Err(DecodeError::corrupt(Format::Avif, "the file is incomplete"));
    }
    // Reject decompression bombs before the AV1 decoder allocates anything.
    if let Some((width, height)) = isobmff::largest_ispe(bytes) {
        check_size(width, height, options)?;
    }
    let corrupt = |err: avif_decode::Error| DecodeError::corrupt(Format::Avif, err);
    let image = avif_decode::Decoder::from_avif(bytes)
        .map_err(corrupt)?
        .to_image()
        .map_err(corrupt)?;
    let (width, height, data, has_alpha) = match image {
        avif_decode::Image::Rgba8(img) => {
            let px = img.pixels().flat_map(|p| [p.r, p.g, p.b, p.a]).collect();
            (img.width(), img.height(), px, true)
        }
        avif_decode::Image::Rgb8(img) => {
            let px = img
                .pixels()
                .flat_map(|p| [p.r, p.g, p.b, u8::MAX])
                .collect();
            (img.width(), img.height(), px, false)
        }
        avif_decode::Image::Rgba16(img) => {
            let px = img
                .pixels()
                .flat_map(|p| [p.r, p.g, p.b, p.a].map(high_byte))
                .collect();
            (img.width(), img.height(), px, true)
        }
        avif_decode::Image::Rgb16(img) => {
            let px = img
                .pixels()
                .flat_map(|p| [high_byte(p.r), high_byte(p.g), high_byte(p.b), u8::MAX])
                .collect();
            (img.width(), img.height(), px, false)
        }
        avif_decode::Image::Gray8(img) => {
            let px = img
                .pixels()
                .flat_map(|p| {
                    let v = p.value();
                    [v, v, v, u8::MAX]
                })
                .collect();
            (img.width(), img.height(), px, false)
        }
        avif_decode::Image::Gray16(img) => {
            let px = img
                .pixels()
                .flat_map(|p| {
                    let v = high_byte(p.value());
                    [v, v, v, u8::MAX]
                })
                .collect();
            (img.width(), img.height(), px, false)
        }
    };
    let width = u32::try_from(width).map_err(|_| DecodeError::corrupt(Format::Avif, "width"))?;
    let height = u32::try_from(height).map_err(|_| DecodeError::corrupt(Format::Avif, "height"))?;
    let rgba = Rgba8Image::new(width, height, data)
        .ok_or_else(|| DecodeError::corrupt(Format::Avif, "pixel count"))?;
    let (quarter_turns, mirror) = isobmff::transforms(bytes);
    let rgba = transform(rgba, quarter_turns, mirror)
        .ok_or_else(|| DecodeError::corrupt(Format::Avif, "transform"))?;
    let size: Size = rgba.size();
    Ok(finish_still(
        DecodedImage::still(rgba, size, Format::Avif, has_alpha),
        options,
    ))
}

/// 16-bit sample → 8-bit (the samples are already scaled to the full 16-bit range).
fn high_byte(value: u16) -> u8 {
    value.to_be_bytes()[0]
}
