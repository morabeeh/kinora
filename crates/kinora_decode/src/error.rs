//! Decode errors. Every way a file can fail ends up here as a value — never as a panic —
//! and the app turns it into a friendly error card.

use crate::format::Format;

/// Why an image could not be shown.
#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    /// Reading the file failed (missing, no permission, disk error).
    #[error("the file could not be read: {0}")]
    Io(#[from] std::io::Error),
    /// The file has no content.
    #[error("the file is empty")]
    Empty,
    /// Not an image format Kinora knows.
    #[error("this is not an image format Kinora can open")]
    UnknownFormat,
    /// The format is known but its decoder is not available (e.g. HEIC without libheif).
    #[error("{0} support is not available on this computer")]
    Unavailable(Format),
    /// Opening it would need more memory than allowed (brief §11, §7.6).
    #[error("the image is too large to open ({width} × {height} pixels)")]
    TooLarge {
        /// Width in pixels.
        width: u64,
        /// Height in pixels.
        height: u64,
    },
    /// The decoder rejected the data (damaged, truncated or unsupported variant).
    #[error("the {format} file is damaged or uses an unsupported feature ({detail})")]
    Corrupt {
        /// Format that was being decoded.
        format: Format,
        /// Technical detail for the log.
        detail: String,
    },
    /// A decoder panicked; the panic was caught (brief §8.3).
    #[error("the {0} decoder failed unexpectedly")]
    Panicked(Format),
}

impl DecodeError {
    pub(crate) fn corrupt(format: Format, detail: impl std::fmt::Display) -> Self {
        Self::Corrupt {
            format,
            detail: detail.to_string(),
        }
    }
}
