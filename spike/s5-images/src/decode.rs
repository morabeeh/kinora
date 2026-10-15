//! Decode any supported image file to RGBA8 by sniffing its *content* (not its extension).
//! Every decoder call is wrapped so that a panic inside a decoder becomes an error.

use std::io::Cursor;
use std::panic::{AssertUnwindSafe, catch_unwind};

use anyhow::Context as _;

/// Reject images whose RGBA buffer would exceed this (brief §11 default ≈ 1.5 GB).
pub const MAX_RGBA_BYTES: u64 = 1_500_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Jpeg,
    Png,
    Gif,
    WebP,
    Avif,
    Heif,
    Jxl,
    Other,
}

#[derive(Debug)]
pub struct Decoded {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub format: Format,
    pub frames: usize,
}

pub fn sniff(bytes: &[u8]) -> Format {
    let at = |range: std::ops::Range<usize>| bytes.get(range).unwrap_or_default();
    match bytes {
        [0xFF, 0xD8, 0xFF, ..] => Format::Jpeg,
        [0x89, b'P', b'N', b'G', ..] => Format::Png,
        [b'G', b'I', b'F', b'8', ..] => Format::Gif,
        [0xFF, 0x0A, ..] | [0, 0, 0, 0x0C, b'J', b'X', b'L', b' ', ..] => Format::Jxl,
        _ if at(0..4) == b"RIFF" && at(8..12) == b"WEBP" => Format::WebP,
        _ if at(4..8) == b"ftyp" => match at(8..12) {
            b"avif" | b"avis" => Format::Avif,
            _ => Format::Heif,
        },
        _ => Format::Other,
    }
}

fn check_size(width: u64, height: u64) -> anyhow::Result<()> {
    let bytes = width.saturating_mul(height).saturating_mul(4);
    anyhow::ensure!(
        bytes <= MAX_RGBA_BYTES,
        "image is too large ({width}×{height}, would need {} MB)",
        bytes / 1_000_000
    );
    Ok(())
}

/// Decode with panics turned into errors (one bad decoder must not kill the app).
pub fn decode(bytes: &[u8]) -> anyhow::Result<Decoded> {
    match catch_unwind(AssertUnwindSafe(|| decode_inner(bytes))) {
        Ok(result) => result,
        Err(_) => anyhow::bail!("decoder panicked"),
    }
}

fn decode_inner(bytes: &[u8]) -> anyhow::Result<Decoded> {
    anyhow::ensure!(!bytes.is_empty(), "the file is empty");
    let format = sniff(bytes);
    let mut decoded = match format {
        Format::Avif => decode_avif(bytes)?,
        Format::Heif => decode_heif(bytes)?,
        Format::Jxl => decode_jxl(bytes)?,
        _ => decode_with_image(bytes)?,
    };
    decoded.format = format;
    if matches!(format, Format::Gif | Format::WebP | Format::Png) {
        decoded.frames = count_frames(bytes, format);
    }
    Ok(decoded)
}

/// JPEG/PNG/GIF/WebP/TIFF/BMP/…: the `image` crate, with EXIF orientation applied.
fn decode_with_image(bytes: &[u8]) -> anyhow::Result<Decoded> {
    let mut reader = image::ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(MAX_RGBA_BYTES);
    reader.limits(limits);
    let mut decoder = reader.into_decoder()?;
    let (w, h) = image::ImageDecoder::dimensions(&decoder);
    check_size(u64::from(w), u64::from(h))?;
    let orientation = image::ImageDecoder::orientation(&mut decoder)?;
    let mut img = image::DynamicImage::from_decoder(decoder)?;
    img.apply_orientation(orientation);
    let rgba = img.into_rgba8();
    Ok(Decoded {
        width: rgba.width(),
        height: rgba.height(),
        rgba: rgba.into_raw(),
        format: Format::Other,
        frames: 1,
    })
}

/// Hard cap on frames we will walk through in one animation.
const MAX_FRAMES: usize = 10_000;

/// Counts animation frames. On truncated input `image`'s frame iterators yield the same
/// error *forever* instead of ending, so stop at the first error and cap the count.
fn count_frames(bytes: &[u8], format: Format) -> usize {
    use image::AnimationDecoder as _;
    let cursor = Cursor::new(bytes);
    let frames = match format {
        Format::Gif => image::codecs::gif::GifDecoder::new(cursor).map(|d| d.into_frames().take(MAX_FRAMES).take_while(Result::is_ok).count()),
        Format::WebP => {
            image::codecs::webp::WebPDecoder::new(cursor).map(|d| d.into_frames().take(MAX_FRAMES).take_while(Result::is_ok).count())
        }
        Format::Png => image::codecs::png::PngDecoder::new(cursor).and_then(|d| {
            if d.is_apng()? {
                d.apng().map(|a| a.into_frames().take(MAX_FRAMES).take_while(Result::is_ok).count())
            } else {
                Ok(1)
            }
        }),
        _ => Ok(1),
    };
    frames.unwrap_or(1)
}

