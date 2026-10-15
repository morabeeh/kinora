//! The image viewer: folder navigation, background decoding with preloading and
//! cancellation, the byte-budgeted cache, zoom/pan, and input handling.
//!
//! Threading (brief §7.5, ARCHITECTURE.md): this controller lives on the **UI thread**.
//! Decoding runs on the [`WorkPool`] workers, viewport scaling on the render thread, and the
//! folder scan on its own thread. They hand results back with `slint::invoke_from_event_loop`,
//! which runs a closure on the UI thread; the closure finds the controller through
//! [`with_viewer`] (a thread-local, because the controller is not `Send`).

mod input;
mod jobs;
mod render;
mod scan;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use kinora_cache::{ByteLru, WorkPool};
use kinora_core::folder::{FolderModel, MediaItem};
use kinora_core::hardware::Budgets;
use kinora_core::media::MediaKind;
use kinora_core::navigation::{Direction, NavCommand, NavOutcome, Navigator};
use kinora_core::preload::preload_targets;
use kinora_core::settings::{Settings, TransparencyBackground, WheelAction};
use kinora_core::viewport::{self, FitOptions, Plan, Size, View};
use kinora_decode::viewport::Background;
use kinora_decode::{DecodeOptions, DecodedImage, Target};
use slint::winit_030::WinitWindowAccessor as _;
use slint::{ComponentHandle as _, Timer, TimerMode};

use self::input::{KeyAction, map_key};
use self::jobs::{Failure, ImageKey, Outcome, Quality};
use self::render::{RenderRequest, RenderThread, Rendered};
use crate::ui::{AppWindow, ErrorKind};

/// Show "Loading…" only if a picture takes longer than this (no flicker for fast ones).
const LOADING_DELAY: Duration = Duration::from_millis(150);
/// How long the "end of folder" hint stays.
const END_HINT_TIME: Duration = Duration::from_millis(1500);
/// Scroll distance (logical px) that counts as one wheel notch (Slint: 1 line = 60 px).
const WHEEL_NOTCH: f64 = 60.0;
/// Horizontal touchpad swipe distance that changes the picture.
const SWIPE_DISTANCE: f64 = 150.0;
/// Arrow-key pan step in logical pixels.
const PAN_STEP: f64 = 80.0;

thread_local! {
    static VIEWER: RefCell<Option<Rc<Viewer>>> = const { RefCell::new(None) };
}

/// Registers the controller for [`with_viewer`].
pub fn install(viewer: Rc<Viewer>) {
    VIEWER.with(|cell| *cell.borrow_mut() = Some(viewer));
}

/// Removes the controller (at shutdown, so its threads stop before the UI goes away).
pub fn uninstall() {
    let viewer = VIEWER.with(|cell| cell.borrow_mut().take());
    drop(viewer);
}

/// Runs `f` with the controller, on the UI thread.
pub fn with_viewer(f: impl FnOnce(&Viewer)) {
    // Clone the Rc and release the thread-local borrow before calling `f`, so `f` may
    // itself trigger code that uses `with_viewer`.
    let viewer = VIEWER.with(|cell| cell.borrow().clone());
    if let Some(viewer) = viewer {
        f(&viewer);
    }
}

/// Runs `f` on the UI thread with the controller (from any thread).
fn post(f: impl FnOnce(&Viewer) + Send + 'static) {
    // Fails only while the event loop shuts down; the result is no longer needed then.
    let _ = slint::invoke_from_event_loop(move || with_viewer(f));
}

/// Fixed configuration for this session.
#[derive(Debug, Clone)]
pub struct Config {
    pub budgets: Budgets,
    pub settings: Settings,
    /// Screen size (physical px) used for screen-sized decodes until the window knows its
    /// monitor (remembered from the last session).
    pub screen_guess: Size,
    pub bench: bool,
    pub process_start: Instant,
}

struct Shown {
    path: PathBuf,
    image: Arc<DecodedImage>,
    quality: Quality,
}

