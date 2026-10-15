//! The viewport render thread (brief §7.5: the UI thread never waits for pixel work).
//!
//! The UI posts the newest wanted frame into a one-slot mailbox; the thread always renders
//! the latest request and skips the ones that were overtaken (fast panning or resizing just
//! drops intermediate frames). Finished frames go back to the UI thread.

use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use kinora_core::viewport::{Plan, Rect, Size, to_px};
use kinora_decode::viewport::{Background, Pyramid, ViewportRenderer};
use kinora_decode::{DecodedImage, Rgba8Image};
use slint::{Rgba8Pixel, SharedPixelBuffer};

/// A frame the UI wants drawn.
#[derive(Debug, Clone)]
pub struct RenderRequest {
    /// Increases with every request; the UI ignores results older than what it shows.
    pub seq: u64,
    pub image: Arc<DecodedImage>,
    pub frame: usize,
    pub plan: Plan,
    pub background: Option<Background>,
    /// Window size in physical pixels (for the scaled-image cache limit).
    pub window: Size,
}

/// A finished frame.
#[derive(Debug)]
pub struct Rendered {
    pub seq: u64,
    pub buffer: SharedPixelBuffer<Rgba8Pixel>,
    pub plan: Plan,
    pub elapsed: Duration,
}

struct Mailbox {
    request: Option<RenderRequest>,
    stop: bool,
}

struct Shared {
    mailbox: Mutex<Mailbox>,
    wake: Condvar,
}

/// Owner of the render thread; dropping it stops the thread.
pub struct RenderThread {
    shared: Arc<Shared>,
    handle: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for RenderThread {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenderThread").finish_non_exhaustive()
    }
}

impl RenderThread {
    pub fn start(deliver: impl Fn(Rendered) + Send + 'static) -> std::io::Result<Self> {
        let shared = Arc::new(Shared {
            mailbox: Mutex::new(Mailbox {
                request: None,
                stop: false,
            }),
            wake: Condvar::new(),
        });
        let thread_shared = Arc::clone(&shared);
        let handle = std::thread::Builder::new()
            .name(String::from("viewport"))
            .spawn(move || run(&thread_shared, &deliver))?;
        Ok(Self {
            shared,
            handle: Some(handle),
        })
    }

    /// Replaces any not-yet-started request with this one.
    pub fn submit(&self, request: RenderRequest) {
        let mut mailbox = self
            .shared
            .mailbox
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        mailbox.request = Some(request);
        drop(mailbox);
        self.shared.wake.notify_one();
    }
}

