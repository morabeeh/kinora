//! `cargo xtask gen-folder <dir>`: writes a folder of synthetic, mixed-format "photos" for
//! the Phase 2 flick test ("500 mixed-format photos with no visible delay").
//!
//! Every file is generated here (no licensing questions). Mostly JPEGs of 2–12 MP with mild
//! noise (so they decode like real photos, not like flat gradients), some with an EXIF
//! rotation, plus PNG/WebP/GIF/BMP/TIFF, copies of the HEIC/AVIF/JPEG XL/SVG fixtures from
//! `test_data/images` when present, and a few corrupt files that must show an error card.

use std::path::{Path, PathBuf};

use image::{ImageEncoder as _, RgbImage};

struct Options {
    dir: PathBuf,
    count: u32,
    megapixels: u32,
}

pub fn run(args: &[String]) -> Result<(), String> {
    let options = parse(args)?;
    std::fs::create_dir_all(&options.dir)
        .map_err(|err| format!("cannot create {}: {err}", options.dir.display()))?;
    let fixtures = super::workspace_root().join("test_data").join("images");
    let workers = std::thread::available_parallelism().map_or(4, std::num::NonZero::get);
    let workers = u32::try_from(workers).unwrap_or(4);
    println!(
        "writing {} files to {} with {workers} threads…",
        options.count,
        options.dir.display()
    );
    let results: Vec<Result<(), String>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|worker| {
                let options = &options;
                let fixtures = &fixtures;
                scope.spawn(move || -> Result<(), String> {
                    let mut index = worker;
                    while index < options.count {
                        write_item(options, fixtures, index)?;
                        index += workers;
                    }
                    Ok(())
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .unwrap_or_else(|_| Err(String::from("a worker thread panicked")))
            })
            .collect()
    });
    results.into_iter().collect::<Result<Vec<()>, String>>()?;
    println!("done.");
    Ok(())
}

fn parse(args: &[String]) -> Result<Options, String> {
    let mut iter = args.iter();
    let dir = iter
        .next()
        .ok_or("usage: cargo xtask gen-folder <dir> [--count N] [--megapixels M]")?;
    let mut options = Options {
        dir: PathBuf::from(dir),
        count: 500,
        megapixels: 12,
    };
    while let Some(flag) = iter.next() {
        let value = iter.next().ok_or(format!("{flag} needs a value"))?;
        let number: u32 = value
            .parse()
            .map_err(|_| format!("{flag} expects a number, got `{value}`"))?;
        match flag.as_str() {
            "--count" => options.count = number,
            "--megapixels" => options.megapixels = number.clamp(1, 100),
            other => return Err(format!("unknown option `{other}`")),
        }
    }
    Ok(options)
}

/// Picks a format for item `index` (deterministic, so two runs give the same folder).
fn write_item(options: &Options, fixtures: &Path, index: u32) -> Result<(), String> {
    // Names like "photo 7.jpg" and "photo 10.jpg" exercise the natural sort.
    let number = index + 1;
    let path = |ext: &str| options.dir.join(format!("photo {number}.{ext}"));
    let small = (1600, 1200); // non-JPEG formats stay ≈2 MP so the folder stays a sane size
    match index % 50 {
        // 1 in 50: corrupt files (truncated JPEG / random bytes).
        13 => {
            let jpeg = encode_jpeg(&photo(index, 1200, 900), None)?;
            let cut = jpeg.get(..jpeg.len() / 3).unwrap_or_default();
            write(&path("jpg"), cut)
        }
        37 => write(&path("png"), &noise_bytes(index, 5000)),
        // Fixtures in formats `image` can't write: copied (and renamed) when available.
        7 => copy_fixture(fixtures, "still.heic", &path("heic"), index),
        17 => copy_fixture(fixtures, "still.avif", &path("avif"), index),
        27 => copy_fixture(fixtures, "still.jxl", &path("jxl"), index),
        47 => copy_fixture(fixtures, "vector.svg", &path("svg"), index),
        3 | 23 => save(&photo(index, small.0, small.1), &path("png")),
        9 | 29 => save(&photo(index, small.0, small.1), &path("webp")),
        19 => save(&photo(index, 800, 600), &path("gif")),
        33 => save(&photo(index, small.0, small.1), &path("bmp")),
        43 => save(&photo(index, small.0, small.1), &path("tiff")),
        _ => {
            let (width, height) = jpeg_size(index, options.megapixels);
            // Every 8th JPEG is stored sideways with EXIF orientation 6 (like phone photos).
            let orientation = (index % 8 == 5).then_some(6);
            let (w, h) = if orientation.is_some() {
                (height, width)
            } else {
                (width, height)
            };
            write(
                &path("jpg"),
                &encode_jpeg(&photo(index, w, h), orientation)?,
            )
        }
    }
}

/// JPEG sizes cycle through 2, 5, 8 and the maximum megapixels, in 4:3, 3:2 and portrait.
fn jpeg_size(index: u32, max_megapixels: u32) -> (u32, u32) {
    let megapixels = match index % 4 {
        0 => 2,
        1 => 5,
        2 => 8,
        _ => max_megapixels,
    }
    .min(max_megapixels);
    let (aw, ah): (u64, u64) = match index % 3 {
        0 => (4, 3),
        1 => (3, 2),
        _ => (3, 4),
    };
    // width = sqrt(MP × 1e6 × aw / ah), using integer square root.
    let area = u64::from(megapixels) * 1_000_000;
    let width = (area * aw / ah).isqrt();
    let height = width * ah / aw;
    (
        u32::try_from(width).unwrap_or(4000),
        u32::try_from(height).unwrap_or(3000),
    )
}