struct State {
    folder: FolderModel,
    nav: Navigator,
    folder_path: Option<PathBuf>,
    cache: ByteLru<ImageKey, DecodedImage>,
    failures: HashMap<PathBuf, Failure>,
    /// Pictures too large for a full-resolution decode on this PC.
    full_refused: HashSet<PathBuf>,
    /// Full resolution was requested for these (zoomed in past the screen-sized decode).
    full_wanted: HashSet<PathBuf>,
    /// Jobs submitted whose result hasn't reached the UI thread yet. (The pool only knows
    /// queued and running jobs; a finished job's result can still be on its way.)
    requested: HashSet<ImageKey>,
    shown: Option<Shown>,
    view: View,
    press_view: Option<View>,
    window: Size,
    scale_factor: f32,
    screen: Size,
    /// The monitor's size, once the native window exists.
    monitor: Option<Size>,
    render_seq: u64,
    applied_seq: u64,
    wheel: (f64, f64),
    frame: usize,
    playing: bool,
    navigated_at: Instant,
    first_pixel_done: bool,
    dialog_open: bool,
}

/// The viewer controller (UI thread only).
pub struct Viewer {
    window: slint::Weak<AppWindow>,
    config: Config,
    state: RefCell<State>,
    pool: WorkPool<ImageKey>,
    renderer: RenderThread,
    loading_timer: Timer,
    hint_timer: Timer,
    animation_timer: Timer,
    quit_timer: Timer,
}

impl Viewer {
    pub fn new(window: &AppWindow, config: Config) -> anyhow::Result<Rc<Self>> {
        let pool = WorkPool::new("decode", config.budgets.decode_workers, |key: &ImageKey| {
            let key = key.clone();
            post(move |viewer| {
                viewer.on_decoded(Outcome {
                    key,
                    result: Err(Failure::panicked()),
                    elapsed: Duration::ZERO,
                });
            });
        })?;
        let renderer = RenderThread::start(|frame| post(move |viewer| viewer.on_rendered(frame)))?;
        let settings = &config.settings;
        let state = State {
            folder: FolderModel::new(settings.viewer.sort),
            nav: Navigator::new(settings.viewer.wrap_around),
            folder_path: None,
            cache: ByteLru::new(config.budgets.image_cache_bytes),
            failures: HashMap::new(),
            full_refused: HashSet::new(),
            full_wanted: HashSet::new(),
            requested: HashSet::new(),
            shown: None,
            view: View::default(),
            press_view: None,
            window: Size::new(1280, 800),
            scale_factor: 1.0,
            screen: config.screen_guess,
            monitor: None,
            render_seq: 0,
            applied_seq: 0,
            wheel: (0.0, 0.0),
            frame: 0,
            playing: true,
            navigated_at: Instant::now(),
            first_pixel_done: false,
            dialog_open: false,
        };
        Ok(Rc::new(Self {
            window: window.as_weak(),
            config,
            state: RefCell::new(state),
            pool,
            renderer,
            loading_timer: Timer::default(),
            hint_timer: Timer::default(),
            animation_timer: Timer::default(),
            quit_timer: Timer::default(),
        }))
    }

    /// Connects the window's callbacks to this controller.
    pub fn bind(window: &AppWindow) {
        window.on_key(|text, ctrl, shift, alt| {
            let mut handled = false;
            with_viewer(|v| handled = v.key(&text, ctrl, shift, alt));
            handled
        });
        window.on_wheel(|dx, dy, x, y, ctrl| with_viewer(|v| v.wheel(dx, dy, (x, y), ctrl)));
        window.on_press(|x, y| with_viewer(|v| v.press(x, y)));
        window.on_drag(|dx, dy| with_viewer(|v| v.drag(dx, dy)));
        window.on_release(|| with_viewer(Viewer::release));
        window.on_double_click(|x, y| with_viewer(|v| v.double_click(x, y)));
        window.on_navigate(|delta| {
            with_viewer(|v| {
                v.navigate(if delta < 0 {
                    NavCommand::Previous
                } else {
                    NavCommand::Next
                });
            });
        });
        window.on_open_file(|| with_viewer(Viewer::open_dialog));
        window.on_resized(|| with_viewer(Viewer::resized));
    }

    // ---- Opening ----------------------------------------------------------------------

    /// Opens a file (shows it at once) or a folder (shows its first picture once listed).
    pub fn open(&self, path: &Path) {
        let path = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
        let Some(folder) = scan::folder_of(&path) else {
            return;
        };
        let opened_file = (!path.is_dir()).then(|| path.clone());
        tracing::info!(path = %path.display(), "opening");
        {
            let mut state = self.state.borrow_mut();
            let order = state.folder.order();
            state.folder = match &opened_file {
                Some(file) => {
                    let kind = file
                        .file_name()
                        .and_then(|name| {
                            kinora_core::media::kind_from_file_name(&name.to_string_lossy())
                        })
                        .unwrap_or(MediaKind::Image);
                    FolderModel::with_single(MediaItem::new(file.clone(), kind), order)
                }
                None => FolderModel::new(order),
            };
            state.nav.bump_generation();
            state.folder_path = Some(folder.clone());
            state.failures.clear();
            state.full_wanted.clear();
        }
        self.show_current();
        self.start_scan(folder, opened_file);
    }

