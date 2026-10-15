//! JPEG metadata we read ourselves: the EXIF block (orientation, embedded thumbnail).
//!
//! The marker scanner below is a parser we own, so it is written defensively (every read
//! is bounds-checked) and is a fuzzing target in Phase 8.

use image::metadata::Orientation;

/// The TIFF-structured EXIF payload of a JPEG's APP1 segment (after `Exif\0\0`).
#[must_use]
pub fn jpeg_exif(bytes: &[u8]) -> Option<&[u8]> {
    if !bytes.starts_with(&[0xFF, 0xD8]) {
        return None;
    }
    let mut pos = 2usize;
    // Only the header segments are scanned; image data starts at SOS.
    for _ in 0..1024 {
        if *bytes.get(pos)? != 0xFF {
            return None;
        }
        // Skip fill bytes (0xFF 0xFF …).
        while *bytes.get(pos + 1)? == 0xFF {
            pos += 1;
        }
        let marker = *bytes.get(pos + 1)?;
        pos += 2;
        match marker {
            0xD9 | 0xDA => return None,     // EOI, SOS: no EXIF before the image data
            0x01 | 0xD0..=0xD7 => continue, // markers without a length
            _ => {}
        }
        let len = usize::from(u16::from_be_bytes([*bytes.get(pos)?, *bytes.get(pos + 1)?]));
        let payload = bytes.get(pos + 2..pos.checked_add(len)?)?;
        if marker == 0xE1
            && let Some(tiff) = payload.strip_prefix(b"Exif\0\0")
        {
            return Some(tiff);
        }
        pos = pos.checked_add(len)?;
    }
    None
}

/// The EXIF orientation of a JPEG (`NoTransforms` when absent or invalid).
#[must_use]
pub fn jpeg_orientation(bytes: &[u8]) -> Orientation {
    jpeg_exif(bytes)
        .and_then(Orientation::from_exif_chunk)
        .unwrap_or(Orientation::NoTransforms)
}

/// `true` if applying `orientation` swaps width and height.
#[must_use]
pub fn swaps_axes(orientation: Orientation) -> bool {
    matches!(
        orientation,
        Orientation::Rotate90
            | Orientation::Rotate270
            | Orientation::Rotate90FlipH
            | Orientation::Rotate270FlipH
    )
}

/// The small JPEG thumbnail many cameras and phones embed in the EXIF block (typically
/// 160×120). Showing it first gives an instant picture on slow PCs (brief §7.6).
#[must_use]
pub fn jpeg_thumbnail_bytes(bytes: &[u8]) -> Option<Vec<u8>> {
    use exif::{In, Tag};
    let tiff = jpeg_exif(bytes)?;
    let exif = exif::Reader::new().read_raw(tiff.to_vec()).ok()?;
    let field = |tag| {
        exif.get_field(tag, In::THUMBNAIL)
            .and_then(|f| f.value.get_uint(0))
            .and_then(|v| usize::try_from(v).ok())
    };
    let offset = field(Tag::JPEGInterchangeFormat)?;
    let length = field(Tag::JPEGInterchangeFormatLength)?;
    let thumb = exif.buf().get(offset..offset.checked_add(length)?)?;
    thumb.starts_with(&[0xFF, 0xD8]).then(|| thumb.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segment(marker: u8, payload: &[u8]) -> Vec<u8> {
        let mut out = vec![0xFF, marker];
        out.extend_from_slice(&u16::try_from(payload.len() + 2).unwrap().to_be_bytes());
        out.extend_from_slice(payload);
        out
    }

    fn exif_orientation(value: u16) -> Vec<u8> {
        let mut tiff = b"Exif\0\0II*\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0".to_vec();
        tiff.extend_from_slice(&value.to_le_bytes());
        tiff.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
        tiff
    }

    #[test]
    fn finds_exif_after_other_segments() {
        let mut jpeg = vec![0xFF, 0xD8];
        jpeg.extend(segment(0xE0, b"JFIF\0\x01\x02"));
        jpeg.extend(segment(0xE1, &exif_orientation(6)));
        jpeg.extend([0xFF, 0xDA, 0, 2]);
        assert_eq!(jpeg_orientation(&jpeg), Orientation::Rotate90);
        assert!(swaps_axes(jpeg_orientation(&jpeg)));
    }

    #[test]
    fn missing_or_broken_exif_means_no_rotation() {
        assert_eq!(
            jpeg_orientation(b"\xFF\xD8\xFF\xDA"),
            Orientation::NoTransforms
        );
        assert_eq!(jpeg_orientation(b"not a jpeg"), Orientation::NoTransforms);
        // Segment length pointing past the end of the file.
        assert_eq!(
            jpeg_orientation(b"\xFF\xD8\xFF\xE1\xFF\xFFExif"),
            Orientation::NoTransforms
        );
        // Garbage instead of a marker.
        assert_eq!(
            jpeg_orientation(b"\xFF\xD8\x00\x00"),
            Orientation::NoTransforms
        );
        assert_eq!(jpeg_thumbnail_bytes(b"\xFF\xD8\xFF\xD9"), None);
    }

    #[test]
    fn every_prefix_of_a_header_is_safe() {
        let mut jpeg = vec![0xFF, 0xD8];
        jpeg.extend(segment(0xE1, &exif_orientation(3)));
        for len in 0..=jpeg.len() {
            let _ = jpeg_orientation(jpeg.get(..len).unwrap());
            let _ = jpeg_thumbnail_bytes(jpeg.get(..len).unwrap());
        }
    }
}
