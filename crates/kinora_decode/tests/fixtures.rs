//! Decode tests over the committed fixtures in `test_data/images` (brief §8.5): every
//! format, all 8 EXIF orientations, CMYK, 16-bit, alpha, animations, HDR, SVG, and damaged
//! files that must fail gracefully — never panic, never hang.
#![cfg(test)] // test-only crate: lets clippy apply its test allowances to the helpers

use std::path::PathBuf;
use std::time::{Duration, Instant};

use kinora_core::viewport::Size;
use kinora_decode::{
    DecodeError, DecodeOptions, DecodedImage, Format, Target, decode_file, embedded_thumbnail,
};

fn path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../test_data/images")
        .join(name)
}

fn open(name: &str) -> Result<DecodedImage, DecodeError> {
    decode_file(&path(name), &DecodeOptions::default())
}

fn open_ok(name: &str) -> DecodedImage {
    open(name).unwrap_or_else(|err| panic!("{name}: {err}"))
}

fn screen(width: u32, height: u32, keep_full_below_bytes: u64) -> DecodeOptions {
    DecodeOptions {
        target: Target::Screen {
            width,
            height,
            keep_full_below_bytes,
        },
        ..DecodeOptions::default()
    }
}

/// The canonical fixture has red in its top-left corner, green top-right, blue bottom-left.
fn is_red([r, g, b, _]: [u8; 4]) -> bool {
    r > 170 && g < 90 && b < 90
}

#[test]
fn every_still_format_decodes_with_its_size() {
    let cases = [
        ("orient-1.jpg", Format::Jpeg, (64, 48)),
        ("rgb16.png", Format::Png, (64, 48)),
        ("rgba.png", Format::Png, (64, 48)),
        ("still.webp", Format::WebP, (64, 48)),
        ("lossy.webp", Format::WebP, (64, 48)),
        ("still.bmp", Format::Bmp, (64, 48)),
        ("still.tif", Format::Tiff, (64, 48)),
        ("still.tga", Format::Tga, (64, 48)),
        ("still.qoi", Format::Qoi, (64, 48)),
        ("still.ppm", Format::Pnm, (64, 48)),
        ("still.ico", Format::Ico, (32, 32)),
        ("still.dds", Format::Dds, (16, 16)),
        ("still.hdr", Format::Hdr, (32, 24)),
        ("still.exr", Format::Exr, (32, 24)),
        ("still.avif", Format::Avif, (128, 96)),
        ("still.jxl", Format::Jxl, (64, 48)),
        ("vector.svg", Format::Svg, (64, 48)),
        ("text.svg", Format::Svg, (200, 60)),
        ("png-named.jpg", Format::Png, (64, 48)),
    ];
    for (name, format, (width, height)) in cases {
        let image = open_ok(name);
        assert_eq!(image.format, format, "{name}");
        assert_eq!(image.source_size, Size::new(width, height), "{name}");
        assert!(image.first().width() > 0, "{name}");
    }
}

#[test]
#[cfg(feature = "heif")]
fn heic_decodes() {
    let image = open_ok("still.heic");
    assert_eq!(image.format, Format::Heif);
    assert_eq!(image.source_size, Size::new(128, 96));
    assert!(
        is_red(image.first().pixel(4, 4).unwrap()),
        "red top-left corner"
    );
}

#[test]
fn red_corner_survives_every_lossless_and_lossy_format() {
    for name in [
        "orient-1.jpg",
        "rgb16.png",
        "still.webp",
        "lossy.webp",
        "still.bmp",
        "still.tif",
        "still.tga",
        "still.qoi",
        "still.ppm",
        "still.avif",
        "still.jxl",
        "vector.svg",
    ] {
        let image = open_ok(name);
        let px = image.first().pixel(2, 2).unwrap();
        assert!(is_red(px), "{name}: top-left pixel {px:?} should be red");
    }
}

