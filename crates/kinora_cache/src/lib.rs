//! Caching for Kinora.
//!
//! Will contain: an LRU of decoded images budgeted in **bytes** (sized from the
//! hardware profile), a smaller thumbnail LRU, and reuse of the freedesktop thumbnail
//! cache (`~/.cache/thumbnails`) on Linux.
