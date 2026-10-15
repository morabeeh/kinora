//! Writes the decode-test fixtures into `test_data/images/`.
//!
//! Every picture is synthetic (no licensing questions; CC0): a gradient with a red square
//! in the top-left, green in the top-right and blue in the bottom-left corner, so tests can
//! check orientation and colours automatically. Formats the `image` crate can't write come
//! from FFmpeg (`KINORA_FFMPEG` = path to ffmpeg.exe) and libheif's x265 encoder.
//!
//! Run from the repository root (Windows, with vcpkg's bin folder on PATH):
//!   cargo run --manifest-path test_data/generator/Cargo.toml

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context as _, ensure};
use image::{ImageEncoder as _, Rgb, RgbImage, Rgba, RgbaImage};

const W: u32 = 64;
const H: u32 = 48;

fn main() -> anyhow::Result<()> {
    let dir = PathBuf::from("test_data/images");
    std::fs::create_dir_all(&dir)?;
    let canon = canonical(W, H);

    // EXIF orientations 1–8: same pixels, different tag.
    let jpeg = encode_jpeg(&canon, 92)?;
    for orientation in 1..=8u16 {
        std::fs::write(
            dir.join(format!("orient-{orientation}.jpg")),
            with_exif(&jpeg, orientation, None)?,
        )?;
    }

    // 640×480 photo with an embedded 160×120 EXIF thumbnail, upright and rotated (6).
    let photo = canonical(640, 480);
    let thumb = encode_jpeg(&image::imageops::thumbnail(&photo, 160, 120), 80)?;
    let photo_jpeg = encode_jpeg(&photo, 85)?;
    std::fs::write(dir.join("photo.jpg"), with_exif(&photo_jpeg, 1, Some(&thumb))?)?;
    std::fs::write(
        dir.join("photo-rotated.jpg"),
        with_exif(&photo_jpeg, 6, Some(&thumb))?,
    )?;

    write_cmyk_jpeg(&canon, &dir.join("cmyk.jpg"))?;
    image::DynamicImage::ImageRgb8(canon.clone())
        .into_rgb16()
        .save(dir.join("rgb16.png"))?;
    transparent(W, H).save(dir.join("rgba.png"))?;
    for ext in ["bmp", "tif", "tga", "qoi", "ppm", "webp"] {
        let name = if ext == "webp" { "still.webp".to_owned() } else { format!("still.{ext}") };
        canon.save(dir.join(name))?;
    }
    image::imageops::thumbnail(&transparent(W, H), 32, 32).save(dir.join("still.ico"))?;
    std::fs::write(dir.join("still.dds"), dxt1_dds())?;
    hdr_image()?.save(dir.join("still.hdr"))?;
    image::DynamicImage::ImageRgba32F(hdr_rgba()).save(dir.join("still.exr"))?;
    canon.save(dir.join("base.png"))?;
    write_animated_gif(&dir.join("anim.gif"))?;
    encode_heic(&canonical(128, 96), &dir.join("still.heic"))?;
    canonical(128, 96).save(dir.join("base-128.png"))?;
    ffmpeg(&dir, &["-i", "base-128.png", "-c:v", "libaom-av1", "-still-picture", "1",
        "-crf", "30", "-cpu-used", "6", "still.avif"])?;
    ffmpeg(&dir, &["-i", "base.png", "-c:v", "libjxl", "-distance", "1", "still.jxl"])?;
    ffmpeg(&dir, &["-i", "base.png", "-c:v", "libwebp", "-quality", "80", "lossy.webp"])?;
    ffmpeg(&dir, &["-i", "anim.gif", "-c:v", "libwebp_anim", "-lossless", "1", "-loop", "0",
        "anim.webp"])?;
    ffmpeg(&dir, &["-i", "anim.gif", "-plays", "0", "-f", "apng", "anim-apng.png"])?;
    for helper in ["base.png", "base-128.png"] {
        std::fs::remove_file(dir.join(helper))?;
    }
    std::fs::write(dir.join("vector.svg"), SVG)?;
    std::fs::write(dir.join("text.svg"), SVG_TEXT)?;

    write_bad_files(&dir)?;
    println!("fixtures written to {}", dir.display());
    Ok(())
}

