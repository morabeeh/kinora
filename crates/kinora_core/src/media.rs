//! What counts as a photo or a video, by file extension.
//!
//! The folder list is built from extensions only, so listing a folder never reads file
//! contents (brief §7.6 "Disk"). Opening a file sniffs its content instead
//! (`kinora_decode`), so a mislabelled file still opens.

/// The two kinds of media Kinora shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MediaKind {
    /// A still or animated image.
    Image,
    /// A video (played with libmpv from Phase 3 on).
    Video,
}

/// Lower-case image extensions (v1 formats, brief §5.2).
pub const IMAGE_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "jpe", "jfif", "png", "apng", "gif", "webp", "heic", "heif", "hif", "avif",
    "bmp", "dib", "ico", "tif", "tiff", "tga", "pnm", "pbm", "pgm", "ppm", "pam", "qoi", "dds",
    "hdr", "exr", "svg", "svgz", "jxl",
];

/// Lower-case video extensions (everything libmpv plays that people commonly have).
pub const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "m4v", "mkv", "webm", "mov", "avi", "wmv", "asf", "flv", "f4v", "ts", "m2ts", "mts",
    "3gp", "3g2", "ogv", "mpg", "mpeg", "vob", "divx", "h264", "264", "hevc", "h265", "265",
];

/// The media kind for a file extension (without the dot), ignoring case.
///
/// ```
/// use kinora_core::media::{kind_from_extension, MediaKind};
/// assert_eq!(kind_from_extension("JPG"), Some(MediaKind::Image));
/// assert_eq!(kind_from_extension("mkv"), Some(MediaKind::Video));
/// assert_eq!(kind_from_extension("txt"), None);
/// ```
#[must_use]
pub fn kind_from_extension(extension: &str) -> Option<MediaKind> {
    let matches = |list: &[&str]| list.iter().any(|ext| ext.eq_ignore_ascii_case(extension));
    if matches(IMAGE_EXTENSIONS) {
        Some(MediaKind::Image)
    } else if matches(VIDEO_EXTENSIONS) {
        Some(MediaKind::Video)
    } else {
        None
    }
}

/// The media kind for a file name such as `"Holiday.HEIC"`.
#[must_use]
pub fn kind_from_file_name(name: &str) -> Option<MediaKind> {
    let (_, extension) = name.rsplit_once('.')?;
    kind_from_extension(extension)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extensions_are_case_insensitive() {
        assert_eq!(kind_from_file_name("a.HeIc"), Some(MediaKind::Image));
        assert_eq!(kind_from_file_name("clip.M2TS"), Some(MediaKind::Video));
        assert_eq!(kind_from_file_name("noext"), None);
        assert_eq!(kind_from_file_name("notes.txt"), None);
    }

    #[test]
    fn lists_are_lower_case_and_disjoint() {
        for ext in IMAGE_EXTENSIONS.iter().chain(VIDEO_EXTENSIONS) {
            assert_eq!(*ext, ext.to_lowercase());
        }
        for ext in IMAGE_EXTENSIONS {
            assert!(!VIDEO_EXTENSIONS.contains(ext), "{ext} is in both lists");
        }
    }
}
