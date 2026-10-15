//! Decode jobs: what runs on the worker threads, and how results get back to the UI.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use kinora_decode::{DecodeError, DecodeOptions, DecodedImage};

use crate::ui::ErrorKind;

/// How good a decoded picture is. Ordered: a better quality replaces a worse one on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Quality {
    /// The small JPEG embedded in the EXIF data (instant, blurry).
    Thumbnail,
    /// Big enough for "fit" on this screen (scaled JPEG decoding).
    Screen,
    /// Full resolution, for zooming in.
    Full,
}

/// Cache and job key: one picture at one quality.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImageKey {
    pub path: PathBuf,
    pub quality: Quality,
}

impl ImageKey {
    pub fn new(path: PathBuf, quality: Quality) -> Self {
        Self { path, quality }
    }
}

/// Why a picture can't be shown, in the form the error card needs.
#[derive(Debug, Clone, PartialEq)]
pub struct Failure {
    pub kind: ErrorKind,
    /// Format name for messages like "This HEIC file is damaged".
    pub format: String,
    /// Technical detail (small print on the card, full text in the log).
    pub detail: String,
}

impl Failure {
    pub fn from_error(error: &DecodeError) -> Self {
        // The card's title already says what happened; the small print adds only the
        // technical detail.
        let detail = match error {
            DecodeError::Io(io) => io.to_string(),
            DecodeError::Corrupt { detail, .. } => detail.clone(),
            DecodeError::TooLarge { width, height } if *width > 0 => {
                format!("{width} × {height} pixels")
            }
            _ => String::new(),
        };
        let (kind, format) = match error {
            DecodeError::Io(io) => (
                match io.kind() {
                    std::io::ErrorKind::NotFound => ErrorKind::NotFound,
                    std::io::ErrorKind::PermissionDenied => ErrorKind::NoPermission,
                    _ => ErrorKind::Unreadable,
                },
                String::new(),
            ),
            DecodeError::Empty => (ErrorKind::Empty, String::new()),
            DecodeError::UnknownFormat => (ErrorKind::UnknownFormat, String::new()),
            DecodeError::Unavailable(format) => (ErrorKind::Unavailable, format.to_string()),
            DecodeError::TooLarge { .. } => (ErrorKind::TooLarge, String::new()),
            DecodeError::Corrupt { format, .. } => (ErrorKind::Damaged, format.to_string()),
            DecodeError::Panicked(format) => (ErrorKind::DecoderFailed, format.to_string()),
        };
        Self {
            kind,
            format,
            detail,
        }
    }

    pub fn panicked() -> Self {
        Self {
            kind: ErrorKind::DecoderFailed,
            format: String::new(),
            detail: String::from("the decoder stopped unexpectedly"),
        }
    }

    pub fn video() -> Self {
        Self {
            kind: ErrorKind::VideoNotYet,
            format: String::new(),
            detail: String::new(),
        }
    }
}

/// A finished job, delivered to the UI thread.
#[derive(Debug)]
pub struct Outcome {
    pub key: ImageKey,
    pub result: Result<Arc<DecodedImage>, Failure>,
    pub elapsed: Duration,
}

/// Reads and decodes one file. With `thumbnail_first` (Lite mode, the item on screen), the
/// embedded EXIF thumbnail is delivered before the real decode starts, so slow PCs show
/// something within a few milliseconds (brief §7.6).
pub fn decode(
    key: &ImageKey,
    options: &DecodeOptions,
    thumbnail_first: bool,
    deliver: &dyn Fn(Outcome),
) {
    let started = Instant::now();
    let bytes = match kinora_decode::read_file(&key.path) {
        Ok(bytes) => bytes,
        Err(error) => {
            deliver(Outcome {
                key: key.clone(),
                result: Err(Failure::from_error(&error)),
                elapsed: started.elapsed(),
            });
            return;
        }
    };
    if thumbnail_first && let Some(thumbnail) = kinora_decode::embedded_thumbnail(&bytes) {
        deliver(Outcome {
            key: ImageKey::new(key.path.clone(), Quality::Thumbnail),
            result: Ok(Arc::new(thumbnail)),
            elapsed: started.elapsed(),
        });
    }
    let extension = key.path.extension().and_then(|ext| ext.to_str());
    let result = kinora_decode::decode(&bytes, extension, options);
    if let Err(error) = &result {
        tracing::info!(file = %key.path.display(), "can't show: {error}");
    }
    deliver(Outcome {
        key: key.clone(),
        result: result
            .map(Arc::new)
            .map_err(|error| Failure::from_error(&error)),
        elapsed: started.elapsed(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_map_to_card_kinds() {
        let missing = DecodeError::Io(std::io::Error::from(std::io::ErrorKind::NotFound));
        assert_eq!(Failure::from_error(&missing).kind, ErrorKind::NotFound);
        let corrupt = DecodeError::Corrupt {
            format: kinora_decode::Format::Heif,
            detail: String::from("bad box"),
        };
        let failure = Failure::from_error(&corrupt);
        assert_eq!(failure.kind, ErrorKind::Damaged);
        assert_eq!(failure.format, "HEIC");
        assert_eq!(failure.detail, "bad box");
    }

    #[test]
    fn quality_order() {
        assert!(Quality::Thumbnail < Quality::Screen && Quality::Screen < Quality::Full);
    }
}