    fn start_scan(&self, folder: PathBuf, opened: Option<PathBuf>) {
        let sort = self.state.borrow().folder.order().key;
        let spawned = std::thread::Builder::new()
            .name(String::from("folder-scan"))
            .spawn(move || {
                let started = Instant::now();
                let result = scan::scan(&folder, opened.as_deref(), sort);
                let elapsed = started.elapsed();
                post(move |viewer| viewer.on_scanned(&folder, result, elapsed));
            });
        if let Err(err) = spawned {
            tracing::warn!(%err, "could not start the folder scan");
        }
    }

    fn on_scanned(
        &self,
        folder: &Path,
        result: std::io::Result<Vec<MediaItem>>,
        elapsed: Duration,
    ) {
        let items = match result {
            Ok(items) => items,
            Err(err) => {
                tracing::warn!(folder = %folder.display(), %err, "could not list the folder");
                return;
            }
        };
        let had_current = {
            let mut state = self.state.borrow_mut();
            if state.folder_path.as_deref() != Some(folder) {
                return; // the user opened something else meanwhile
            }
            tracing::info!(
                folder = %folder.display(),
                items = items.len(),
                ms = elapsed.as_millis(),
                "folder listed"
            );
            let had_current = state.folder.current_index().is_some();
            state.folder.replace_items(items);
            had_current
        };
        if had_current {
            self.update_texts();
            self.schedule();
        } else {
            self.show_current();
        }
    }

    /// Opens the native "Open" dialog on a helper thread (it blocks while open).
    pub fn open_dialog(&self) {
        let start_dir = {
            let mut state = self.state.borrow_mut();
            if state.dialog_open {
                return;
            }
            state.dialog_open = true;
            state.folder_path.clone()
        };
        let spawned = std::thread::Builder::new()
            .name(String::from("open-dialog"))
            .spawn(move || {
                let mut dialog = rfd::FileDialog::new()
                    .set_title("Open")
                    .add_filter("Pictures", kinora_core::media::IMAGE_EXTENSIONS)
                    .add_filter("All files", &["*"]);
                if let Some(dir) = start_dir {
                    dialog = dialog.set_directory(dir);
                }
                let picked = dialog.pick_file();
                post(move |viewer| {
                    viewer.state.borrow_mut().dialog_open = false;
                    if let Some(path) = picked {
                        viewer.open(&path);
                    }
                });
            });
        if let Err(err) = spawned {
            tracing::warn!(%err, "could not open the file dialog");
            self.state.borrow_mut().dialog_open = false;
        }
    }

    /// Files forwarded by a second Kinora (single instance) or dropped on the window.
    pub fn open_paths(&self, paths: &[PathBuf]) {
        if let Some(path) = paths.iter().find(|path| path.exists()) {
            self.open(path);
        }
    }

    // ---- Navigation -------------------------------------------------------------------

    pub fn navigate(&self, command: NavCommand) {
        let outcome = {
            let mut guard = self.state.borrow_mut();
            let state = &mut *guard;
            state.nav.apply(&mut state.folder, command)
        };
        match outcome {
            NavOutcome::Moved { .. } => self.show_current(),
            NavOutcome::EndReached(direction) => self.show_end_hint(direction),
            NavOutcome::Unchanged => {}
        }
    }

    fn show_end_hint(&self, direction: Direction) {
        if let Some(ui) = self.window.upgrade() {
            ui.set_end_hint(if direction == Direction::Forward {
                1
            } else {
                -1
            });
        }
        let window = self.window.clone();
        self.hint_timer
            .start(TimerMode::SingleShot, END_HINT_TIME, move || {
                if let Some(ui) = window.upgrade() {
                    ui.set_end_hint(0);
                }
            });
    }

