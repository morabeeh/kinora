//! Image format detection by **content** (magic bytes), with the file extension only as a
//! fallback for formats that have no magic bytes (TGA) — brief §5.1, ADR 0005.

use std::fmt;

/// Every image format Kinora decodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Format {
    /// JPEG / JFIF.
    Jpeg,
    /// PNG and animated PNG (APNG).
    Png,
    /// GIF (still or animated).
    Gif,
    /// WebP (still or animated).
    WebP,
    /// HEIC / HEIF (HEVC in a HEIF container).
    Heif,
    /// AVIF (AV1 in a HEIF container).
    Avif,
    /// JPEG XL.
    Jxl,
    /// Windows bitmap.
    Bmp,
    /// Windows icon / cursor.
    Ico,
    /// TIFF.
    Tiff,
    /// Truevision TGA (no magic bytes: detected by extension).
    Tga,
    /// Portable any-map (PBM/PGM/PPM/PAM).
    Pnm,
    /// Quite OK Image.
    Qoi,
    /// DDS (`DirectDraw` Surface).
    Dds,
    /// Radiance HDR.
    Hdr,
    /// `OpenEXR`.
    Exr,
    /// SVG (rendered with resvg).
    Svg,
}

impl Format {
    /// A short, human-readable name ("JPEG", "HEIC", …).
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Jpeg => "JPEG",
            Self::Png => "PNG",
            Self::Gif => "GIF",
            Self::WebP => "WebP",
            Self::Heif => "HEIC",
            Self::Avif => "AVIF",
            Self::Jxl => "JPEG XL",
            Self::Bmp => "BMP",
            Self::Ico => "ICO",
            Self::Tiff => "TIFF",
            Self::Tga => "TGA",
            Self::Pnm => "PNM",
            Self::Qoi => "QOI",
            Self::Dds => "DDS",
            Self::Hdr => "HDR",
            Self::Exr => "OpenEXR",
            Self::Svg => "SVG",
        }
    }
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Detects the format from the first bytes of a file (a few hundred bytes are enough;
/// SVG detection looks at up to 4 KiB).
///
/// ```
/// use kinora_decode::format::{sniff, Format};
/// assert_eq!(sniff(b"\x89PNG\r\n\x1a\n...."), Some(Format::Png));
/// assert_eq!(sniff(b"hello"), None);
/// ```
#[must_use]
pub fn sniff(bytes: &[u8]) -> Option<Format> {
    let at = |range: std::ops::Range<usize>| bytes.get(range).unwrap_or_default();
    let format = match bytes {
        [0xFF, 0xD8, 0xFF, ..] => Format::Jpeg,
        [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, ..] => Format::Png,
        [b'G', b'I', b'F', b'8', b'7' | b'9', b'a', ..] => Format::Gif,
        [0xFF, 0x0A, ..]
        | [
            0,
            0,
            0,
            0x0C,
            b'J',
            b'X',
            b'L',
            b' ',
            0x0D,
            0x0A,
            0x87,
            0x0A,
            ..,
        ] => Format::Jxl,
        [b'B', b'M', ..] => Format::Bmp,
        [0, 0, 1 | 2, 0, ..] if looks_like_ico(bytes) => Format::Ico,
        [b'I', b'I', 0x2A, 0, ..] | [b'M', b'M', 0, 0x2A, ..] => Format::Tiff,
        [b'q', b'o', b'i', b'f', ..] => Format::Qoi,
        [b'D', b'D', b'S', b' ', ..] => Format::Dds,
        [0x76, 0x2F, 0x31, 0x01, ..] => Format::Exr,
        [b'P', b'1'..=b'7', b' ' | b'\t' | b'\r' | b'\n', ..] => Format::Pnm,
        _ if at(0..4) == b"RIFF" && at(8..12) == b"WEBP" => Format::WebP,
        _ if at(0..10) == b"#?RADIANCE" || at(0..6) == b"#?RGBE" => Format::Hdr,
        _ if at(4..8) == b"ftyp" => return sniff_heif_brand(bytes),
        _ if looks_like_svg(bytes) => Format::Svg,
        _ => return None,
    };
    Some(format)
}

/// HEIF-family files start with an `ftyp` box listing "brands". AVIF files may also carry
/// the generic `mif1` brand, so AVIF brands are checked first.
fn sniff_heif_brand(bytes: &[u8]) -> Option<Format> {
    let size = usize::try_from(u32::from_be_bytes(bytes.get(0..4)?.try_into().ok()?)).ok()?;
    let ftyp = bytes.get(8..size.clamp(8, 4096).min(bytes.len()))?;
    // Major brand (4 bytes), minor version (4 bytes), then compatible brands.
    let brands = ftyp
        .as_chunks::<4>()
        .0
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != 1)
        .map(|(_, brand)| brand);
    let mut heif = false;
    for brand in brands {
        match brand {
            b"avif" | b"avis" => return Some(Format::Avif),
            b"heic" | b"heix" | b"hevc" | b"hevx" | b"heim" | b"heis" | b"mif1" | b"msf1" => {
                heif = true;
            }
            _ => {}
        }
    }
    heif.then_some(Format::Heif)
}

