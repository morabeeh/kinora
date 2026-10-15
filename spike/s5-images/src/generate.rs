//! Generates the S5 test images (all synthetic, so they have no license questions).
//! Formats `image` can't write (AVIF, JPEG XL, animated WebP/APNG/GIF) come from `FFmpeg`
//! (`KINORA_FFMPEG` env var); HEIC comes from libheif's x265 encoder.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Context as _;

pub const CANON_W: u32 = 4000;
pub const CANON_H: u32 = 3000;
/// Marker squares in the canonical image (top-left red, top-right green, bottom-left blue).
pub const MARK: u32 = 500;

/// 12 MP canonical image: gradient + three coloured corner squares, so orientation can be
/// checked automatically by looking for the red square.
pub fn canonical_rgb(width: u32, height: u32) -> image::RgbImage {
    image::RgbImage::from_fn(width, height, |x, y| {
        let in_left = x < MARK;
        let in_top = y < MARK;
        let in_right = x >= width - MARK;
        let in_bottom = y >= height - MARK;
        if in_left && in_top {
            image::Rgb([230, 20, 20])
        } else if in_right && in_top {
            image::Rgb([20, 200, 40])
        } else if in_left && in_bottom {
            image::Rgb([30, 60, 230])
        } else {
            let r = u8::try_from(x * 160 / width + 40).unwrap_or(u8::MAX);
            let g = u8::try_from(y * 160 / height + 40).unwrap_or(u8::MAX);
            image::Rgb([r, g, 120])
        }
    })
}

/// A 1-pixel black/white checkerboard: at exactly 100% (1 image px = 1 screen px) it looks
/// like flat grey; any scaling shows obvious moiré bands.
fn checker(width: u32, height: u32) -> image::RgbImage {
    image::RgbImage::from_fn(width, height, |x, y| {
        if (x + y) % 2 == 0 {
            image::Rgb([0, 0, 0])
        } else {
            image::Rgb([255, 255, 255])
        }
    })
}

pub fn generate(dir: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    let canon = canonical_rgb(CANON_W, CANON_H);
    let base = dir.join("base.png");
    canon.save(&base)?;
    tracing::info!("wrote {}", base.display());

    // JPEG at quality 90 with each of the 8 EXIF orientations (same pixels, different tag).
    let jpeg = encode_jpeg(&canon, 90)?;
    for orientation in 1..=8u16 {
        std::fs::write(
            dir.join(format!("orient-{orientation}.jpg")),
            with_exif_orientation(&jpeg, orientation)?,
        )?;
    }
    // 24 MP photo for zoom/pan (S7) and scaled-decode timing.
    std::fs::write(
        dir.join("photo-24mp.jpg"),
        encode_jpeg(&canonical_rgb(6000, 4000), 90)?,
    )?;
    checker(1600, 1000).save(dir.join("checker-1px.png"))?;

    // Formats the `image` crate writes itself.
    image::DynamicImage::ImageRgb8(canon.clone())
        .into_rgb16()
        .save(dir.join("rgb16.png"))?;
    for ext in ["bmp", "tiff", "tga", "qoi", "ppm", "webp"] {
        canon.save(dir.join(format!("still.{ext}")))?;
    }
    image::imageops::thumbnail(&canon, 256, 192).save(dir.join("icon.ico"))?;

    encode_heic(&canon, &dir.join("still.heic"))?;
    run_ffmpeg(
        dir,
        &[
            "-i",
            "base.png",
            "-c:v",
            "libaom-av1",
            "-still-picture",
            "1",
            "-crf",
            "30",
            "-cpu-used",
            "6",
            "still.avif",
        ],
    )?;
    run_ffmpeg(
        dir,
        &[
            "-i",
            "base.png",
            "-c:v",
            "libjxl",
            "-distance",
            "1",
            "still.jxl",
        ],
    )?;
    let anim = [
        "-f",
        "lavfi",
        "-i",
        "testsrc2=size=640x360:rate=15",
        "-t",
        "2",
    ];
    run_ffmpeg(
        dir,
        &[
            &anim[..],
            &["-c:v", "libwebp_anim", "-loop", "0", "anim.webp"],
        ]
        .concat(),
    )?;
    run_ffmpeg(
        dir,
        &[&anim[..], &["-plays", "0", "-f", "apng", "anim-apng.png"]].concat(),
    )?;
    run_ffmpeg(dir, &[&anim[..], &["anim.gif"]].concat())?;

    write_bad_files(dir)?;
    Ok(())
}

