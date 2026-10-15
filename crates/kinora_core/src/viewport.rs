//! Zoom and pan math for the image canvas (brief §5.4, ADR 0004).
//!
//! Everything here works in **physical pixels**, so "100%" means one image pixel per
//! physical screen pixel on every display scale (HiDPI-correct). The app converts pointer
//! positions from Slint's logical pixels by multiplying with the window's scale factor.
//!
//! The renderer asks [`plan`] which part of the image is visible and how big it is on
//! screen, then crops and scales exactly that (the CPU viewport).

/// A size in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Size {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl Size {
    /// A new size.
    #[must_use]
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    fn w(self) -> f64 {
        f64::from(self.width.max(1))
    }

    fn h(self) -> f64 {
        f64::from(self.height.max(1))
    }
}

/// A rectangle in image pixels (fractional, because zoomed views rarely align to pixels).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    /// Left edge.
    pub left: f64,
    /// Top edge.
    pub top: f64,
    /// Width.
    pub width: f64,
    /// Height.
    pub height: f64,
}

/// The zoom mode.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Zoom {
    /// Fit the image into the window (never enlarging small images unless the setting says
    /// so). Follows window resizes.
    #[default]
    Fit,
    /// A fixed scale: 1.0 = 100% (one image pixel per physical screen pixel).
    Scale(f64),
}

/// What the user is looking at.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct View {
    /// Fit or a fixed scale.
    pub zoom: Zoom,
    /// The image point (in image pixels) shown at the centre of the window.
    pub center: (f64, f64),
}

impl View {
    /// "Fit" view, centred on the image.
    #[must_use]
    pub fn fit(image: Size) -> Self {
        Self {
            zoom: Zoom::Fit,
            center: (image.w() / 2.0, image.h() / 2.0),
        }
    }
}

/// Options that change the fit behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FitOptions {
    /// Setting "Enlarge small images to fit" (default off).
    pub enlarge_small: bool,
}

/// Largest zoom (3200%).
pub const MAX_SCALE: f64 = 32.0;
/// Zoom factor for one mouse-wheel notch or `+`/`-` press.
pub const STEP_FACTOR: f64 = 1.25;

const EPSILON: f64 = 1e-9;

/// What to draw for one frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plan {
    /// Effective scale (1.0 = 100%).
    pub scale: f64,
    /// Size of the image on screen, in physical pixels (never larger than the window).
    pub output: Size,
    /// Where the output goes inside the window (it is centred when smaller).
    pub offset: (u32, u32),
    /// The visible part of the image, in image pixels.
    pub source: Rect,
}

/// The scale that fits `image` into `window`.
#[must_use]
pub fn fit_scale(image: Size, window: Size, options: FitOptions) -> f64 {
    let scale = (window.w() / image.w()).min(window.h() / image.h());
    if options.enlarge_small {
        scale.min(MAX_SCALE)
    } else {
        scale.min(1.0)
    }
}

/// The smallest scale the user can zoom out to: the fit scale (or 100% for small images).
#[must_use]
pub fn min_scale(image: Size, window: Size, options: FitOptions) -> f64 {
    fit_scale(image, window, options).min(1.0)
}

/// The scale `view` resolves to in this window.
#[must_use]
pub fn effective_scale(view: &View, image: Size, window: Size, options: FitOptions) -> f64 {
    match view.zoom {
        Zoom::Fit => fit_scale(image, window, options),
        Zoom::Scale(scale) => clamp_scale(scale, image, window, options),
    }
}

fn clamp_scale(scale: f64, image: Size, window: Size, options: FitOptions) -> f64 {
    let min = min_scale(image, window, options);
    if scale.is_finite() {
        scale.clamp(min, MAX_SCALE.max(min))
    } else {
        min
    }
}

/// Works out the visible part of the image and its on-screen size.
#[must_use]
pub fn plan(image: Size, window: Size, view: &View, options: FitOptions) -> Plan {
    let scale = effective_scale(view, image, window, options);
    let out_w = to_px((image.w() * scale).min(window.w())).max(1);
    let out_h = to_px((image.h() * scale).min(window.h())).max(1);
    let src_w = (f64::from(out_w) / scale).min(image.w());
    let src_h = (f64::from(out_h) / scale).min(image.h());
    let (cx, cy) = clamp_center(view.center, image, (src_w, src_h));
    Plan {
        scale,
        output: Size::new(out_w, out_h),
        offset: (
            window.width.saturating_sub(out_w) / 2,
            window.height.saturating_sub(out_h) / 2,
        ),
        source: Rect {
            left: cx - src_w / 2.0,
            top: cy - src_h / 2.0,
            width: src_w,
            height: src_h,
        },
    }
}