/// ICO/CUR start with `00 00 01 00` / `00 00 02 00`, but so do many TGA files (TGA has no
/// magic bytes). An icon also has 1–256 directory entries whose "reserved" byte is zero.
fn looks_like_ico(bytes: &[u8]) -> bool {
    let count = bytes
        .get(4..6)
        .and_then(|b| b.try_into().ok())
        .map_or(0, u16::from_le_bytes);
    (1..=256).contains(&count) && bytes.get(9) == Some(&0)
}

/// SVG is XML text: an optional BOM, whitespace, comments or an XML declaration, then an
/// `<svg` element near the start.
fn looks_like_svg(bytes: &[u8]) -> bool {
    let head = bytes.get(..bytes.len().min(4096)).unwrap_or_default();
    let head = head.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(head);
    let trimmed = head.trim_ascii_start();
    trimmed.starts_with(b"<") && head.windows(4).any(|window| window == b"<svg")
}

/// The format a file extension suggests (case-insensitive, without the dot).
#[must_use]
pub fn from_extension(extension: &str) -> Option<Format> {
    let ext = extension.to_ascii_lowercase();
    Some(match ext.as_str() {
        "jpg" | "jpeg" | "jpe" | "jfif" => Format::Jpeg,
        "png" | "apng" => Format::Png,
        "gif" => Format::Gif,
        "webp" => Format::WebP,
        "heic" | "heif" | "hif" => Format::Heif,
        "avif" => Format::Avif,
        "jxl" => Format::Jxl,
        "bmp" | "dib" => Format::Bmp,
        "ico" | "cur" => Format::Ico,
        "tif" | "tiff" => Format::Tiff,
        "tga" => Format::Tga,
        "pnm" | "pbm" | "pgm" | "ppm" | "pam" => Format::Pnm,
        "qoi" => Format::Qoi,
        "dds" => Format::Dds,
        "hdr" => Format::Hdr,
        "exr" => Format::Exr,
        "svg" | "svgz" => Format::Svg,
        _ => return None,
    })
}

