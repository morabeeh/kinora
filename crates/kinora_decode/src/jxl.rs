//! JPEG XL through `jxl-oxide` (pure Rust).

use std::io::Cursor;

use jxl_oxide::{AllocTracker, EnumColourEncoding, JxlImage, RenderingIntent};

use crate::error::DecodeError;
use crate::format::Format;
use crate::image_buf::{DecodedImage, Rgba8Image};
use crate::{DecodeOptions, check_size, finish_still};

pub(crate) fn decode(bytes: &[u8], options: &DecodeOptions) -> Result<DecodedImage, DecodeError> {
    let corrupt =
        |err: Box<dyn std::error::Error + Send + Sync>| DecodeError::corrupt(Format::Jxl, err);
    // jxl-oxide works in f32 internally; allow a few times the output size, which also
    // stops decompression bombs inside the decoder.
    let budget = usize::try_from(options.max_rgba_bytes.saturating_mul(4)).unwrap_or(usize::MAX);
    let mut image = JxlImage::builder()
        .alloc_tracker(AllocTracker::with_limit(budget))
        .read(Cursor::new(bytes))
        .map_err(corrupt)?;
    check_size(image.width(), image.height(), options)?;
    // Wide-gamut and HDR files are converted to sRGB by the decoder itself.
    image.request_color_encoding(EnumColourEncoding::srgb(RenderingIntent::Relative));
    let render = image.render_frame(0).map_err(corrupt)?;
    let mut stream = render.stream();
    let (width, height, channels) = (stream.width(), stream.height(), stream.channels());
    let samples_len = usize::try_from(width)
        .ok()
        .zip(usize::try_from(height).ok())
        .zip(usize::try_from(channels).ok())
        .and_then(|((w, h), c)| w.checked_mul(h)?.checked_mul(c))
        .ok_or_else(|| DecodeError::corrupt(Format::Jxl, "size"))?;
    let mut samples = vec![0u8; samples_len];
    stream.write_to_buffer(&mut samples);
    let (rgba, has_alpha): (Vec<u8>, bool) = match channels {
        1 => (
            samples.iter().flat_map(|&v| [v, v, v, u8::MAX]).collect(),
            false,
        ),
        2 => (
            samples
                .as_chunks::<2>()
                .0
                .iter()
                .flat_map(|&[v, a]| [v, v, v, a])
                .collect(),
            true,
        ),
        3 => (
            samples
                .as_chunks::<3>()
                .0
                .iter()
                .flat_map(|&[r, g, b]| [r, g, b, u8::MAX])
                .collect(),
            false,
        ),
        4 => (samples, true),
        other => {
            return Err(DecodeError::corrupt(
                Format::Jxl,
                format!("{other} channels"),
            ));
        }
    };
    let rgba = Rgba8Image::new(width, height, rgba)
        .ok_or_else(|| DecodeError::corrupt(Format::Jxl, "pixel count"))?;
    let size = rgba.size();
    Ok(finish_still(
        DecodedImage::still(rgba, size, Format::Jxl, has_alpha),
        options,
    ))
}