/// Keeps the centre where the visible part stays inside the image.
fn clamp_center(center: (f64, f64), image: Size, visible: (f64, f64)) -> (f64, f64) {
    let axis = |c: f64, len: f64, vis: f64| {
        let half = vis / 2.0;
        if c.is_finite() {
            c.clamp(half, (len - half).max(half))
        } else {
            len / 2.0
        }
    };
    (
        axis(center.0, image.w(), visible.0),
        axis(center.1, image.h(), visible.1),
    )
}

/// Normalises a view after a change: clamps the scale and the centre.
#[must_use]
pub fn clamp_view(view: &View, image: Size, window: Size, options: FitOptions) -> View {
    let p = plan(image, window, view, options);
    View {
        zoom: view.zoom,
        center: (
            p.source.left + p.source.width / 2.0,
            p.source.top + p.source.height / 2.0,
        ),
    }
}

/// Sets the scale while keeping the image point under `anchor` (a window position in
/// physical pixels; `None` = window centre) where it is — "zoom at the cursor".
#[must_use]
pub fn zoom_to(
    view: &View,
    image: Size,
    window: Size,
    options: FitOptions,
    new_scale: f64,
    anchor: Option<(f64, f64)>,
) -> View {
    let before = plan(image, window, view, options);
    let (ax, ay) = anchor.unwrap_or((window.w() / 2.0, window.h() / 2.0));
    // Image point under the anchor before zooming.
    let ux = before.source.left + (ax - f64::from(before.offset.0)) / before.scale;
    let uy = before.source.top + (ay - f64::from(before.offset.1)) / before.scale;
    let scale = clamp_scale(new_scale, image, window, options);
    let fit = fit_scale(image, window, options);
    let zoom = if (scale - fit).abs() < EPSILON {
        Zoom::Fit
    } else {
        Zoom::Scale(scale)
    };
    let next = View {
        zoom,
        center: (
            ux - (ax - window.w() / 2.0) / scale,
            uy - (ay - window.h() / 2.0) / scale,
        ),
    };
    clamp_view(&next, image, window, options)
}

/// Zooms in (`steps > 0`) or out (`steps < 0`) by [`STEP_FACTOR`] per step, anchored at
/// `anchor`. Fractional steps (touchpad pinch) work too. Zooming across 100% or the fit
/// scale stops there first, so those levels are easy to hit.
#[must_use]
pub fn zoom_by_steps(
    view: &View,
    image: Size,
    window: Size,
    options: FitOptions,
    steps: f64,
    anchor: Option<(f64, f64)>,
) -> View {
    if steps == 0.0 || !steps.is_finite() {
        return *view;
    }
    let current = effective_scale(view, image, window, options);
    let mut target = current * STEP_FACTOR.powf(steps);
    let fit = fit_scale(image, window, options);
    for stop in [1.0, fit] {
        let crosses = (current < stop - EPSILON && target > stop + EPSILON)
            || (current > stop + EPSILON && target < stop - EPSILON);
        if crosses {
            target = stop;
        }
    }
    zoom_to(view, image, window, options, target, anchor)
}

/// Double-click behaviour: fit → 100% at the clicked point, anything else → fit.
#[must_use]
pub fn toggle_fit_actual(
    view: &View,
    image: Size,
    window: Size,
    options: FitOptions,
    anchor: Option<(f64, f64)>,
) -> View {
    let scale = effective_scale(view, image, window, options);
    let fit = fit_scale(image, window, options);
    if (scale - fit).abs() < EPSILON {
        zoom_to(view, image, window, options, 1.0, anchor)
    } else {
        View::fit(image)
    }
}

/// Moves the view by a drag of (`dx`, `dy`) physical pixels (dragging right shows more of
/// the left side, like grabbing paper). Does nothing when the whole image is visible.
#[must_use]
pub fn pan_by(
    view: &View,
    image: Size,
    window: Size,
    options: FitOptions,
    delta: (f64, f64),
) -> View {
    let p = plan(image, window, view, options);
    if !can_pan(&p, image) {
        return *view;
    }
    let base = clamp_view(view, image, window, options);
    let next = View {
        zoom: Zoom::Scale(p.scale),
        center: (
            base.center.0 - delta.0 / p.scale,
            base.center.1 - delta.1 / p.scale,
        ),
    };
    clamp_view(&next, image, window, options)
}