/// Gradient + red (top-left), green (top-right) and blue (bottom-left) squares.
fn canonical(width: u32, height: u32) -> RgbImage {
    let mark_w = width / 4;
    let mark_h = height / 4;
    RgbImage::from_fn(width, height, |x, y| {
        let (left, top) = (x < mark_w, y < mark_h);
        let (right, bottom) = (x >= width - mark_w, y >= height - mark_h);
        if left && top {
            Rgb([230, 20, 20])
        } else if right && top {
            Rgb([20, 200, 40])
        } else if left && bottom {
            Rgb([30, 60, 230])
        } else {
            let r = u8::try_from(x * 120 / width + 60).unwrap_or(u8::MAX);
            let g = u8::try_from(y * 120 / height + 60).unwrap_or(u8::MAX);
            Rgb([r, g, 128])
        }
    })
}

/// Transparent background with an opaque red top-left square and a half-transparent
/// centre.
fn transparent(width: u32, height: u32) -> RgbaImage {
    RgbaImage::from_fn(width, height, |x, y| {
        if x < width / 4 && y < height / 4 {
            Rgba([230, 20, 20, 255])
        } else if (width / 3..2 * width / 3).contains(&x) && (height / 3..2 * height / 3).contains(&y) {
            Rgba([20, 20, 230, 128])
        } else {
            Rgba([0, 0, 0, 0])
        }
    })
}

fn encode_jpeg(img: &RgbImage, quality: u8) -> anyhow::Result<Vec<u8>> {
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality).write_image(
        img.as_raw(),
        img.width(),
        img.height(),
        image::ExtendedColorType::Rgb8,
    )?;
    Ok(out)
}

/// Inserts an EXIF APP1 segment after SOI: IFD0 with the orientation, and optionally IFD1
/// pointing at an embedded JPEG thumbnail.
fn with_exif(jpeg: &[u8], orientation: u16, thumbnail: Option<&[u8]>) -> anyhow::Result<Vec<u8>> {
    let mut tiff = vec![b'I', b'I', 42, 0, 8, 0, 0, 0];
    // IFD0: one entry (Orientation); next-IFD offset points at IFD1 (or 0).
    tiff.extend_from_slice(&1u16.to_le_bytes());
    tiff.extend_from_slice(&0x0112u16.to_le_bytes());
    tiff.extend_from_slice(&3u16.to_le_bytes());
    tiff.extend_from_slice(&1u32.to_le_bytes());
    tiff.extend_from_slice(&orientation.to_le_bytes());
    tiff.extend_from_slice(&[0, 0]);
    let ifd1_offset: u32 = 8 + 2 + 12 + 4;
    tiff.extend_from_slice(&(if thumbnail.is_some() { ifd1_offset } else { 0 }).to_le_bytes());
    if let Some(thumb) = thumbnail {
        let data_offset: u32 = ifd1_offset + 2 + 3 * 12 + 4;
        let entry = |tiff: &mut Vec<u8>, tag: u16, kind: u16, value: u32| {
            tiff.extend_from_slice(&tag.to_le_bytes());
            tiff.extend_from_slice(&kind.to_le_bytes());
            tiff.extend_from_slice(&1u32.to_le_bytes());
            tiff.extend_from_slice(&value.to_le_bytes());
        };
        tiff.extend_from_slice(&3u16.to_le_bytes());
        entry(&mut tiff, 0x0103, 3, 6); // Compression = JPEG
        entry(&mut tiff, 0x0201, 4, data_offset); // JPEGInterchangeFormat
        entry(&mut tiff, 0x0202, 4, u32::try_from(thumb.len())?); // …Length
        tiff.extend_from_slice(&0u32.to_le_bytes());
        ensure!(tiff.len() == usize::try_from(data_offset)?);
        tiff.extend_from_slice(thumb);
    }
    let length = u16::try_from(2 + 6 + tiff.len())?;
    let (soi, rest) = jpeg.split_at_checked(2).context("not a JPEG")?;
    let mut out = soi.to_vec();
    out.extend_from_slice(&[0xFF, 0xE1]);
    out.extend_from_slice(&length.to_be_bytes());
    out.extend_from_slice(b"Exif\0\0");
    out.extend_from_slice(&tiff);
    out.extend_from_slice(rest);
    Ok(out)
}

