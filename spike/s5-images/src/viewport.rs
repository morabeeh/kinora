//! CPU viewport (brief §7.6): crop the visible part of the decoded image and scale it to the
//! exact on-screen pixel size, so the software renderer only has to copy pixels 1:1.

use fast_image_resize as fr;

/// `f64` → pixel count. Rust has no `TryFrom<f64>` for integers, so this is the one audited
/// place that uses `as`: the value is rounded and clamped to `[0, u32::MAX]` first, and Rust
/// defines float→int `as` as saturating (NaN → 0), so the result is always well-defined.
#[allow(
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
pub fn to_px(value: f64) -> u32 {
    value.round().clamp(0.0, f64::from(u32::MAX)) as u32
}

/// What the user is looking at: `zoom = None` means "fit" (never enlarging small images).
#[derive(Debug, Clone, Copy)]
pub struct View {
    pub zoom: Option<f64>,
    /// Image point (in image pixels) at the centre of the window.
    pub center: (f64, f64),
}

/// The crop rectangle (image pixels) and output size (screen pixels) for one frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plan {
    pub zoom: f64,
    pub out_w: u32,
    pub out_h: u32,
    pub left: f64,
    pub top: f64,
    pub crop_w: f64,
    pub crop_h: f64,
}

pub fn fit_zoom(image: (u32, u32), window: (u32, u32)) -> f64 {
    let (iw, ih) = (f64::from(image.0.max(1)), f64::from(image.1.max(1)));
    let (ww, wh) = (f64::from(window.0.max(1)), f64::from(window.1.max(1)));
    (ww / iw).min(wh / ih).min(1.0)
}

pub fn plan(image: (u32, u32), window: (u32, u32), view: &View) -> Plan {
    let (iw, ih) = (f64::from(image.0.max(1)), f64::from(image.1.max(1)));
    let (ww, wh) = (f64::from(window.0.max(1)), f64::from(window.1.max(1)));
    let zoom = view
        .zoom
        .unwrap_or_else(|| fit_zoom(image, window))
        .clamp(0.01, 64.0);
    let out_w = to_px((iw * zoom).min(ww)).max(1);
    let out_h = to_px((ih * zoom).min(wh)).max(1);
    let crop_w = (f64::from(out_w) / zoom).min(iw);
    let crop_h = (f64::from(out_h) / zoom).min(ih);
    let left = (view.center.0 - crop_w / 2.0).clamp(0.0, iw - crop_w);
    let top = (view.center.1 - crop_h / 2.0).clamp(0.0, ih - crop_h);
    Plan {
        zoom,
        out_w,
        out_h,
        left,
        top,
        crop_w,
        crop_h,
    }
}

/// Renders one viewport frame. Downscaling uses a bilinear *convolution* (the kernel widens
/// with the scale factor, so zoomed-out photos don't shimmer); at ≥ 100% we use nearest
/// neighbour so 100% is pixel-exact.
pub fn render(
    resizer: &mut fr::Resizer,
    src: &fr::images::Image<'_>,
    plan: &Plan,
) -> anyhow::Result<fr::images::Image<'static>> {
    let mut dst = fr::images::Image::new(plan.out_w, plan.out_h, fr::PixelType::U8x4);
    let alg = if plan.zoom < 1.0 {
        fr::ResizeAlg::Convolution(fr::FilterType::Bilinear)
    } else {
        fr::ResizeAlg::Nearest
    };
    let options = fr::ResizeOptions::new()
        .crop(plan.left, plan.top, plan.crop_w, plan.crop_h)
        .resize_alg(alg);
    resizer.resize(src, &mut dst, &options)?;
    Ok(dst)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_never_enlarges_small_images() {
        assert!((fit_zoom((800, 600), (1920, 1080)) - 1.0).abs() < f64::EPSILON);
        assert!((fit_zoom((6000, 4000), (1920, 1080)) - 0.27).abs() < 1e-9);
    }

    #[test]
    fn hundred_percent_is_pixel_exact_and_centred() {
        let view = View {
            zoom: Some(1.0),
            center: (3000.0, 2000.0),
        };
        let p = plan((6000, 4000), (1920, 1080), &view);
        assert_eq!((p.out_w, p.out_h), (1920, 1080));
        assert!((p.crop_w - 1920.0).abs() < 1e-9 && (p.crop_h - 1080.0).abs() < 1e-9);
        assert!((p.left - 2040.0).abs() < 1e-9 && (p.top - 1460.0).abs() < 1e-9);
    }

    #[test]
    fn panning_is_clamped_to_the_image() {
        let view = View {
            zoom: Some(1.0),
            center: (-500.0, 99_999.0),
        };
        let p = plan((6000, 4000), (1920, 1080), &view);
        assert!(p.left.abs() < 1e-9);
        assert!((p.top - (4000.0 - 1080.0)).abs() < 1e-9);
    }

    #[test]
    fn to_px_saturates() {
        assert_eq!(to_px(-3.0), 0);
        assert_eq!(to_px(f64::NAN), 0);
        assert_eq!(to_px(1e20), u32::MAX);
        assert_eq!(to_px(2.5), 3);
    }
}
