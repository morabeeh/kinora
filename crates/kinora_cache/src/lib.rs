//! Caching and background work for Kinora (brief §7.5).
//!
//! - [`lru::ByteLru`]: a least-recently-used cache budgeted in **bytes**, with pinning so
//!   the picture on screen is never evicted by preloading.
//! - [`pool::WorkPool`]: worker threads that run the most urgent job first and drop jobs
//!   nobody needs any more (cancellation), with panics caught per job.
//!
//! Sizes (bytes, threads) always come from the hardware profile
//! (`kinora_core::hardware::budgets`). Phase 4 adds the thumbnail caches here.

pub mod lru;
pub mod pool;

pub use lru::ByteLru;
pub use pool::WorkPool;
