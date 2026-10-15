//! Fast, good-quality resizing with `fast_image_resize` (SIMD picked at run time, so no
//! AVX2 requirement — brief §4.1).

use fast_image_resize as fr;
use kinora_core::viewport::{Size, to_px};

use crate::image_buf::Rgba8Image;

/// A borrowed view of an [`Rgba8Image`] for `fast_image_resize`.
pub(crate) fn view(image: &Rgba8Image) -> Option<fr::images::ImageRef<'_>> {
    fr::images::ImageRef::new(
        image.width(),
        image.height(),
        image.data(),
        fr::PixelType::U8x4,
    )
    .ok()
}

/// Resizes the whole image to exactly `width` × `height` with a Catmull-Rom filter
/// (sharp, no ringing worth noticing). `None` only for empty sizes.
#[must_use]
pub fn resize_to(
    resizer: &mut fr::Resizer,
    image: &Rgba8Image,
    width: u32,
    height: u32,
    has_alpha: bool,
) -> Option<Rgba8Image> {
    if width == 0 || height == 0 {
        return None;
    }
    let src = view(image)?;
    let mut dst = fr::images::Image::new(width, height, fr::PixelType::U8x4);
    let options = fr::ResizeOptions::new()
        .resize_alg(fr::ResizeAlg::Convolution(fr::FilterType::CatmullRom))
        .use_alpha(has_alpha);
    resizer.resize(&src, &mut dst, &options).ok()?;
    Rgba8Image::new(width, height, dst.into_vec())
}

/// The largest size with the aspect ratio of `source` that fits in `bounds` (never larger
/// than `source` itself).
#[must_use]
pub fn fit_size(source: Size, bounds: Size) -> Size {
    let sw = f64::from(source.width.max(1));
    let sh = f64::from(source.height.max(1));
    let scale = (f64::from(bounds.width.max(1)) / sw)
        .min(f64::from(bounds.height.max(1)) / sh)
        .min(1.0);
    Size::new(to_px(sw * scale).max(1), to_px(sh * scale).max(1))
}

/// Shrinks `image` to fit in `bounds`; returns it unchanged if it already fits.
#[must_use]
pub fn shrink_to_fit(
    resizer: &mut fr::Resizer,
    image: Rgba8Image,
    bounds: Size,
    has_alpha: bool,
) -> Rgba8Image {
    let target = fit_size(image.size(), bounds);
    if target == image.size() {
        return image;
    }
    resize_to(resizer, &image, target.width, target.height, has_alpha).unwrap_or(image)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_size_keeps_aspect_and_never_enlarges() {
        assert_eq!(
            fit_size(Size::new(6000, 4000), Size::new(1920, 1080)),
            Size::new(1620, 1080)
        );
        assert_eq!(
            fit_size(Size::new(400, 300), Size::new(1920, 1080)),
            Size::new(400, 300)
        );
        assert_eq!(fit_size(Size::new(0, 0), Size::new(0, 0)), Size::new(1, 1));
    }

    #[test]
    fn shrinking_averages_colours() {
        let mut resizer = fr::Resizer::new();
        // 2×1 black/white image shrunk to 1×1 → mid grey.
        let img = Rgba8Image::new(2, 1, vec![0, 0, 0, 255, 255, 255, 255, 255]).unwrap();
        let out = shrink_to_fit(&mut resizer, img, Size::new(1, 1), false);
        let [r, _, _, a] = out.pixel(0, 0).unwrap();
        assert!((100..=155).contains(&r), "{r}");
        assert_eq!(a, 255);
    }
}