    /// Shows the current item: from the cache if possible, otherwise starts decoding it.
    fn show_current(&self) {
        self.animation_timer.stop();
        self.update_texts();
        let (item, cached, failure) = {
            let mut state = self.state.borrow_mut();
            state.navigated_at = Instant::now();
            let Some(item) = state.folder.current().cloned() else {
                drop(state);
                self.show_nothing();
                return;
            };
            let cached = [Quality::Full, Quality::Screen]
                .into_iter()
                .find_map(|quality| {
                    let key = ImageKey::new(item.path.clone(), quality);
                    state.cache.get(&key).map(|image| (image, quality))
                });
            let failure = state.failures.get(&item.path).cloned();
            (item, cached, failure)
        };
        if item.kind == MediaKind::Video {
            self.show_failure(&Failure::video());
        } else if let Some(failure) = failure {
            self.show_failure(&failure);
        } else if let Some((image, quality)) = cached {
            self.display(&item.path, image, quality, true);
        } else {
            self.start_loading_timer(item.path.clone());
        }
        self.schedule();
    }

    fn show_nothing(&self) {
        self.state.borrow_mut().shown = None;
        if let Some(ui) = self.window.upgrade() {
            ui.set_has_picture(false);
            ui.set_loading(false);
            ui.set_error_kind(ErrorKind::None);
        }
    }

    /// After a short delay without a picture, hide the previous one and say "Loading…".
    fn start_loading_timer(&self, path: PathBuf) {
        let window = self.window.clone();
        self.loading_timer
            .start(TimerMode::SingleShot, LOADING_DELAY, move || {
                with_viewer(|viewer| {
                    let still_waiting = viewer
                        .state
                        .borrow()
                        .shown
                        .as_ref()
                        .is_none_or(|shown| shown.path != path);
                    let current = viewer
                        .state
                        .borrow()
                        .folder
                        .current()
                        .is_some_and(|item| item.path == path);
                    if still_waiting
                        && current
                        && let Some(ui) = window.upgrade()
                    {
                        ui.set_loading(true);
                        ui.set_error_kind(ErrorKind::None);
                    }
                });
            });
    }

    fn update_texts(&self) {
        let Some(ui) = self.window.upgrade() else {
            return;
        };
        let state = self.state.borrow();
        let item = state.folder.current();
        ui.set_file_name(
            item.map(|item| item.name.clone())
                .unwrap_or_default()
                .into(),
        );
        let position = state.folder.current_index().map_or(0, |index| index + 1);
        ui.set_position(i32::try_from(position).unwrap_or(i32::MAX));
        ui.set_count(i32::try_from(state.folder.len()).unwrap_or(i32::MAX));
    }

    // ---- Background work ----------------------------------------------------------------

    /// Decodes the current item first, then preloads neighbours (direction-aware); drops
    /// queued jobs nobody needs any more and pins the current item in the cache.
    fn schedule(&self) {
        let mut wanted: Vec<(ImageKey, u8)> = Vec::new();
        {
            let mut state = self.state.borrow_mut();
            let Some(current) = state.folder.current_index() else {
                return;
            };
            let mut consider = |state: &State, index: usize, priority: u8| {
                let Some(item) = state.folder.get(index) else {
                    return;
                };
                if item.kind != MediaKind::Image || state.failures.contains_key(&item.path) {
                    return;
                }
                let screen = ImageKey::new(item.path.clone(), Quality::Screen);
                let full = ImageKey::new(item.path.clone(), Quality::Full);
                if state.cache.contains(&full) {
                    return;
                }
                if !state.cache.contains(&screen) {
                    wanted.push((screen, priority));
                }
                if priority == 0
                    && state.full_wanted.contains(&item.path)
                    && !state.full_refused.contains(&item.path)
                {
                    wanted.push((full, 0));
                }
            };
            consider(&state, current, 0);
            let targets = preload_targets(
                state.folder.len(),
                current,
                state.nav.direction(),
                state.nav.wraps(),
                self.config.budgets.preload,
            );
            for target in targets {
                consider(&state, target.index, target.priority);
            }
            if let Some(item) = state.folder.current() {
                let pinned = [
                    ImageKey::new(item.path.clone(), Quality::Screen),
                    ImageKey::new(item.path.clone(), Quality::Full),
                ];
                state.cache.set_pinned(pinned);
            }
        }
        let mut dropped: Vec<ImageKey> = Vec::new();
        self.pool.reprioritize(|key| {
            let priority = wanted
                .iter()
                .find(|(wanted_key, _)| wanted_key == key)
                .map(|(_, priority)| *priority);
            if priority.is_none() {
                dropped.push(key.clone());
            }
            priority
        });
        let to_submit: Vec<(ImageKey, u8)> = {
            let mut state = self.state.borrow_mut();
            for key in &dropped {
                state.requested.remove(key);
            }
            wanted
                .into_iter()
                .filter(|(key, _)| state.requested.insert(key.clone()))
                .collect()
        };
        for (key, priority) in to_submit {
            self.submit_decode(key, priority);
        }
    }

