//! The CPU viewport (brief §7.6, ADR 0004): crop the visible part of the decoded picture
//! and scale it to the exact on-screen size, so every UI renderer only copies pixels 1:1.
//! Fit and 100% therefore look identical with the GPU and the software renderer.

use std::sync::Arc;

use fast_image_resize as fr;
use kinora_core::viewport::{Plan, Size};

use crate::image_buf::Rgba8Image;
use crate::resize::view;

/// What to draw behind transparent pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Background {
    /// Light/dark squares of `cell` physical pixels.
    Checkerboard {
        /// Square size in physical pixels.
        cell: u32,
        /// Light square colour.
        light: [u8; 3],
        /// Dark square colour.
        dark: [u8; 3],
    },
    /// One solid colour.
    Solid([u8; 3]),
}

impl Background {
    /// The default checkerboard, scaled for the display (8 logical px squares).
    #[must_use]
    pub fn checkerboard(scale_factor: f32) -> Self {
        let cell = kinora_core::viewport::to_px(f64::from(scale_factor) * 8.0).max(1);
        Self::Checkerboard {
            cell,
            light: [0x99, 0x99, 0x99],
            dark: [0x66, 0x66, 0x66],
        }
    }
}

/// Why a frame could not be rendered (should not happen with valid plans).
#[derive(Debug, thiserror::Error)]
#[error("viewport rendering failed: {0}")]
pub struct RenderError(String);

/// Renders viewport frames; keeps scratch buffers between frames. One per thread.
#[derive(Debug, Default)]
pub struct ViewportRenderer {
    resizer: fr::Resizer,
}

impl ViewportRenderer {
    /// A new renderer.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Renders `plan` from `source`.
    ///
    /// `source` may be smaller than the full picture (a screen-sized decode or a pyramid
    /// level); `source_size` is the full size and the plan's rectangle is in full-size
    /// coordinates. `alpha_background` is `Some` when the picture has transparency.
    ///
    /// # Errors
    /// [`RenderError`] if the resizer rejects the request (empty sizes).
    pub fn render(
        &mut self,
        source: &Rgba8Image,
        source_size: Size,
        plan: &Plan,
        alpha_background: Option<Background>,
    ) -> Result<Rgba8Image, RenderError> {
        let fx = f64::from(source.width()) / f64::from(source_size.width.max(1));
        let fy = f64::from(source.height()) / f64::from(source_size.height.max(1));
        let (bw, bh) = (f64::from(source.width()), f64::from(source.height()));
        let width = (plan.source.width * fx).clamp(1e-3, bw);
        let height = (plan.source.height * fy).clamp(1e-3, bh);
        let left = (plan.source.left * fx).clamp(0.0, bw - width);
        let top = (plan.source.top * fy).clamp(0.0, bh - height);

        let out = plan.output;
        let ratio = f64::from(out.width) / width;
        // Exactly 1:1 (e.g. 100% of a full-size picture) is a plain copy; at ≥ 300% of a
        // full-size picture we show crisp pixels for inspection.
        let one_to_one = (ratio - 1.0).abs() < 1e-6;
        let inspecting = ratio > 1.0 && plan.scale >= 3.0 && fx >= 1.0 - 1e-9;
        let algorithm = if one_to_one || inspecting {
            fr::ResizeAlg::Nearest
        } else {
            // Downscaling: the convolution kernel widens with the scale factor, so zoomed-out
            // photos don't shimmer. Upscaling: plain bilinear interpolation.
            fr::ResizeAlg::Convolution(fr::FilterType::Bilinear)
        };
        let src = view(source).ok_or_else(|| RenderError(String::from("bad source")))?;
        let mut dst = fr::images::Image::new(out.width, out.height, fr::PixelType::U8x4);
        let options = fr::ResizeOptions::new()
            .crop(left, top, width, height)
            .resize_alg(algorithm)
            .use_alpha(alpha_background.is_some());
        self.resizer
            .resize(&src, &mut dst, &options)
            .map_err(|err| RenderError(err.to_string()))?;
        let mut frame = Rgba8Image::new(out.width, out.height, dst.into_vec())
            .ok_or_else(|| RenderError(String::from("size mismatch")))?;
        if let Some(background) = alpha_background {
            composite(&mut frame, background);
        }
        Ok(frame)
    }
}

/// Blends every pixel over `background`, leaving an opaque frame.
fn composite(frame: &mut Rgba8Image, background: Background) {
    let width = usize::try_from(frame.width()).unwrap_or(0).max(1);
    for (index, px) in frame
        .data_mut()
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .enumerate()
    {
        let alpha = px[3];
        if alpha == u8::MAX {
            continue;
        }
        let behind = match background {
            Background::Solid(color) => color,
            Background::Checkerboard { cell, light, dark } => {
                let cell = usize::try_from(cell).unwrap_or(8).max(1);
                let (column, row) = (index % width, index / width);
                if (column / cell + row / cell) % 2 == 0 {
                    light
                } else {
                    dark
                }
            }
        };
        let weight = u16::from(alpha);
        for (channel, under) in px.iter_mut().zip(behind) {
            let value =
                (u16::from(*channel) * weight + u16::from(under) * (255 - weight) + 127) / 255;
            *channel = u8::try_from(value).unwrap_or(u8::MAX);
        }
        px[3] = u8::MAX;
    }
}

