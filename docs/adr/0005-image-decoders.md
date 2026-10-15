# ADR 0005 — Image decoders (corrections to the brief's stack table)

- **Status:** **Accepted** (owner, 2026-10-15)
- **Deciders:** project owner, Claude Code
- **Evidence:** `docs/perf.md` (Phase 1 S5), spike `spike/s5-images`.

## Context
PROJECT_BRIEF.md §7.2 lists the `image` crate for AVIF, scaled JPEG decoding "e.g. via jpeg-decoder", libheif for HEIC, and jxl-oxide for JPEG XL. The brief asks to be told when a table entry turns out to be wrong. Two entries need correcting.

## Findings
1. **`image` cannot decode AVIF by itself.** Its `avif` feature only *encodes* (ravif). Decoding needs `avif-native`, which uses the C library libdav1d (another native dependency to ship on Windows).
2. **`image`'s JPEG decoder (zune-jpeg) has no DCT-scaled decoding.** `jpeg-decoder` 0.3.2 does (`Decoder::scale`), giving 19–44 ms instead of 79 ms for a 24 MP JPEG decoded for screen size.
3. libheif via vcpkg on Windows works (HEIC: 167 ms for 12 MP). The AV1 part of vcpkg's libheif (aom) failed to build here (download of a Perl tool timed out), and isn't needed if AVIF is handled separately.
4. `image`'s animation iterators never end on truncated files: they return the same error forever. Callers must stop at the first error and cap frame counts.
5. TGA has no magic bytes, so content sniffing can't find it. We need "sniff first, then fall back to the extension".

## Proposed decision
| Format | Decoder | License | Notes |
|---|---|---|---|
| JPEG, PNG/APNG, GIF, WebP, TIFF, BMP, ICO, TGA, PNM, QOI, DDS, HDR, EXR | `image` 0.25.x (no `avif` feature) | MIT/Apache-2.0 | EXIF orientation via `ImageDecoder::orientation` + `apply_orientation` (all 8 verified); `Limits::max_alloc` |
| JPEG, screen-sized preview | `jpeg-decoder` 0.3.x `scale()` | MIT/Apache-2.0 | fast first pixel; full-resolution decode stays on `image` |
| **AVIF** | **`avif-decode` 3.x → `rav1d`** (memory-safe Rust port of dav1d) | BSD-3 / BSD-2 | no C library to ship; needs **NASM at build time** on x86-64 (runtime SIMD detection, no AVX2 requirement) |
| HEIC/HEIF | `libheif-rs` 3.x (`default-features = false, features = ["v1_17"]`) → system/vcpkg **libheif**, dynamically linked | MIT / LGPL-3 | `v1_17` because Mint 22 ships libheif 1.17.6; ship only the HEVC *decoder* (libde265) |
| JPEG XL | `jxl-oxide` 0.12.x | MIT/Apache-2.0 | 271 ms for 12 MP here; check its threading in Phase 2 |
| SVG | `resvg` | (verify in Phase 2) | not tested in Phase 1 |

Every decode goes through: **content sniffing → extension fallback → size check before allocating (≤ 1.5 GB RGBA) → decoder inside `catch_unwind` → error mapped to a friendly message**. Verified with truncated/empty/random/bomb files of every format: no panics, no crashes.

## Consequences
- The development setup needs **NASM** (Windows: `winget install NASM.NASM`; Linux: `apt install nasm`). CI must install it.
- The Windows bundle ships libheif + libde265 DLLs (LGPL, dynamic) but **not** x265 or aom (those were only for generating test files).
- Phase 8 fuzzing must use **time limits** (hangs, not only crashes); see finding 4.