/// A colourful gradient (hue changes per item) with a big item number-ish block pattern and
/// mild pseudo-random noise.
fn photo(index: u32, width: u32, height: u32) -> RgbImage {
    let hue = index.wrapping_mul(47) % 256;
    RgbImage::from_fn(width, height, |x, y| {
        let gx = x * 255 / width.max(1);
        let gy = y * 255 / height.max(1);
        let block = ((x / 97 + y / 97 + index) % 7) * 12;
        let noise = hash(x, y, index) % 17;
        let channel = |base: u32| u8::try_from((base + block + noise) % 256).unwrap_or(0);
        image::Rgb([
            channel(gx + hue),
            channel(gy + 255 - hue),
            channel(gx.midpoint(gy) + hue / 2),
        ])
    })
}

fn hash(x: u32, y: u32, seed: u32) -> u32 {
    let mut h = x
        .wrapping_mul(0x9E37_79B1)
        .wrapping_add(y.wrapping_mul(0x85EB_CA77))
        .wrapping_add(seed.wrapping_mul(0xC2B2_AE3D));
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^ (h >> 12)
}

fn noise_bytes(seed: u32, len: u32) -> Vec<u8> {
    (0..len)
        .map(|i| u8::try_from(hash(i, 7, seed) % 256).unwrap_or(0))
        .collect()
}

fn encode_jpeg(img: &RgbImage, orientation: Option<u16>) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 88)
        .write_image(
            img.as_raw(),
            img.width(),
            img.height(),
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|err| format!("JPEG encode failed: {err}"))?;
    match orientation {
        Some(value) => with_exif_orientation(&out, value),
        None => Ok(out),
    }
}

/// Inserts a minimal EXIF APP1 segment (IFD0 with only the Orientation tag) after SOI.
fn with_exif_orientation(jpeg: &[u8], orientation: u16) -> Result<Vec<u8>, String> {
    let mut tiff = vec![b'I', b'I', 42, 0, 8, 0, 0, 0]; // little-endian header, IFD0 at 8
    tiff.extend_from_slice(&1u16.to_le_bytes()); // one entry
    tiff.extend_from_slice(&0x0112u16.to_le_bytes()); // tag: Orientation
    tiff.extend_from_slice(&3u16.to_le_bytes()); // type: SHORT
    tiff.extend_from_slice(&1u32.to_le_bytes()); // count
    tiff.extend_from_slice(&orientation.to_le_bytes());
    tiff.extend_from_slice(&[0, 0]); // value padding
    tiff.extend_from_slice(&0u32.to_le_bytes()); // no next IFD
    let length = u16::try_from(2 + 6 + tiff.len()).map_err(|err| err.to_string())?;
    let (soi, rest) = jpeg.split_at_checked(2).ok_or("not a JPEG")?;
    let mut out = soi.to_vec();
    out.extend_from_slice(&[0xFF, 0xE1]);
    out.extend_from_slice(&length.to_be_bytes());
    out.extend_from_slice(b"Exif\0\0");
    out.extend_from_slice(&tiff);
    out.extend_from_slice(rest);
    Ok(out)
}

fn save(img: &RgbImage, path: &Path) -> Result<(), String> {
    img.save(path)
        .map_err(|err| format!("cannot write {}: {err}", path.display()))
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(path, bytes).map_err(|err| format!("cannot write {}: {err}", path.display()))
}

/// Copies a fixture if it exists; otherwise writes a JPEG so the count stays the same.
fn copy_fixture(fixtures: &Path, name: &str, dest: &Path, index: u32) -> Result<(), String> {
    let source = fixtures.join(name);
    if source.is_file() {
        std::fs::copy(&source, dest)
            .map(|_| ())
            .map_err(|err| format!("cannot copy {}: {err}", source.display()))
    } else {
        write(
            &dest.with_extension("jpg"),
            &encode_jpeg(&photo(index, 2000, 1500), None)?,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jpeg_sizes_respect_the_megapixel_cap() {
        for index in 0..12 {
            let (w, h) = jpeg_size(index, 12);
            let mp = u64::from(w) * u64::from(h);
            assert!(mp <= 12_100_000, "{w}×{h}");
            assert!(mp >= 1_900_000, "{w}×{h}");
        }
        let (w, h) = jpeg_size(3, 1);
        assert!(u64::from(w) * u64::from(h) <= 1_010_000);
    }

    #[test]
    fn exif_segment_is_inserted_after_soi() {
        let jpeg = [0xFF, 0xD8, 0xFF, 0xDB, 0x00];
        let out = with_exif_orientation(&jpeg, 6).unwrap();
        assert_eq!(&out[..4], &[0xFF, 0xD8, 0xFF, 0xE1]);
        assert_eq!(&out[out.len() - 3..], &[0xFF, 0xDB, 0x00]);
    }
}