fn write_cmyk_jpeg(img: &RgbImage, path: &Path) -> anyhow::Result<()> {
    let cmyk: Vec<u8> = img
        .pixels()
        .flat_map(|Rgb([r, g, b])| [255 - r, 255 - g, 255 - b, 0])
        .collect();
    let encoder = jpeg_encoder::Encoder::new_file(path, 92)?;
    encoder.encode(
        &cmyk,
        u16::try_from(img.width())?,
        u16::try_from(img.height())?,
        jpeg_encoder::ColorType::Cmyk,
    )?;
    Ok(())
}

/// A 16×16 DXT1 DDS: four 8×8 quadrants in solid colours (red, green, blue, white).
fn dxt1_dds() -> Vec<u8> {
    let mut out = b"DDS ".to_vec();
    let mut header = [0u32; 31];
    header[0] = 124; // size
    header[1] = 0x1 | 0x2 | 0x4 | 0x1000 | 0x80000; // CAPS|HEIGHT|WIDTH|PIXELFORMAT|LINEARSIZE
    header[2] = 16; // height
    header[3] = 16; // width
    header[4] = 128; // linear size: 16 blocks × 8 bytes
    header[18] = 32; // pixel format size
    header[19] = 0x4; // DDPF_FOURCC
    header[20] = u32::from_le_bytes(*b"DXT1");
    header[26] = 0x1000; // DDSCAPS_TEXTURE
    for word in header {
        out.extend_from_slice(&word.to_le_bytes());
    }
    let rgb565 = |r: u16, g: u16, b: u16| ((r >> 3) << 11) | ((g >> 2) << 5) | (b >> 3);
    let colours = [
        rgb565(230, 20, 20),
        rgb565(20, 200, 40),
        rgb565(30, 60, 230),
        rgb565(255, 255, 255),
    ];
    for block_y in 0..4u8 {
        for block_x in 0..4u8 {
            let quadrant = usize::from(block_y / 2) * 2 + usize::from(block_x / 2);
            let colour = colours.get(quadrant).copied().unwrap_or(0);
            out.extend_from_slice(&colour.to_le_bytes());
            out.extend_from_slice(&colour.to_le_bytes());
            out.extend_from_slice(&[0, 0, 0, 0]); // every pixel uses colour 0
        }
    }
    out
}

fn hdr_image() -> anyhow::Result<image::DynamicImage> {
    let img = image::Rgb32FImage::from_fn(32, 24, |x, _| {
        let v = 0.01 * 2f32.powi(i32::try_from(x).unwrap_or(0) / 2);
        Rgb([v, v * 0.8, v * 0.6])
    });
    Ok(image::DynamicImage::ImageRgb32F(img))
}

fn hdr_rgba() -> image::Rgba32FImage {
    image::Rgba32FImage::from_fn(32, 24, |x, y| {
        let v = 0.02 * 2f32.powi(i32::try_from(x).unwrap_or(0) / 3);
        Rgba([v, v, v * 0.5, if y < 12 { 1.0 } else { 0.5 }])
    })
}

/// Three 32×24 frames (red, green, blue), 100 ms each, looping.
fn write_animated_gif(path: &Path) -> anyhow::Result<()> {
    let file = std::fs::File::create(path)?;
    let mut encoder = image::codecs::gif::GifEncoder::new(file);
    encoder.set_repeat(image::codecs::gif::Repeat::Infinite)?;
    for colour in [[230, 20, 20, 255], [20, 200, 40, 255], [30, 60, 230, 255]] {
        let frame = RgbaImage::from_pixel(32, 24, Rgba(colour));
        encoder.encode_frame(image::Frame::from_parts(
            frame,
            0,
            0,
            image::Delay::from_numer_denom_ms(100, 1),
        ))?;
    }
    Ok(())
}

fn encode_heic(img: &RgbImage, path: &Path) -> anyhow::Result<()> {
    use libheif_rs::{
        Channel, ColorSpace, CompressionFormat, EncoderQuality, HeifContext, Image, LibHeif,
        RgbChroma,
    };
    let (width, height) = img.dimensions();
    let mut heif = Image::new(width, height, ColorSpace::Rgb(RgbChroma::C444))?;
    for channel in [Channel::R, Channel::G, Channel::B] {
        heif.create_plane(channel, width, height, 8)?;
    }
    let planes = heif.planes_mut();
    let (red, green, blue) = (planes.r.context("r")?, planes.g.context("g")?, planes.b.context("b")?);
    let stride = red.stride;
    for (col, row, pixel) in img.enumerate_pixels() {
        let index = usize::try_from(row)? * stride + usize::try_from(col)?;
        for (plane, value) in [
            (&mut *red.data, pixel.0[0]),
            (&mut *green.data, pixel.0[1]),
            (&mut *blue.data, pixel.0[2]),
        ] {
            *plane.get_mut(index).context("plane index")? = value;
        }
    }
    let lib = LibHeif::new();
    let mut ctx = HeifContext::new()?;
    let mut encoder = lib.encoder_for_format(CompressionFormat::Hevc)?;
    encoder.set_quality(EncoderQuality::Lossy(85))?;
    ctx.encode_image(&heif, &mut encoder, None)?;
    ctx.write_to_file(path.to_str().context("non-UTF-8 path")?)?;
    Ok(())
}