/// Pre-scaled halves of a picture (½, ¼, …), built on demand. Zoomed-out views of huge
/// pictures resample from the nearest level instead of the full picture, which is what
/// makes "fit" fast on slow CPUs (ADR 0004, perf.md S7).
#[derive(Debug, Clone)]
pub struct Pyramid {
    levels: Vec<Arc<Rgba8Image>>,
    has_alpha: bool,
}

impl Pyramid {
    /// A pyramid whose level 0 is `base`.
    #[must_use]
    pub fn new(base: Arc<Rgba8Image>, has_alpha: bool) -> Self {
        Self {
            levels: vec![base],
            has_alpha,
        }
    }

    /// Level 0.
    #[must_use]
    pub fn base(&self) -> Option<&Arc<Rgba8Image>> {
        self.levels.first()
    }

    /// Bytes used by the extra levels (level 0 is owned elsewhere).
    #[must_use]
    pub fn extra_bytes(&self) -> u64 {
        self.levels
            .iter()
            .skip(1)
            .map(|level| level.byte_size())
            .fold(0, u64::saturating_add)
    }

    /// The smallest level that still has at least `scale` pixels per full-size pixel, so
    /// drawing at `scale` only ever shrinks it. Missing levels are built (each is half the
    /// previous one).
    pub fn level_for(
        &mut self,
        renderer: &mut ViewportRenderer,
        source_size: Size,
        scale: f64,
    ) -> Option<Arc<Rgba8Image>> {
        let ratio =
            |level: &Rgba8Image| f64::from(level.width()) / f64::from(source_size.width.max(1));
        loop {
            let last = self.levels.last()?;
            if ratio(last) / 2.0 < scale || last.width() < 128 || last.height() < 128 {
                break;
            }
            let (w, h) = (last.width().div_ceil(2), last.height().div_ceil(2));
            let half = crate::resize::resize_to(&mut renderer.resizer, last, w, h, self.has_alpha)?;
            self.levels.push(Arc::new(half));
        }
        self.levels
            .iter()
            .rev()
            .find(|level| ratio(level) >= scale)
            .or_else(|| self.levels.first())
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kinora_core::viewport::{FitOptions, View, Zoom, plan};

    fn gradient(width: u32, height: u32) -> Rgba8Image {
        let data = (0..height)
            .flat_map(|y| {
                (0..width).flat_map(move |x| {
                    [
                        u8::try_from(x % 256).unwrap(),
                        u8::try_from(y % 256).unwrap(),
                        0,
                        255,
                    ]
                })
            })
            .collect();
        Rgba8Image::new(width, height, data).unwrap()
    }

    #[test]
    fn hundred_percent_is_an_exact_copy() {
        let img = gradient(300, 200);
        let view = View {
            zoom: Zoom::Scale(1.0),
            center: (150.0, 100.0),
        };
        let p = plan(img.size(), Size::new(100, 50), &view, FitOptions::default());
        let frame = ViewportRenderer::new()
            .render(&img, img.size(), &p, None)
            .unwrap();
        assert_eq!(frame.size(), Size::new(100, 50));
        // Window pixel (0, 0) shows image pixel (100, 75).
        assert_eq!(frame.pixel(0, 0), img.pixel(100, 75));
        assert_eq!(frame.pixel(99, 49), img.pixel(199, 124));
    }

    #[test]
    fn screen_sized_source_maps_to_full_size_coordinates() {
        // A half-size decode of a 400×200 picture, viewed at fit in a 100×50 window.
        let half = gradient(200, 100);
        let full = Size::new(400, 200);
        let p = plan(
            full,
            Size::new(100, 50),
            &View::fit(full),
            FitOptions::default(),
        );
        let frame = ViewportRenderer::new()
            .render(&half, full, &p, None)
            .unwrap();
        assert_eq!(frame.size(), Size::new(100, 50));
    }

    #[test]
    fn transparent_pixels_get_the_checkerboard() {
        let img = Rgba8Image::filled(16, 16, [255, 0, 0, 0]).unwrap();
        let p = plan(
            img.size(),
            Size::new(16, 16),
            &View::fit(img.size()),
            FitOptions::default(),
        );
        let bg = Background::Checkerboard {
            cell: 8,
            light: [200, 200, 200],
            dark: [50, 50, 50],
        };
        let frame = ViewportRenderer::new()
            .render(&img, img.size(), &p, Some(bg))
            .unwrap();
        assert_eq!(frame.pixel(0, 0), Some([200, 200, 200, 255]));
        assert_eq!(frame.pixel(8, 0), Some([50, 50, 50, 255]));
        assert_eq!(frame.pixel(8, 8), Some([200, 200, 200, 255]));
    }

    #[test]
    fn pyramid_picks_the_smallest_level_above_the_scale() {
        let mut renderer = ViewportRenderer::new();
        let base = Arc::new(gradient(1024, 512));
        let mut pyramid = Pyramid::new(Arc::clone(&base), false);
        let full = base.size();
        assert_eq!(
            pyramid.level_for(&mut renderer, full, 1.0).unwrap().width(),
            1024
        );
        assert_eq!(pyramid.extra_bytes(), 0, "nothing built for 100%");
        let level = pyramid.level_for(&mut renderer, full, 0.3).unwrap();
        assert_eq!(level.width(), 512, "½ ≥ 0.3 > ¼");
        let level = pyramid.level_for(&mut renderer, full, 0.1).unwrap();
        assert_eq!(level.width(), 128);
        assert!(pyramid.extra_bytes() > 0);
    }
}