#[test]
fn all_eight_exif_orientations_are_applied() {
    for orientation in 1..=8u8 {
        let image = open_ok(&format!("orient-{orientation}.jpg"));
        let first = image.first();
        let swapped = orientation >= 5;
        assert_eq!(
            (first.width(), first.height()),
            if swapped { (48, 64) } else { (64, 48) },
            "orientation {orientation}"
        );
        let (w, h) = (first.width(), first.height());
        let corner = match orientation {
            1 | 5 => (3, 3),
            2 | 6 => (w - 4, 3),
            3 | 7 => (w - 4, h - 4),
            _ => (3, h - 4),
        };
        let px = first.pixel(corner.0, corner.1).unwrap();
        assert!(
            is_red(px),
            "orientation {orientation}: {corner:?} is {px:?}"
        );
    }
}

#[test]
fn cmyk_jpeg_has_the_right_colours() {
    let image = open_ok("cmyk.jpg");
    let px = image.first().pixel(2, 2).unwrap();
    assert!(is_red(px), "CMYK red corner decoded as {px:?}");
}

#[test]
fn transparency_is_kept() {
    let image = open_ok("rgba.png");
    assert!(image.has_alpha);
    assert_eq!(image.first().pixel(63, 47).unwrap()[3], 0);
    let half = image.first().pixel(32, 24).unwrap()[3];
    assert!((120..=136).contains(&half), "{half}");
    assert!(!open_ok("orient-1.jpg").has_alpha);
}

#[test]
fn animations_have_all_frames() {
    for name in ["anim.gif", "anim.webp", "anim-apng.png"] {
        let image = open_ok(name);
        assert_eq!(image.frame_count(), 3, "{name}");
        assert!(image.is_animated());
        let delay = image.frame(1).unwrap().delay;
        assert!(
            (Duration::from_millis(90)..=Duration::from_millis(110)).contains(&delay),
            "{name}: {delay:?}"
        );
        assert!(is_red(image.first().pixel(1, 1).unwrap()), "{name} frame 0");
        let green = image.frame(1).unwrap().image.pixel(1, 1).unwrap();
        assert!(green[1] > 150 && green[0] < 90, "{name} frame 1: {green:?}");
    }
}

#[test]
fn hdr_is_tone_mapped_not_clipped() {
    for name in ["still.hdr", "still.exr"] {
        let image = open_ok(name);
        let dark = image.first().pixel(0, 0).unwrap();
        let bright = image.first().pixel(31, 0).unwrap();
        assert!(
            bright[0] > dark[0],
            "{name}: gradient kept ({dark:?} → {bright:?})"
        );
        assert!(bright[0] < 255 || dark[0] > 0, "{name}: not all clipped");
    }
}

#[test]
fn screen_target_uses_scaled_jpeg_decoding() {
    let image = decode_file(&path("photo.jpg"), &screen(160, 120, 0)).unwrap();
    assert_eq!(image.source_size, Size::new(640, 480));
    assert_eq!(image.first().size(), Size::new(160, 120), "1/4 DCT scaling");
    assert!(!image.is_full_resolution());
    // Rotated photo: the scale is chosen for the rotated shape.
    let rotated = decode_file(&path("photo-rotated.jpg"), &screen(120, 160, 0)).unwrap();
    assert_eq!(rotated.source_size, Size::new(480, 640));
    assert_eq!(rotated.first().size(), Size::new(120, 160));
    // A screen as big as the photo: no reduction possible, full decode.
    let full = decode_file(&path("photo.jpg"), &screen(4000, 4000, 0)).unwrap();
    assert!(full.is_full_resolution());
}

#[test]
fn screen_target_shrinks_big_non_jpeg_pictures_only() {
    let shrunk = decode_file(&path("still.bmp"), &screen(32, 24, 0)).unwrap();
    assert_eq!(shrunk.first().size(), Size::new(32, 24));
    assert_eq!(shrunk.source_size, Size::new(64, 48));
    let kept = decode_file(&path("still.bmp"), &screen(32, 24, 1 << 20)).unwrap();
    assert!(
        kept.is_full_resolution(),
        "small enough to keep at full size"
    );
}

#[test]
fn svg_renders_at_the_requested_size() {
    let image = decode_file(&path("vector.svg"), &screen(640, 480, 0)).unwrap();
    assert_eq!(image.source_size, Size::new(64, 48));
    assert_eq!(
        image.first().size(),
        Size::new(640, 480),
        "vector: crisp at fit"
    );
    let full = open_ok("vector.svg");
    assert_eq!(
        full.first().size(),
        Size::new(128, 96),
        "full = 2× natural size"
    );
}

