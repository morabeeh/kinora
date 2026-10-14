//! Operating-system integration for Kinora.
//!
//! Will contain, behind one cross-platform API with `#[cfg(target_os = ...)]` modules:
//! hardware detection (CPU, RAM, GL renderer), "Open with…", move to Trash,
//! reveal in folder, set wallpaper, single-instance IPC and file associations.
//!
//! `unsafe` will be allowed here (and in `kinora_video`) once real OS calls arrive;
//! every `unsafe` block must carry a `// SAFETY:` comment.