/// Content sniffing first; the extension is used only for formats without magic bytes
/// (TGA) and for gzip-compressed SVG (`.svgz`). A `.jpg` that is really a PNG opens as PNG.
#[must_use]
pub fn detect(bytes: &[u8], extension: Option<&str>) -> Option<Format> {
    sniff(bytes).or_else(|| {
        let by_extension = from_extension(extension?)?;
        match by_extension {
            Format::Tga => Some(Format::Tga),
            Format::Svg if bytes.starts_with(&[0x1F, 0x8B]) => Some(Format::Svg),
            _ => None,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ftyp(major: [u8; 4], compatible: &[&[u8; 4]]) -> Vec<u8> {
        let size = u32::try_from(16 + 4 * compatible.len()).unwrap();
        let mut out = size.to_be_bytes().to_vec();
        out.extend_from_slice(b"ftyp");
        out.extend_from_slice(&major);
        out.extend_from_slice(&[0, 0, 0, 0]);
        for brand in compatible {
            out.extend_from_slice(*brand);
        }
        out
    }

    #[test]
    fn magic_bytes() {
        let cases: &[(&[u8], Format)] = &[
            (b"\xFF\xD8\xFF\xE0", Format::Jpeg),
            (b"GIF89a....", Format::Gif),
            (b"RIFF\0\0\0\0WEBPVP8 ", Format::WebP),
            (b"\xFF\x0A....", Format::Jxl),
            (b"\0\0\0\x0CJXL \x0D\x0A\x87\x0A", Format::Jxl),
            (b"BM......", Format::Bmp),
            (b"\0\0\x01\0\x01\0\x10\x10\0\0\x01\0", Format::Ico),
            (b"II*\0....", Format::Tiff),
            (b"MM\0*....", Format::Tiff),
            (b"qoif....", Format::Qoi),
            (b"DDS |...", Format::Dds),
            (b"v/1\x01....", Format::Exr),
            (b"P6\n640 480\n255\n", Format::Pnm),
            (b"#?RADIANCE\n", Format::Hdr),
            (
                b"\xEF\xBB\xBF<?xml version=\"1.0\"?>\n<svg xmlns=",
                Format::Svg,
            ),
            (b"  <svg width=\"10\"", Format::Svg),
        ];
        for (bytes, expected) in cases {
            assert_eq!(sniff(bytes), Some(*expected), "{bytes:?}");
        }
    }

    #[test]
    fn heif_family_brands() {
        assert_eq!(
            sniff(&ftyp(*b"heic", &[b"mif1", b"heic"])),
            Some(Format::Heif)
        );
        assert_eq!(sniff(&ftyp(*b"avif", &[b"mif1"])), Some(Format::Avif));
        assert_eq!(
            sniff(&ftyp(*b"mif1", &[b"avif", b"miaf"])),
            Some(Format::Avif)
        );
        assert_eq!(sniff(&ftyp(*b"mif1", &[b"heic"])), Some(Format::Heif));
        assert_eq!(
            sniff(&ftyp(*b"isom", &[b"mp41"])),
            None,
            "an MP4 video is not an image"
        );
    }

    #[test]
    fn not_images() {
        for bytes in [
            &b""[..],
            b"hello world",
            b"<html><body>",
            b"\0\0\0\0",
            b"P9 nope",
        ] {
            assert_eq!(sniff(bytes), None, "{bytes:?}");
        }
    }

    #[test]
    fn extension_is_only_a_fallback() {
        let png = b"\x89PNG\r\n\x1a\n";
        assert_eq!(detect(png, Some("jpg")), Some(Format::Png));
        // A typical uncompressed TGA header starts like a cursor file, but isn't one.
        let tga = b"\0\0\x02\0\0\0\0\0\0\0\0\0\x40\0\x30\0\x20\0";
        assert_eq!(detect(tga, Some("tga")), Some(Format::Tga));
        assert_eq!(detect(b"\0\x01\x0A\0", Some("TGA")), Some(Format::Tga));
        assert_eq!(detect(b"\x1F\x8B\x08", Some("svgz")), Some(Format::Svg));
        assert_eq!(detect(b"random", Some("jpg")), None);
        assert_eq!(detect(b"random", None), None);
    }

    #[test]
    fn truncated_headers_do_not_panic() {
        for len in 0..12 {
            let bytes = &b"\0\0\0\x18ftypheic"[..len.min(12)];
            let _ = sniff(bytes);
        }
        let _ = sniff(b"\xFF\xFF\xFF\xFFftyp");
    }
}
