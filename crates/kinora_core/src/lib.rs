//! Pure logic for Kinora — no UI, no I/O side effects (only traits for them).
//!
//! Will contain: `MediaItem`, `FolderModel`, natural sorting, the navigation state
//! machine, the settings model, `HardwareProfile` tier rules, zoom/pan/crop math and
//! the edit-stack model. Everything here is heavily unit-tested.
//!
//! Dependency rule: this crate depends on **no other Kinora crate**.