    fn submit_decode(&self, key: ImageKey, priority: u8) {
        let budgets = &self.config.budgets;
        let screen = self.state.borrow().screen;
        let options = DecodeOptions {
            target: match key.quality {
                Quality::Full => Target::Full,
                Quality::Screen | Quality::Thumbnail => Target::Screen {
                    width: screen.width,
                    height: screen.height,
                    keep_full_below_bytes: budgets.image_cache_bytes / 8,
                },
            },
            max_rgba_bytes: budgets.max_image_bytes,
            max_animation_bytes: budgets.image_cache_bytes / 2,
            ..DecodeOptions::default()
        };
        // Lite mode: show the embedded EXIF thumbnail first for the picture on screen.
        let thumbnail_first = budgets.lite_mode && priority == 0 && key.quality == Quality::Screen;
        let job_key = key.clone();
        self.pool.submit(key, priority, move || {
            jobs::decode(&job_key, &options, thumbnail_first, &|outcome| {
                post(move |viewer| viewer.on_decoded(outcome));
            });
        });
    }

    fn on_decoded(&self, outcome: Outcome) {
        let Outcome {
            key,
            result,
            elapsed,
        } = outcome;
        self.state.borrow_mut().requested.remove(&key);
        let is_current = self
            .state
            .borrow()
            .folder
            .current()
            .is_some_and(|item| item.path == key.path);
        match result {
            Ok(image) => {
                tracing::debug!(
                    file = %key.path.display(),
                    quality = ?key.quality,
                    ms = elapsed.as_millis(),
                    "decoded"
                );
                if key.quality != Quality::Thumbnail {
                    let mut state = self.state.borrow_mut();
                    state.failures.remove(&key.path);
                    let bytes = image.byte_size();
                    state.cache.insert(key.clone(), Arc::clone(&image), bytes);
                }
                if is_current {
                    let better =
                        self.state.borrow().shown.as_ref().is_none_or(|shown| {
                            shown.path != key.path || shown.quality < key.quality
                        });
                    if better {
                        self.display(&key.path, image, key.quality, false);
                    }
                }
            }
            Err(failure) if key.quality == Quality::Full => {
                // Keep showing the screen-sized picture; just don't try again.
                tracing::info!(file = %key.path.display(), detail = %failure.detail, "full resolution not available");
                self.state
                    .borrow_mut()
                    .full_refused
                    .insert(key.path.clone());
            }
            Err(failure) => {
                self.state
                    .borrow_mut()
                    .failures
                    .insert(key.path.clone(), failure.clone());
                if is_current {
                    self.show_failure(&failure);
                }
            }
        }
        self.schedule();
    }

    fn show_failure(&self, failure: &Failure) {
        self.loading_timer.stop();
        self.state.borrow_mut().shown = None;
        if let Some(ui) = self.window.upgrade() {
            ui.set_loading(false);
            ui.set_has_picture(false);
            ui.set_can_pan(false);
            ui.set_error_kind(failure.kind);
            ui.set_error_format(failure.format.clone().into());
            ui.set_error_detail(failure.detail.clone().into());
        }
    }

    /// Puts a decoded picture on screen. A better version of the same picture keeps the
    /// current zoom and position.
    fn display(&self, path: &Path, image: Arc<DecodedImage>, quality: Quality, cached: bool) {
        self.loading_timer.stop();
        let animated = image.is_animated();
        let new_picture = {
            let mut state = self.state.borrow_mut();
            let new_picture = state.shown.as_ref().is_none_or(|shown| shown.path != path);
            if new_picture {
                state.view = View::fit(image.source_size);
                state.frame = 0;
                state.playing = true;
                let ms = state.navigated_at.elapsed().as_millis();
                tracing::info!(
                    file = %path.display(),
                    ?quality,
                    cached,
                    ms,
                    "shown"
                );
            }
            state.shown = Some(Shown {
                path: path.to_path_buf(),
                image,
                quality,
            });
            new_picture
        };
        if let Some(ui) = self.window.upgrade() {
            ui.set_error_kind(ErrorKind::None);
            ui.set_loading(false);
        }
        if new_picture && animated {
            self.schedule_animation_frame();
        }
        self.request_render();
    }

