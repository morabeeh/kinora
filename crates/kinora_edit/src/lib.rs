//! Basic image editing for Kinora.
//!
//! Will contain: the crop / rotate / flip / straighten / adjustments pipeline,
//! encoders, lossless JPEG rotation, and metadata-preserving **atomic** saves
//! (temp file → fsync → rename) that never damage the original.