/// HEIC/HEIF through libheif (dynamically linked); libheif applies rotation/mirroring.
fn decode_heif(bytes: &[u8]) -> anyhow::Result<Decoded> {
    use libheif_rs::{ColorSpace, HeifContext, LibHeif, RgbChroma};
    let lib = LibHeif::new();
    let ctx = HeifContext::read_from_bytes(bytes)?;
    let handle = ctx.primary_image_handle()?;
    check_size(u64::from(handle.width()), u64::from(handle.height()))?;
    let image = lib.decode(&handle, ColorSpace::Rgb(RgbChroma::Rgba), None)?;
    let planes = image.planes();
    let plane = planes.interleaved.context("no interleaved plane")?;
    let row_bytes = usize::try_from(plane.width)? * 4;
    let mut rgba = Vec::with_capacity(row_bytes * usize::try_from(plane.height)?);
    for row in plane
        .data
        .chunks(plane.stride)
        .take(usize::try_from(plane.height)?)
    {
        rgba.extend_from_slice(row.get(..row_bytes).context("short row")?);
    }
    Ok(Decoded {
        width: plane.width,
        height: plane.height,
        rgba,
        format: Format::Heif,
        frames: 1,
    })
}

/// AVIF through avif-decode + rav1d (memory-safe Rust port of dav1d).
fn decode_avif(bytes: &[u8]) -> anyhow::Result<Decoded> {
    use avif_decode::Image;
    let image = avif_decode::Decoder::from_avif(bytes)?.to_image()?;
    let (width, height, rgba) = match image {
        Image::Rgba8(img) => {
            let px: Vec<u8> = img.pixels().flat_map(|p| [p.r, p.g, p.b, p.a]).collect();
            (img.width(), img.height(), px)
        }
        Image::Rgb8(img) => {
            let px: Vec<u8> = img
                .pixels()
                .flat_map(|p| [p.r, p.g, p.b, u8::MAX])
                .collect();
            (img.width(), img.height(), px)
        }
        Image::Rgba16(img) => {
            let px: Vec<u8> = img
                .pixels()
                .flat_map(|p| [p.r, p.g, p.b, p.a].map(high_byte))
                .collect();
            (img.width(), img.height(), px)
        }
        Image::Rgb16(img) => {
            let px: Vec<u8> = img
                .pixels()
                .flat_map(|p| [high_byte(p.r), high_byte(p.g), high_byte(p.b), u8::MAX])
                .collect();
            (img.width(), img.height(), px)
        }
        Image::Gray8(img) => {
            let px: Vec<u8> = img
                .pixels()
                .flat_map(|p| [p.value(), p.value(), p.value(), u8::MAX])
                .collect();
            (img.width(), img.height(), px)
        }
        Image::Gray16(img) => {
            let px: Vec<u8> = img
                .pixels()
                .flat_map(|p| {
                    let v = high_byte(p.value());
                    [v, v, v, u8::MAX]
                })
                .collect();
            (img.width(), img.height(), px)
        }
    };
    Ok(Decoded {
        width: u32::try_from(width)?,
        height: u32::try_from(height)?,
        rgba,
        format: Format::Avif,
        frames: 1,
    })
}

fn high_byte(value: u16) -> u8 {
    u8::try_from(value >> 8).unwrap_or(u8::MAX)
}

/// JPEG XL through jxl-oxide (pure Rust); orientation is applied by the renderer.
fn decode_jxl(bytes: &[u8]) -> anyhow::Result<Decoded> {
    let image = jxl_oxide::JxlImage::builder()
        .read(Cursor::new(bytes))
        .map_err(|err| anyhow::anyhow!(err))?;
    check_size(u64::from(image.width()), u64::from(image.height()))?;
    let render = image.render_frame(0).map_err(|err| anyhow::anyhow!(err))?;
    let mut stream = render.stream();
    let (width, height, channels) = (stream.width(), stream.height(), stream.channels());
    let pixels = usize::try_from(width)? * usize::try_from(height)?;
    let mut samples = vec![0u8; pixels * usize::try_from(channels)?];
    stream.write_to_buffer(&mut samples);
    let rgba: Vec<u8> = match channels {
        1 => samples.iter().flat_map(|&v| [v, v, v, u8::MAX]).collect(),
        2 => samples
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|&[v, a]| [v, v, v, a])
            .collect(),
        3 => samples
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|&[r, g, b]| [r, g, b, u8::MAX])
            .collect(),
        4 => samples,
        other => anyhow::bail!("unsupported JPEG XL channel count {other}"),
    };
    Ok(Decoded {
        width,
        height,
        rgba,
        format: Format::Jxl,
        frames: image.num_loaded_keyframes(),
    })
}

/// Scaled JPEG decoding (DCT-domain 1/2, 1/4, 1/8) through jpeg-decoder.
/// Returns the decoded size; used to measure "fast first pixel".
pub fn decode_jpeg_scaled(
    bytes: &[u8],
    max_width: u16,
    max_height: u16,
) -> anyhow::Result<(u16, u16, usize)> {
    let mut decoder = jpeg_decoder::Decoder::new(Cursor::new(bytes));
    decoder.read_info()?;
    let (w, h) = decoder.scale(max_width, max_height)?;
    let pixels = decoder.decode()?;
    Ok((w, h, pixels.len()))
}
