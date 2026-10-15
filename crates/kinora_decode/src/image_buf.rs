//! The decoded pixel buffer type shared by every decoder.

use std::time::Duration;

use kinora_core::viewport::Size;

use crate::format::Format;

/// An 8-bit RGBA image with straight (not premultiplied) alpha, rows top to bottom, no
/// padding. The invariant `data.len() == width * height * 4` always holds (the default is
/// an empty 0 × 0 image).
#[derive(Clone, PartialEq, Eq, Default)]
pub struct Rgba8Image {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

impl std::fmt::Debug for Rgba8Image {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Rgba8Image")
            .field("width", &self.width)
            .field("height", &self.height)
            .finish_non_exhaustive()
    }
}

impl Rgba8Image {
    /// Wraps a pixel buffer; `None` if its length doesn't match the size.
    #[must_use]
    pub fn new(width: u32, height: u32, data: Vec<u8>) -> Option<Self> {
        (byte_len(width, height)? == data.len()).then_some(Self {
            width,
            height,
            data,
        })
    }

    /// A buffer filled with one colour.
    #[must_use]
    pub fn filled(width: u32, height: u32, rgba: [u8; 4]) -> Option<Self> {
        let len = byte_len(width, height)?;
        let data = rgba.iter().copied().cycle().take(len).collect();
        Some(Self {
            width,
            height,
            data,
        })
    }

    /// Width in pixels.
    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in pixels.
    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Size in pixels.
    #[must_use]
    pub fn size(&self) -> Size {
        Size::new(self.width, self.height)
    }

    /// The RGBA bytes.
    #[must_use]
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Mutable RGBA bytes (the length can't change).
    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    /// Takes the bytes out.
    #[must_use]
    pub fn into_data(self) -> Vec<u8> {
        self.data
    }

    /// Bytes used by the pixels.
    #[must_use]
    pub fn byte_size(&self) -> u64 {
        u64::try_from(self.data.len()).unwrap_or(u64::MAX)
    }

    /// The RGBA value at (x, y), if inside the image.
    #[must_use]
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let index = (usize::try_from(y).ok()? * usize::try_from(self.width).ok()?
            + usize::try_from(x).ok()?)
            * 4;
        let px = self.data.get(index..index + 4)?;
        px.try_into().ok()
    }

    /// `true` if any pixel is not fully opaque.
    #[must_use]
    pub fn has_transparency(&self) -> bool {
        self.data
            .as_chunks::<4>()
            .0
            .iter()
            .any(|px| px[3] != u8::MAX)
    }
}

/// `width * height * 4` as `usize`, if it fits.
pub(crate) fn byte_len(width: u32, height: u32) -> Option<usize> {
    usize::try_from(width)
        .ok()?
        .checked_mul(usize::try_from(height).ok()?)?
        .checked_mul(4)
}

/// One frame of a (possibly animated) image.
#[derive(Debug, Clone)]
pub struct Frame {
    /// The full composited picture for this frame.
    pub image: Rgba8Image,
    /// How long to show it (zero for still images).
    pub delay: Duration,
}

/// A decoded picture, ready for the viewport renderer.
#[derive(Debug, Clone)]
pub struct DecodedImage {
    first: Frame,
    rest: Vec<Frame>,
    /// Size of the *full-resolution* picture after EXIF orientation. `first().size()` can
    /// be smaller when a screen-sized version was decoded (scaled JPEG, downscaled RAW-size
    /// photos); zoom percentages are always relative to this size.
    pub source_size: Size,
    /// The detected format.
    pub format: Format,
    /// Whether the picture has transparent parts (a checkerboard is drawn behind them).
    pub has_alpha: bool,
    /// An animation was cut short (too many frames, too much memory, or damaged data).
    pub animation_truncated: bool,
}

impl DecodedImage {
    /// A still image.
    #[must_use]
    pub fn still(image: Rgba8Image, source_size: Size, format: Format, has_alpha: bool) -> Self {
        Self {
            first: Frame {
                image,
                delay: Duration::ZERO,
            },
            rest: Vec::new(),
            source_size,
            format,
            has_alpha,
            animation_truncated: false,
        }
    }

    /// An animation (`frames` must not be empty; returns `None` otherwise).
    #[must_use]
    pub fn animated(frames: Vec<Frame>, format: Format, has_alpha: bool) -> Option<Self> {
        let mut frames = frames.into_iter();
        let first = frames.next()?;
        let source_size = first.image.size();
        Some(Self {
            first,
            rest: frames.collect(),
            source_size,
            format,
            has_alpha,
            animation_truncated: false,
        })
    }

    /// The first (or only) frame's picture.
    #[must_use]
    pub fn first(&self) -> &Rgba8Image {
        &self.first.image
    }

    /// Frame `index`, if it exists.
    #[must_use]
    pub fn frame(&self, index: usize) -> Option<&Frame> {
        match index.checked_sub(1) {
            None => Some(&self.first),
            Some(rest_index) => self.rest.get(rest_index),
        }
    }

    /// Number of frames (1 for still images).
    #[must_use]
    pub fn frame_count(&self) -> usize {
        self.rest.len() + 1
    }

    /// `true` for animations with more than one frame.
    #[must_use]
    pub fn is_animated(&self) -> bool {
        !self.rest.is_empty()
    }

    /// `true` when the pixels are the full-resolution picture (not a screen-sized version).
    #[must_use]
    pub fn is_full_resolution(&self) -> bool {
        self.first.image.width() >= self.source_size.width
            && self.first.image.height() >= self.source_size.height
    }

    /// Memory used by all frames, for the byte-budgeted cache.
    #[must_use]
    pub fn byte_size(&self) -> u64 {
        self.rest
            .iter()
            .map(|frame| frame.image.byte_size())
            .fold(self.first.image.byte_size(), u64::saturating_add)
    }

    /// Replaces the first frame's picture (used after orientation/ICC/downscale steps).
    pub(crate) fn map_first(mut self, f: impl FnOnce(Rgba8Image) -> Rgba8Image) -> Self {
        let image = std::mem::take(&mut self.first.image);
        self.first.image = f(image);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_length_must_match() {
        assert!(Rgba8Image::new(2, 2, vec![0; 16]).is_some());
        assert!(Rgba8Image::new(2, 2, vec![0; 15]).is_none());
        assert!(Rgba8Image::new(u32::MAX, u32::MAX, Vec::new()).is_none());
    }

    #[test]
    fn pixels_and_transparency() {
        let mut img = Rgba8Image::filled(3, 2, [10, 20, 30, 255]).unwrap();
        assert_eq!(img.pixel(2, 1), Some([10, 20, 30, 255]));
        assert_eq!(img.pixel(3, 0), None);
        assert!(!img.has_transparency());
        if let Some(alpha) = img.data_mut().get_mut(3) {
            *alpha = 0;
        }
        assert!(img.has_transparency());
    }

    #[test]
    fn frames() {
        let frame = |v: u8| Frame {
            image: Rgba8Image::filled(2, 2, [v; 4]).unwrap(),
            delay: Duration::from_millis(100),
        };
        assert!(DecodedImage::animated(Vec::new(), Format::Gif, false).is_none());
        let anim = DecodedImage::animated(vec![frame(1), frame(2)], Format::Gif, true).unwrap();
        assert_eq!(anim.frame_count(), 2);
        assert!(anim.is_animated());
        assert_eq!(anim.frame(1).unwrap().image.pixel(0, 0), Some([2; 4]));
        assert!(anim.frame(2).is_none());
        assert_eq!(anim.byte_size(), 32);
        assert!(anim.is_full_resolution());
    }
}