fn ffmpeg(dir: &Path, args: &[&str]) -> anyhow::Result<()> {
    let ffmpeg = std::env::var("KINORA_FFMPEG").context("set KINORA_FFMPEG to ffmpeg.exe")?;
    let status = Command::new(&ffmpeg)
        .current_dir(dir)
        .args(["-hide_banner", "-loglevel", "error", "-y"])
        .args(args)
        .status()
        .with_context(|| format!("could not run {ffmpeg}"))?;
    ensure!(status.success(), "ffmpeg failed for {}", args.last().unwrap_or(&""));
    Ok(())
}

/// Files that must produce a friendly error (or a partial picture), never a crash or hang.
fn write_bad_files(dir: &Path) -> anyhow::Result<()> {
    for name in [
        "orient-1.jpg", "rgba.png", "anim.gif", "still.webp", "anim.webp", "still.heic",
        "still.avif", "still.jxl", "still.tif", "still.bmp", "vector.svg", "still.exr",
        "anim-apng.png",
    ] {
        let bytes = std::fs::read(dir.join(name))?;
        let cut = bytes.get(..bytes.len() * 2 / 5).context("cut")?;
        std::fs::write(dir.join(format!("bad-truncated-{name}")), cut)?;
    }
    std::fs::write(dir.join("bad-empty.jpg"), [])?;
    let random: Vec<u8> = (0..4096u32)
        .map(|i| u8::try_from((i * 7919 + 13) % 251).unwrap_or(0))
        .collect();
    std::fs::write(dir.join("bad-random.png"), &random)?;
    std::fs::write(dir.join("bad-bomb.png"), png_bomb_header(100_000, 100_000))?;
    std::fs::write(dir.join("bad-bomb.jpg"), jpeg_bomb_header(65_000, 65_000))?;
    std::fs::write(
        dir.join("bad-bomb.svg"),
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"100000\" height=\"100000\">\
         <rect width=\"100%\" height=\"100%\" fill=\"red\"/></svg>",
    )?;
    // PNG content with a .jpg name: must still open (content sniffing).
    std::fs::copy(dir.join("rgba.png"), dir.join("png-named.jpg"))?;
    Ok(())
}

/// A PNG that only declares a huge size (decompression-bomb style header).
fn png_bomb_header(width: u32, height: u32) -> Vec<u8> {
    let mut ihdr = b"IHDR".to_vec();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    out.extend_from_slice(&13u32.to_be_bytes());
    out.extend_from_slice(&ihdr);
    out.extend_from_slice(&crc32(&ihdr).to_be_bytes());
    out
}

/// A JPEG header (SOI + SOF0) declaring a huge baseline image, with no data.
fn jpeg_bomb_header(width: u16, height: u16) -> Vec<u8> {
    let mut out = vec![0xFF, 0xD8, 0xFF, 0xC0, 0x00, 0x11, 0x08];
    out.extend_from_slice(&height.to_be_bytes());
    out.extend_from_slice(&width.to_be_bytes());
    out.extend_from_slice(&[3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1]);
    out.extend_from_slice(&[0xFF, 0xD9]);
    out
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

const SVG: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="64" height="48" viewBox="0 0 64 48">
  <rect width="64" height="48" fill="#3c7cbc"/>
  <rect x="0" y="0" width="16" height="12" fill="#e61414"/>
  <circle cx="40" cy="28" r="12" fill="#14c828" fill-opacity="0.5"/>
  <image x="48" y="0" width="16" height="12" href="file:///etc/passwd"/>
</svg>
"##;

const SVG_TEXT: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="60">
  <rect width="200" height="60" fill="white"/>
  <text x="10" y="40" font-size="30" fill="black">Kinora</text>
</svg>
"##;
