//! Image decoding for Kinora (ADR 0005).
//!
//! Every file goes through the same safe pipeline:
//! **content sniffing → extension fallback → size check before allocating → decoder inside
//! `catch_unwind` → typed error**. A damaged or hostile file therefore produces a
//! [`DecodeError`] (shown as a friendly error card), never a crash.
//!
//! The result is always 8-bit RGBA in sRGB with EXIF orientation applied, so the viewer
//! never has to care which format a picture came from. [`viewport`] then crops and scales
//! it to the window (the CPU viewport, ADR 0004).
//!
//! ```no_run
//! use kinora_decode::{decode_file, DecodeOptions};
//! let picture = decode_file(std::path::Path::new("photo.heic"), &DecodeOptions::default())?;
//! println!("{} × {}", picture.source_size.width, picture.source_size.height);
//! # Ok::<(), kinora_decode::DecodeError>(())
//! ```

mod avif;
mod color;
pub mod error;
pub mod exif;
pub mod format;
#[cfg(feature = "heif")]
mod heif;
pub mod image_buf;
mod isobmff;
mod jpeg;
mod jxl;
mod raster;
pub mod resize;
mod svg;
mod tonemap;
pub mod viewport;

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

use image::metadata::Orientation;
use kinora_core::hardware::MAX_IMAGE_BYTES_LIMIT;
use kinora_core::viewport::Size;

pub use error::DecodeError;
pub use format::Format;
#[cfg(feature = "heif")]
pub use heif::set_availability_check as set_heif_availability_check;
pub use image_buf::{DecodedImage, Frame, Rgba8Image};
pub use jpeg::embedded_thumbnail;

/// What size of picture to produce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// The full-resolution picture (for zooming in).
    Full,
    /// Enough pixels to fill a `width` × `height` (physical pixels) window at "fit".
    /// JPEGs use fast DCT-scaled decoding; SVGs render at that size; other formats decode
    /// fully and are shrunk only if the full picture is bigger than `keep_full_below_bytes`
    /// (so small and medium pictures can still be zoomed without decoding again).
    Screen {
        /// Window width in physical pixels.
        width: u32,
        /// Window height in physical pixels.
        height: u32,
        /// Keep full resolution when its RGBA size is at most this many bytes.
        keep_full_below_bytes: u64,
    },
}

/// Limits and target for one decode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodeOptions {
    /// Size of picture to produce.
    pub target: Target,
    /// Largest RGBA buffer one picture may need (brief §11 default ≈ 1.5 GB; the Lite
    /// budget is much smaller). Bigger pictures fail with [`DecodeError::TooLarge`].
    pub max_rgba_bytes: u64,
    /// Most frames to decode from an animation.
    pub max_frames: usize,
    /// Most memory all frames of one animation may use.
    pub max_animation_bytes: u64,
}

impl Default for DecodeOptions {
    fn default() -> Self {
        Self {
            target: Target::Full,
            max_rgba_bytes: MAX_IMAGE_BYTES_LIMIT,
            max_frames: 5_000,
            max_animation_bytes: 512 * 1024 * 1024,
        }
    }
}

/// Files bigger than this are not read at all (no photo is anywhere near it).
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// Reads and decodes a file. The extension is only used as a fallback (TGA, SVGZ).
///
/// # Errors
/// See [`DecodeError`].
pub fn decode_file(path: &Path, options: &DecodeOptions) -> Result<DecodedImage, DecodeError> {
    let bytes = read_file(path)?;
    let extension = path.extension().and_then(|ext| ext.to_str());
    decode(&bytes, extension, options)
}

/// Reads a whole file into memory, refusing absurdly large files.
///
/// # Errors
/// [`DecodeError::Io`] if reading fails, [`DecodeError::TooLarge`] above 2 GiB.
pub fn read_file(path: &Path) -> Result<Vec<u8>, DecodeError> {
    let length = std::fs::metadata(path)?.len();
    if length > MAX_FILE_BYTES {
        return Err(DecodeError::TooLarge {
            width: 0,
            height: 0,
        });
    }
    Ok(std::fs::read(path)?)
}

