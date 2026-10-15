//! Pure logic for Kinora — no UI, no I/O side effects.
//!
//! Folder model and natural sorting, the navigation state machine, preload planning,
//! zoom/pan math, hardware-tier rules and the settings model. Everything here is heavily
//! unit-tested (including property tests), because the rest of the app builds on it.
//!
//! Dependency rule: this crate depends on **no other Kinora crate**.

pub mod folder;
pub mod hardware;
pub mod media;
pub mod natural_sort;
pub mod navigation;
pub mod preload;
pub mod settings;
pub mod viewport;