fn encode_jpeg(img: &image::RgbImage, quality: u8) -> anyhow::Result<Vec<u8>> {
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality).encode_image(img)?;
    Ok(out)
}

/// Inserts a minimal EXIF APP1 segment (IFD0 with only the Orientation tag) after SOI.
fn with_exif_orientation(jpeg: &[u8], orientation: u16) -> anyhow::Result<Vec<u8>> {
    let mut tiff = vec![b'I', b'I', 42, 0, 8, 0, 0, 0]; // little-endian header, IFD0 at 8
    tiff.extend_from_slice(&1u16.to_le_bytes()); // one entry
    tiff.extend_from_slice(&0x0112u16.to_le_bytes()); // tag: Orientation
    tiff.extend_from_slice(&3u16.to_le_bytes()); // type: SHORT
    tiff.extend_from_slice(&1u32.to_le_bytes()); // count
    tiff.extend_from_slice(&orientation.to_le_bytes());
    tiff.extend_from_slice(&[0, 0]); // value padding
    tiff.extend_from_slice(&0u32.to_le_bytes()); // no next IFD
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

fn encode_heic(img: &image::RgbImage, path: &Path) -> anyhow::Result<()> {
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
    let (red, green, blue) = (
        planes.r.context("r")?,
        planes.g.context("g")?,
        planes.b.context("b")?,
    );
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
    encoder.set_quality(EncoderQuality::Lossy(80))?;
    ctx.encode_image(&heif, &mut encoder, None)?;
    ctx.write_to_file(path.to_str().context("non-UTF-8 path")?)?;
    Ok(())
}

fn run_ffmpeg(dir: &Path, args: &[&str]) -> anyhow::Result<()> {
    let ffmpeg = std::env::var("KINORA_FFMPEG").context("set KINORA_FFMPEG to ffmpeg.exe")?;
    let status = Command::new(&ffmpeg)
        .current_dir(dir)
        .args(["-hide_banner", "-loglevel", "error", "-y"])
        .args(args)
        .status()
        .with_context(|| format!("could not run {ffmpeg}"))?;
    anyhow::ensure!(
        status.success(),
        "ffmpeg failed for {}",
        args.last().unwrap_or(&"")
    );
    Ok(())
}

/// Files that must produce a friendly error, never a crash.
fn write_bad_files(dir: &Path) -> anyhow::Result<()> {
    let good: Vec<PathBuf> = [
        "orient-1.jpg",
        "base.png",
        "still.webp",
        "still.heic",
        "still.avif",
        "still.jxl",
        "anim.gif",
        "still.tiff",
    ]
    .iter()
    .map(|name| dir.join(name))
    .collect();
    for path in good {
        let bytes = std::fs::read(&path)?;
        let cut = bytes.get(..bytes.len() * 2 / 5).context("cut")?;
        let name = path.file_name().and_then(|n| n.to_str()).context("name")?;
        std::fs::write(dir.join(format!("bad-truncated-{name}")), cut)?;
    }
    std::fs::write(dir.join("bad-empty.jpg"), [])?;
    std::fs::write(
        dir.join("bad-random.png"),
        (0..4096u32)
            .map(|i| u8::try_from((i * 7919) % 251).unwrap_or(0))
            .collect::<Vec<u8>>(),
    )?;
    std::fs::write(dir.join("bad-bomb.png"), png_bomb_header(100_000, 100_000))?;
    // A PNG with a .jpg extension: must still open (content sniffing).
    std::fs::copy(dir.join("base.png"), dir.join("png-named.jpg")).map(|_| ())?;
    Ok(())
}

/// A PNG that only declares a huge size (decompression-bomb style header).
fn png_bomb_header(width: u32, height: u32) -> Vec<u8> {
    let mut ihdr = b"IHDR".to_vec();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit RGBA
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    out.extend_from_slice(&13u32.to_be_bytes());
    out.extend_from_slice(&ihdr);
    out.extend_from_slice(&crc32(&ihdr).to_be_bytes());
    out
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}