/// Decodes a file already in memory.
///
/// # Errors
/// See [`DecodeError`]; a panicking decoder becomes [`DecodeError::Panicked`].
pub fn decode(
    bytes: &[u8],
    extension: Option<&str>,
    options: &DecodeOptions,
) -> Result<DecodedImage, DecodeError> {
    if bytes.is_empty() {
        return Err(DecodeError::Empty);
    }
    let format = format::detect(bytes, extension).ok_or(DecodeError::UnknownFormat)?;
    // One bad decoder must not take the app down (brief §8.3). The default panic hook still
    // logs the panic; the app routes it to the log file.
    catch_unwind(AssertUnwindSafe(|| decode_format(bytes, format, options)))
        .unwrap_or(Err(DecodeError::Panicked(format)))
}

fn decode_format(
    bytes: &[u8],
    format: Format,
    options: &DecodeOptions,
) -> Result<DecodedImage, DecodeError> {
    match format {
        Format::Jpeg => jpeg::decode(bytes, options),
        Format::Avif => avif::decode(bytes, options),
        Format::Jxl => jxl::decode(bytes, options),
        Format::Svg => svg::decode(bytes, options),
        #[cfg(feature = "heif")]
        Format::Heif => heif::decode(bytes, options),
        #[cfg(not(feature = "heif"))]
        Format::Heif => Err(DecodeError::Unavailable(Format::Heif)),
        _ => raster::decode(bytes, format, options),
    }
}

/// Rejects pictures whose RGBA buffer would exceed the limit — checked from the header,
/// before anything big is allocated (decompression bombs, brief §11).
pub(crate) fn check_size(
    width: u32,
    height: u32,
    options: &DecodeOptions,
) -> Result<(), DecodeError> {
    let bytes = u64::from(width)
        .saturating_mul(u64::from(height))
        .saturating_mul(4);
    if width == 0 || height == 0 || bytes > options.max_rgba_bytes {
        return Err(DecodeError::TooLarge {
            width: u64::from(width),
            height: u64::from(height),
        });
    }
    Ok(())
}

/// Applies the screen-size policy of [`Target::Screen`] to a decoded still picture.
pub(crate) fn finish_still(image: DecodedImage, options: &DecodeOptions) -> DecodedImage {
    let Target::Screen {
        width,
        height,
        keep_full_below_bytes,
    } = options.target
    else {
        return image;
    };
    if image.is_animated() || image.first().byte_size() <= keep_full_below_bytes {
        return image;
    }
    let has_alpha = image.has_alpha;
    let mut resizer = fast_image_resize::Resizer::new();
    image.map_first(|pixels| {
        resize::shrink_to_fit(&mut resizer, pixels, Size::new(width, height), has_alpha)
    })
}

/// Applies an EXIF orientation to the pixels. `None` only if the buffer is inconsistent
/// (cannot happen with a valid [`Rgba8Image`]).
pub(crate) fn orient(image: Rgba8Image, orientation: Orientation) -> Option<Rgba8Image> {
    if orientation == Orientation::NoTransforms {
        return Some(image);
    }
    let (width, height) = (image.width(), image.height());
    let buffer = image::RgbaImage::from_raw(width, height, image.into_data())?;
    let mut dynamic = image::DynamicImage::ImageRgba8(buffer);
    dynamic.apply_orientation(orientation);
    let rgba = dynamic.into_rgba8();
    let (w, h) = rgba.dimensions();
    Rgba8Image::new(w, h, rgba.into_raw())
}

/// HEIF-style transforms: `quarter_turns` anti-clockwise, then an optional mirror
/// (`0` = left↔right, `1` = top↔bottom).
pub(crate) fn transform(
    image: Rgba8Image,
    quarter_turns: u8,
    mirror: Option<u8>,
) -> Option<Rgba8Image> {
    let rotated = match quarter_turns % 4 {
        1 => orient(image, Orientation::Rotate270)?,
        2 => orient(image, Orientation::Rotate180)?,
        3 => orient(image, Orientation::Rotate90)?,
        _ => image,
    };
    match mirror {
        Some(0) => orient(rotated, Orientation::FlipHorizontal),
        Some(_) => orient(rotated, Orientation::FlipVertical),
        None => Some(rotated),
    }
}