/// `true` when part of the image is outside the window (so arrow keys and drags pan).
#[must_use]
pub fn can_pan(plan: &Plan, image: Size) -> bool {
    plan.source.width < image.w() - EPSILON || plan.source.height < image.h() - EPSILON
}

/// Converts a non-negative pixel measure to `u32`, rounding and saturating
/// (NaN and negatives → 0).
#[must_use]
pub fn to_px(value: f64) -> u32 {
    // Rust has no `TryFrom<f64>` for integers, so this is the one audited place that uses
    // `as` for pixel math: the value is rounded and clamped to `0..=u32::MAX` first, and
    // float→int `as` is defined as saturating (NaN → 0), so the result is always correct.
    #[allow(
        clippy::as_conversions,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    let px = value.round().clamp(0.0, f64::from(u32::MAX)) as u32;
    px
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const PHOTO: Size = Size::new(6000, 4000);
    const SCREEN: Size = Size::new(1920, 1080);
    const OPTS: FitOptions = FitOptions {
        enlarge_small: false,
    };

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn fit_never_enlarges_small_images_by_default() {
        assert!(close(fit_scale(Size::new(800, 600), SCREEN, OPTS), 1.0));
        let enlarge = FitOptions {
            enlarge_small: true,
        };
        assert!(close(fit_scale(Size::new(960, 540), SCREEN, enlarge), 2.0));
        assert!(close(fit_scale(PHOTO, SCREEN, OPTS), 0.27));
    }

    #[test]
    fn fit_plan_centres_the_image() {
        let p = plan(PHOTO, SCREEN, &View::fit(PHOTO), OPTS);
        assert_eq!(p.output, Size::new(1620, 1080));
        assert_eq!(p.offset, (150, 0));
        assert!(close(p.source.width, 6000.0) && close(p.source.left, 0.0));
    }

    #[test]
    fn hundred_percent_is_one_image_pixel_per_screen_pixel() {
        let view = View {
            zoom: Zoom::Scale(1.0),
            center: (3000.0, 2000.0),
        };
        let p = plan(PHOTO, SCREEN, &view, OPTS);
        assert_eq!(p.output, SCREEN);
        assert!(close(p.source.width, 1920.0) && close(p.source.height, 1080.0));
        assert!(close(p.source.left, 2040.0) && close(p.source.top, 1460.0));
    }

    #[test]
    fn centre_is_clamped_to_the_image() {
        let view = View {
            zoom: Zoom::Scale(1.0),
            center: (-500.0, 99_999.0),
        };
        let p = plan(PHOTO, SCREEN, &view, OPTS);
        assert!(close(p.source.left, 0.0));
        assert!(close(p.source.top, 4000.0 - 1080.0));
    }

    #[test]
    fn zoom_at_cursor_keeps_the_point_under_the_cursor() {
        let fit = View::fit(PHOTO);
        let cursor = (400.0, 300.0);
        let before = plan(PHOTO, SCREEN, &fit, OPTS);
        let under = (
            before.source.left + (cursor.0 - f64::from(before.offset.0)) / before.scale,
            before.source.top + (cursor.1 - f64::from(before.offset.1)) / before.scale,
        );
        let zoomed = zoom_to(&fit, PHOTO, SCREEN, OPTS, 1.0, Some(cursor));
        let after = plan(PHOTO, SCREEN, &zoomed, OPTS);
        let under_after = (
            after.source.left + (cursor.0 - f64::from(after.offset.0)) / after.scale,
            after.source.top + (cursor.1 - f64::from(after.offset.1)) / after.scale,
        );
        assert!(close(under.0, under_after.0) && close(under.1, under_after.1));
    }

    #[test]
    fn steps_stop_at_100_percent_and_fit() {
        let near = View {
            zoom: Zoom::Scale(0.9),
            center: (3000.0, 2000.0),
        };
        let zoomed = zoom_by_steps(&near, PHOTO, SCREEN, OPTS, 1.0, None);
        assert_eq!(zoomed.zoom, Zoom::Scale(1.0));
        let out = zoom_by_steps(&near, PHOTO, SCREEN, OPTS, -10.0, None);
        assert_eq!(out.zoom, Zoom::Fit, "cannot zoom out past fit");
        // A big jump from below 100% stops at 100% first…
        let big = zoom_by_steps(&near, PHOTO, SCREEN, OPTS, 100.0, None);
        assert_eq!(big.zoom, Zoom::Scale(1.0));
        // …and from there it is capped at the maximum.
        let max = zoom_by_steps(&big, PHOTO, SCREEN, OPTS, 100.0, None);
        assert_eq!(max.zoom, Zoom::Scale(MAX_SCALE));
    }

    #[test]
    fn double_click_toggles_fit_and_actual_size() {
        let fit = View::fit(PHOTO);
        let actual = toggle_fit_actual(&fit, PHOTO, SCREEN, OPTS, Some((960.0, 540.0)));
        assert_eq!(actual.zoom, Zoom::Scale(1.0));
        let back = toggle_fit_actual(&actual, PHOTO, SCREEN, OPTS, None);
        assert_eq!(back.zoom, Zoom::Fit);
    }

    #[test]
    fn panning_only_when_zoomed_in() {
        let fit = View::fit(PHOTO);
        assert_eq!(pan_by(&fit, PHOTO, SCREEN, OPTS, (100.0, 0.0)), fit);
        let actual = View {
            zoom: Zoom::Scale(1.0),
            center: (3000.0, 2000.0),
        };
        let moved = pan_by(&actual, PHOTO, SCREEN, OPTS, (100.0, -50.0));
        assert!(close(moved.center.0, 2900.0) && close(moved.center.1, 2050.0));
        // Overshooting an edge doesn't leave a "dead zone" when dragging back.
        let far = pan_by(&actual, PHOTO, SCREEN, OPTS, (1e6, 0.0));
        let back = pan_by(&far, PHOTO, SCREEN, OPTS, (-10.0, 0.0));
        assert!(close(back.center.0, 960.0 + 10.0));
    }

    #[test]
    fn small_image_in_big_window_is_centred_at_100_percent() {
        let small = Size::new(400, 300);
        let p = plan(small, SCREEN, &View::fit(small), OPTS);
        assert_eq!(p.output, small);
        assert_eq!(p.offset, (760, 390));
        assert!(!can_pan(&p, small));
    }

    #[test]
    fn degenerate_sizes_do_not_break() {
        let p = plan(Size::new(0, 0), Size::new(0, 0), &View::default(), OPTS);
        assert_eq!(p.output, Size::new(1, 1));
        let nan = View {
            zoom: Zoom::Scale(f64::NAN),
            center: (f64::NAN, f64::INFINITY),
        };
        let p = plan(PHOTO, SCREEN, &nan, OPTS);
        assert!(p.scale.is_finite() && p.source.left.is_finite());
    }

    #[test]
    fn to_px_saturates() {
        assert_eq!(to_px(-3.0), 0);
        assert_eq!(to_px(f64::NAN), 0);
        assert_eq!(to_px(1e20), u32::MAX);
        assert_eq!(to_px(2.5), 3);
    }

    proptest! {
        #[test]
        fn visible_part_always_stays_inside_the_image(
            iw in 1u32..20_000, ih in 1u32..20_000,
            ww in 1u32..5_000, wh in 1u32..5_000,
            scale in 0.001f64..64.0,
            cx in -1e5f64..1e5, cy in -1e5f64..1e5,
        ) {
            let image = Size::new(iw, ih);
            let window = Size::new(ww, wh);
            let view = View { zoom: Zoom::Scale(scale), center: (cx, cy) };
            let p = plan(image, window, &view, OPTS);
            prop_assert!(p.output.width <= ww.max(1) && p.output.height <= wh.max(1));
            prop_assert!(p.source.left >= -1e-6 && p.source.top >= -1e-6);
            prop_assert!(p.source.left + p.source.width <= f64::from(iw) + 1e-6);
            prop_assert!(p.source.top + p.source.height <= f64::from(ih) + 1e-6);
            prop_assert!(p.offset.0 + p.output.width <= ww.max(1));
        }

        #[test]
        fn zoom_in_then_out_returns_to_the_same_scale(steps in 0.1f64..5.0) {
            let view = View { zoom: Zoom::Scale(2.0), center: (3000.0, 2000.0) };
            let zoomed = zoom_by_steps(&view, PHOTO, SCREEN, OPTS, steps, Some((500.0, 500.0)));
            let back = zoom_by_steps(&zoomed, PHOTO, SCREEN, OPTS, -steps, Some((500.0, 500.0)));
            let s = effective_scale(&back, PHOTO, SCREEN, OPTS);
            // Stops at 100%/fit can make the round trip land on a stop; otherwise exact.
            prop_assert!((s - 2.0).abs() < 1e-6 || (s - 1.0).abs() < 1e-6 || s >= 2.0);
        }
    }
}