impl Drop for RenderThread {
    fn drop(&mut self) {
        self.shared
            .mailbox
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .stop = true;
        self.shared.wake.notify_one();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Per-thread caches: the half-size pyramid of the current still picture, and the whole
/// picture pre-scaled to the current zoom (so panning at less than 100% is just a copy).
#[derive(Default)]
struct Caches {
    pyramid_for: Option<Arc<DecodedImage>>,
    pyramid: Pyramid,
    scaled: Option<Scaled>,
}

struct Scaled {
    image: Arc<DecodedImage>,
    frame: usize,
    scale_bits: u64,
    background: Option<Background>,
    picture: Rgba8Image,
}

fn run(shared: &Shared, deliver: &dyn Fn(Rendered)) {
    let mut renderer = ViewportRenderer::new();
    let mut caches = Caches::default();
    loop {
        let request = {
            let mut mailbox = shared
                .mailbox
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            loop {
                if mailbox.stop {
                    return;
                }
                if let Some(request) = mailbox.request.take() {
                    break request;
                }
                mailbox = shared
                    .wake
                    .wait(mailbox)
                    .unwrap_or_else(PoisonError::into_inner);
            }
        };
        let started = Instant::now();
        if let Some(picture) = render(&mut renderer, &mut caches, &request) {
            deliver(Rendered {
                seq: request.seq,
                buffer: SharedPixelBuffer::clone_from_slice(
                    picture.data(),
                    picture.width(),
                    picture.height(),
                ),
                plan: request.plan,
                elapsed: started.elapsed(),
            });
        } else {
            tracing::warn!(seq = request.seq, "could not render the viewport");
        }
    }
}

fn render(
    renderer: &mut ViewportRenderer,
    caches: &mut Caches,
    request: &RenderRequest,
) -> Option<Rgba8Image> {
    let image = &request.image;
    let frame = image
        .frame(request.frame)
        .map_or_else(|| image.first(), |frame| &frame.image);
    let plan = &request.plan;
    if image.is_animated() {
        return renderer
            .render(frame, image.source_size, plan, request.background)
            .ok();
    }

    // Zoomed out with the whole picture scaled to at most 4 windows: render the whole
    // picture once at this scale, then every pan is a crop.
    let whole = Size::new(
        to_px(f64::from(image.source_size.width) * plan.scale).max(1),
        to_px(f64::from(image.source_size.height) * plan.scale).max(1),
    );
    let window_area = u64::from(request.window.width) * u64::from(request.window.height);
    let whole_area = u64::from(whole.width) * u64::from(whole.height);
    let pans = whole.width > plan.output.width || whole.height > plan.output.height;
    if plan.scale < 1.0 && pans && whole_area <= window_area.saturating_mul(4) {
        let reusable = caches.scaled.as_ref().is_some_and(|cached| {
            Arc::ptr_eq(&cached.image, image)
                && cached.frame == request.frame
                && cached.scale_bits == plan.scale.to_bits()
                && cached.background == request.background
        });
        if !reusable {
            let full_plan = Plan {
                scale: plan.scale,
                output: whole,
                offset: (0, 0),
                source: Rect {
                    left: 0.0,
                    top: 0.0,
                    width: f64::from(image.source_size.width),
                    height: f64::from(image.source_size.height),
                },
            };
            reset_pyramid_if_new(caches, image);
            let source = caches.pyramid.level_for(
                renderer,
                image.first(),
                image.has_alpha,
                image.source_size,
                plan.scale,
            );
            let picture = renderer
                .render(source, image.source_size, &full_plan, request.background)
                .ok()?;
            caches.scaled = Some(Scaled {
                image: Arc::clone(image),
                frame: request.frame,
                scale_bits: plan.scale.to_bits(),
                background: request.background,
                picture,
            });
        }
        let entry = caches.scaled.as_ref()?;
        return crop(&entry.picture, plan);
    }

    reset_pyramid_if_new(caches, image);
    let source = caches.pyramid.level_for(
        renderer,
        image.first(),
        image.has_alpha,
        image.source_size,
        plan.scale,
    );
    renderer
        .render(source, image.source_size, plan, request.background)
        .ok()
}

/// Starts a fresh pyramid when the picture changed (the old levels belong to another one).
fn reset_pyramid_if_new(caches: &mut Caches, image: &Arc<DecodedImage>) {
    let same = caches
        .pyramid_for
        .as_ref()
        .is_some_and(|current| Arc::ptr_eq(current, image));
    if !same {
        caches.pyramid = Pyramid::new();
        caches.pyramid_for = Some(Arc::clone(image));
    }
}

/// Copies the visible window out of a picture pre-scaled to `plan.scale`.
fn crop(scaled: &Rgba8Image, plan: &Plan) -> Option<Rgba8Image> {
    let left = to_px(plan.source.left * plan.scale);
    let top = to_px(plan.source.top * plan.scale);
    let (out_w, out_h) = (plan.output.width, plan.output.height);
    let left = left.min(scaled.width().saturating_sub(out_w));
    let top = top.min(scaled.height().saturating_sub(out_h));
    let row_bytes = usize::try_from(out_w).ok()?.checked_mul(4)?;
    let stride = usize::try_from(scaled.width()).ok()?.checked_mul(4)?;
    let mut out = Vec::with_capacity(row_bytes.checked_mul(usize::try_from(out_h).ok()?)?);
    for row in 0..out_h {
        let start = usize::try_from(top + row).ok()? * stride + usize::try_from(left).ok()? * 4;
        out.extend_from_slice(scaled.data().get(start..start + row_bytes)?);
    }
    Rgba8Image::new(out_w, out_h, out)
}