    // ---- Rendering ----------------------------------------------------------------------

    fn fit_options(&self) -> FitOptions {
        FitOptions {
            enlarge_small: self.config.settings.viewer.enlarge_small_images,
        }
    }

    fn background(&self) -> Background {
        let scale = self.state.borrow().scale_factor;
        match self.config.settings.viewer.transparency {
            TransparencyBackground::Checkerboard => Background::checkerboard(scale),
            TransparencyBackground::Light => Background::Solid([0xE6, 0xE6, 0xE6]),
            TransparencyBackground::Dark => Background::Solid([0x26, 0x26, 0x26]),
        }
    }

    fn current_plan(&self) -> Option<(Plan, Arc<DecodedImage>)> {
        let state = self.state.borrow();
        let shown = state.shown.as_ref()?;
        let plan = viewport::plan(
            shown.image.source_size,
            state.window,
            &state.view,
            self.fit_options(),
        );
        Some((plan, Arc::clone(&shown.image)))
    }

    /// Asks the render thread for a new frame of the current view.
    fn request_render(&self) {
        let Some((plan, image)) = self.current_plan() else {
            return;
        };
        let background = image.has_alpha.then(|| self.background());
        let (seq, frame, window, need_full) = {
            let mut state = self.state.borrow_mut();
            state.render_seq += 1;
            let buffer_ratio =
                f64::from(image.first().width()) / f64::from(image.source_size.width.max(1));
            let need_full = state.shown.as_ref().is_some_and(|shown| {
                shown.quality != Quality::Full
                    && !image.is_full_resolution()
                    && plan.scale > buffer_ratio * 1.02
                    && !state.full_refused.contains(&shown.path)
            });
            (state.render_seq, state.frame, state.window, need_full)
        };
        if let Some(ui) = self.window.upgrade() {
            ui.set_zoom_percent(i32::try_from(viewport::to_px(plan.scale * 100.0)).unwrap_or(0));
            ui.set_can_pan(viewport::can_pan(&plan, image.source_size));
        }
        self.renderer.submit(RenderRequest {
            seq,
            image,
            frame,
            plan,
            background,
            window,
        });
        if need_full {
            let path = self
                .state
                .borrow()
                .shown
                .as_ref()
                .map(|shown| shown.path.clone());
            if let Some(path) = path
                && self.state.borrow_mut().full_wanted.insert(path)
            {
                self.schedule();
            }
        }
    }

    fn on_rendered(&self, frame: Rendered) {
        {
            let mut state = self.state.borrow_mut();
            if frame.seq < state.applied_seq {
                return; // an older frame finished after a newer one
            }
            state.applied_seq = frame.seq;
        }
        let Some(ui) = self.window.upgrade() else {
            return;
        };
        let plan = frame.plan;
        ui.set_frame(slint::Image::from_rgba8(frame.buffer));
        ui.set_frame_x(i32::try_from(plan.offset.0).unwrap_or(0));
        ui.set_frame_y(i32::try_from(plan.offset.1).unwrap_or(0));
        ui.set_frame_width(i32::try_from(plan.output.width).unwrap_or(0));
        ui.set_frame_height(i32::try_from(plan.output.height).unwrap_or(0));
        let has_picture = self.state.borrow().shown.is_some();
        ui.set_has_picture(has_picture);
        tracing::trace!(ms = frame.elapsed.as_secs_f64() * 1000.0, "viewport frame");
        self.first_pixel();
    }

    /// `--bench-startup`: report time to the first picture, then quit.
    fn first_pixel(&self) {
        let mut state = self.state.borrow_mut();
        let quality = state.shown.as_ref().map(|shown| shown.quality);
        if state.first_pixel_done || quality.is_none_or(|q| q == Quality::Thumbnail) {
            return;
        }
        state.first_pixel_done = true;
        let since_start = self.config.process_start.elapsed();
        let memory = kinora_platform::memory::process_memory();
        tracing::info!(
            ms = since_start.as_millis(),
            rss_mb = memory.map_or(0, |m| m.resident_bytes / (1024 * 1024)),
            "first picture on screen (from process start)"
        );
        if self.config.bench {
            self.quit_timer
                .start(TimerMode::SingleShot, Duration::from_millis(300), || {
                    let _ = slint::quit_event_loop();
                });
        }
    }

    // ---- Animation ----------------------------------------------------------------------