#[test]
fn embedded_exif_thumbnail_gives_a_first_picture() {
    let bytes = std::fs::read(path("photo.jpg")).unwrap();
    let thumb = embedded_thumbnail(&bytes).unwrap();
    assert_eq!(thumb.first().size(), Size::new(160, 120));
    assert_eq!(thumb.source_size, Size::new(640, 480));
    assert!(is_red(thumb.first().pixel(2, 2).unwrap()));
    let rotated = std::fs::read(path("photo-rotated.jpg")).unwrap();
    let thumb = embedded_thumbnail(&rotated).unwrap();
    assert_eq!(thumb.first().size(), Size::new(120, 160));
    assert_eq!(thumb.source_size, Size::new(480, 640));
    assert!(embedded_thumbnail(&std::fs::read(path("orient-1.jpg")).unwrap()).is_none());
}

#[test]
fn bad_files_fail_gracefully_and_quickly() {
    let names = [
        "bad-empty.jpg",
        "bad-random.png",
        "bad-bomb.png",
        "bad-bomb.jpg",
        "bad-truncated-orient-1.jpg",
        "bad-truncated-rgba.png",
        "bad-truncated-anim.gif",
        "bad-truncated-still.webp",
        "bad-truncated-anim.webp",
        "bad-truncated-anim-apng.png",
        "bad-truncated-still.heic",
        "bad-truncated-still.avif",
        "bad-truncated-still.jxl",
        "bad-truncated-still.tif",
        "bad-truncated-still.bmp",
        "bad-truncated-vector.svg",
        "bad-truncated-still.exr",
    ];
    for name in names {
        for options in [DecodeOptions::default(), screen(320, 240, 0)] {
            let started = Instant::now();
            // Ok (a partial picture) is acceptable for some formats; a panic or hang is not.
            let result = decode_file(&path(name), &options);
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "{name} took {:?}",
                started.elapsed()
            );
            if let Err(DecodeError::Panicked(format)) = result {
                panic!("{name}: the {format} decoder panicked");
            }
        }
    }
    assert!(matches!(open("bad-empty.jpg"), Err(DecodeError::Empty)));
    assert!(matches!(
        open("bad-random.png"),
        Err(DecodeError::UnknownFormat)
    ));
    // The PNG "bomb" is only a header: the decoder may call it too large or incomplete.
    assert!(matches!(
        open("bad-bomb.png"),
        Err(DecodeError::TooLarge { .. } | DecodeError::Corrupt { .. })
    ));
    // Header-only "bombs": too large, or incomplete — never an allocation of gigabytes.
    for options in [DecodeOptions::default(), screen(320, 240, 0)] {
        assert!(matches!(
            decode_file(&path("bad-bomb.jpg"), &options),
            Err(DecodeError::TooLarge { .. } | DecodeError::Corrupt { .. })
        ));
    }
    assert!(matches!(
        open("does-not-exist.jpg"),
        Err(DecodeError::Io(_))
    ));
}

#[test]
fn svg_bomb_is_rendered_small_instead_of_allocating_gigabytes() {
    let image = open_ok("bad-bomb.svg");
    assert_eq!(image.source_size, Size::new(100_000, 100_000));
    assert!(image.first().width() <= 8_192);
    assert!(image.first().byte_size() <= DecodeOptions::default().max_rgba_bytes);
}

#[test]
fn memory_limit_is_enforced() {
    let tiny = DecodeOptions {
        max_rgba_bytes: 1000,
        ..DecodeOptions::default()
    };
    for name in [
        "orient-1.jpg",
        "rgba.png",
        "still.avif",
        "still.jxl",
        "still.webp",
    ] {
        assert!(
            matches!(
                decode_file(&path(name), &tiny),
                Err(DecodeError::TooLarge { .. })
            ),
            "{name}"
        );
    }
}

#[test]
fn local_files_referenced_by_svg_are_not_loaded() {
    // vector.svg references file:///etc/passwd; it must render without reading it.
    let image = open_ok("vector.svg");
    assert_eq!(image.format, Format::Svg);
}
