//! Image decoding for Kinora.
//!
//! Will contain: content-based format sniffing, decoders for every v1 image format,
//! EXIF orientation, ICC colour management, fast downscaling (including scaled JPEG
//! decoding) and animation frames. Every input is treated as hostile: dimensions are
//! validated before allocating, and errors are typed — never panics.