    fn schedule_animation_frame(&self) {
        let delay = {
            let state = self.state.borrow();
            let Some(shown) = state.shown.as_ref() else {
                return;
            };
            if !state.playing || !shown.image.is_animated() {
                return;
            }
            shown
                .image
                .frame(state.frame)
                .map_or(Duration::from_millis(100), |frame| frame.delay)
        };
        self.animation_timer
            .start(TimerMode::SingleShot, delay, move || {
                with_viewer(|viewer| viewer.step_frame(1, true));
            });
    }

    fn step_frame(&self, delta: i8, continue_playing: bool) {
        {
            let mut state = self.state.borrow_mut();
            let Some(count) = state.shown.as_ref().map(|s| s.image.frame_count()) else {
                return;
            };
            if count < 2 {
                return;
            }
            state.frame = if delta < 0 {
                (state.frame + count - 1) % count
            } else {
                (state.frame + 1) % count
            };
        }
        self.request_render();
        if continue_playing {
            self.schedule_animation_frame();
        }
    }

    // ---- Input ----------------------------------------------------------------------------

    fn key(&self, text: &str, ctrl: bool, shift: bool, alt: bool) -> bool {
        let Some(action) = map_key(text, ctrl, shift, alt) else {
            return false;
        };
        match action {
            KeyAction::Next => self.navigate(NavCommand::Next),
            KeyAction::Previous => self.navigate(NavCommand::Previous),
            KeyAction::First => self.navigate(NavCommand::First),
            KeyAction::Last => self.navigate(NavCommand::Last),
            KeyAction::Arrow(dx, dy) => {
                if self.can_pan() {
                    let scale = f64::from(self.state.borrow().scale_factor);
                    let step = PAN_STEP * scale;
                    self.update_view(|view, image, window, options| {
                        viewport::pan_by(
                            view,
                            image,
                            window,
                            options,
                            (-f64::from(dx) * step, -f64::from(dy) * step),
                        )
                    });
                } else if dx > 0 {
                    self.navigate(NavCommand::Next);
                } else if dx < 0 {
                    self.navigate(NavCommand::Previous);
                }
            }
            KeyAction::ZoomIn => self.zoom_steps(1.0, None),
            KeyAction::ZoomOut => self.zoom_steps(-1.0, None),
            KeyAction::Fit => self.update_view(|_, image, _, _| View::fit(image)),
            KeyAction::ActualSize => self.update_view(|view, image, window, options| {
                viewport::zoom_to(view, image, window, options, 1.0, None)
            }),
            KeyAction::ToggleFullscreen => self.toggle_fullscreen(),
            KeyAction::LeaveFullscreen => {
                if let Some(ui) = self.window.upgrade()
                    && ui.window().is_fullscreen()
                {
                    ui.window().set_fullscreen(false);
                } else {
                    return false;
                }
            }
            KeyAction::PlayPause => {
                let playing = {
                    let mut state = self.state.borrow_mut();
                    state.playing = !state.playing;
                    state.playing
                };
                if playing {
                    self.schedule_animation_frame();
                } else {
                    self.animation_timer.stop();
                }
            }
            KeyAction::StepFrame(delta) => {
                self.state.borrow_mut().playing = false;
                self.animation_timer.stop();
                self.step_frame(delta, false);
            }
            KeyAction::Open => self.open_dialog(),
        }
        true
    }

    fn toggle_fullscreen(&self) {
        if let Some(ui) = self.window.upgrade() {
            let fullscreen = ui.window().is_fullscreen();
            ui.window().set_fullscreen(!fullscreen);
        }
    }

    fn can_pan(&self) -> bool {
        self.current_plan()
            .is_some_and(|(plan, image)| viewport::can_pan(&plan, image.source_size))
    }

    /// Applies a view change and redraws.
    fn update_view(&self, change: impl FnOnce(&View, Size, Size, FitOptions) -> View) {
        let options = self.fit_options();
        {
            let mut state = self.state.borrow_mut();
            let Some(source) = state.shown.as_ref().map(|shown| shown.image.source_size) else {
                return;
            };
            let window = state.window;
            state.view = change(&state.view, source, window, options);
        }
        self.request_render();
    }

    fn zoom_steps(&self, steps: f64, anchor: Option<(f64, f64)>) {
        self.update_view(|view, image, window, options| {
            viewport::zoom_by_steps(view, image, window, options, steps, anchor)
        });
    }

