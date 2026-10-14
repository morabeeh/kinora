//! Video playback for Kinora — the **only** crate where libmpv FFI lives.
//!
//! Will contain: a safe wrapper over libmpv (created lazily, reused across files),
//! typed player events, and three output backends behind one trait:
//! (A) OpenGL render API into a texture, (B) native child-window (`wid`) embedding,
//! (C) software render API into a CPU buffer.
//!
//! `unsafe` will be allowed here (and in `kinora_platform`) once real FFI arrives;
//! every `unsafe` block must carry a `// SAFETY:` comment.