    /// Mouse wheel / touchpad scroll. `x`/`y` are logical pixels.
    fn wheel(&self, dx: f32, dy: f32, (x, y): (f32, f32), ctrl: bool) {
        let (dx, dy) = (f64::from(dx), f64::from(dy));
        let scale = f64::from(self.state.borrow().scale_factor);
        let anchor = Some((f64::from(x) * scale, f64::from(y) * scale));
        // Mostly-horizontal scrolling is a touchpad swipe: previous/next (or pan if zoomed).
        if !ctrl && dx.abs() > dy.abs() {
            if self.can_pan() {
                self.update_view(|view, image, window, options| {
                    viewport::pan_by(view, image, window, options, (dx * scale, 0.0))
                });
                return;
            }
            let swipe = {
                let mut state = self.state.borrow_mut();
                state.wheel.0 += dx;
                let swipe = state.wheel.0;
                if swipe.abs() >= SWIPE_DISTANCE {
                    state.wheel.0 = 0.0;
                }
                swipe
            };
            if swipe <= -SWIPE_DISTANCE {
                self.navigate(NavCommand::Next);
            } else if swipe >= SWIPE_DISTANCE {
                self.navigate(NavCommand::Previous);
            }
            return;
        }
        let zooms = ctrl || self.config.settings.viewer.wheel == WheelAction::Zoom;
        if zooms {
            self.zoom_steps(dy / WHEEL_NOTCH, anchor);
            return;
        }
        let notches = {
            let mut state = self.state.borrow_mut();
            state.wheel.1 += dy;
            let notches = (state.wheel.1 / WHEEL_NOTCH).trunc();
            state.wheel.1 -= notches * WHEEL_NOTCH;
            notches
        };
        if notches > 0.0 {
            self.navigate(NavCommand::Previous);
        } else if notches < 0.0 {
            self.navigate(NavCommand::Next);
        }
    }

    fn press(&self, _x: f32, _y: f32) {
        let mut state = self.state.borrow_mut();
        state.press_view = Some(state.view);
    }

    fn drag(&self, dx: f32, dy: f32) {
        let options = self.fit_options();
        {
            let mut state = self.state.borrow_mut();
            let Some(origin) = state.press_view else {
                return;
            };
            let Some(source) = state.shown.as_ref().map(|shown| shown.image.source_size) else {
                return;
            };
            let scale = f64::from(state.scale_factor);
            let delta = (f64::from(dx) * scale, f64::from(dy) * scale);
            let window = state.window;
            state.view = viewport::pan_by(&origin, source, window, options, delta);
        }
        self.request_render();
    }

    fn release(&self) {
        self.state.borrow_mut().press_view = None;
    }

    fn double_click(&self, x: f32, y: f32) {
        let scale = f64::from(self.state.borrow().scale_factor);
        let anchor = Some((f64::from(x) * scale, f64::from(y) * scale));
        self.update_view(|view, image, window, options| {
            viewport::toggle_fit_actual(view, image, window, options, anchor)
        });
    }

    fn resized(&self) {
        let Some(ui) = self.window.upgrade() else {
            return;
        };
        let size = ui.window().size();
        let monitor_known = self.state.borrow().monitor.is_some();
        // The real monitor size is known once the native window exists (first resize).
        let monitor = if monitor_known {
            None
        } else {
            ui.window()
                .with_winit_window(|w| w.current_monitor().map(|m| m.size()))
                .flatten()
        };
        {
            let mut state = self.state.borrow_mut();
            state.window = Size::new(size.width.max(1), size.height.max(1));
            state.scale_factor = ui.window().scale_factor();
            if let Some(monitor) = monitor {
                state.screen = Size::new(monitor.width.max(1), monitor.height.max(1));
                state.monitor = Some(state.screen);
                tracing::info!(
                    ms = self.config.process_start.elapsed().as_millis(),
                    monitor_width = monitor.width,
                    monitor_height = monitor.height,
                    "native window ready (from process start)"
                );
            }
            // A window bigger than the screen size we decode for: decode bigger.
            state.screen = Size::new(
                state.screen.width.max(state.window.width),
                state.screen.height.max(state.window.height),
            );
        }
        self.request_render();
    }

    /// The monitor size learned this session (saved for the next start).
    pub fn screen_size(&self) -> Size {
        self.state.borrow().screen
    }

    /// Drag-and-drop feedback.
    pub fn set_drop_hover(&self, hover: bool) {
        if let Some(ui) = self.window.upgrade() {
            ui.set_drop_hover(hover);
        }
    }
}
