//! Colour management: convert pixels with an embedded ICC profile to sRGB (brief §5.2),
//! so photos from wide-gamut cameras and phones (Display P3, Adobe RGB) don't look dull.

use moxcms::{ColorProfile, DataColorSpace, Layout, ProfileText, TransformOptions};

use crate::image_buf::Rgba8Image;

/// Converts `image` from the colour space described by `icc` to sRGB. Profiles that are
/// already sRGB, not RGB (CMYK, grey) or unreadable are left alone: showing the picture
/// slightly off is better than not showing it.
#[must_use]
pub fn to_srgb(image: Rgba8Image, icc: &[u8]) -> Rgba8Image {
    let Ok(profile) = ColorProfile::new_from_slice(icc) else {
        tracing::debug!("unreadable ICC profile ignored");
        return image;
    };
    if profile.color_space != DataColorSpace::Rgb || is_srgb(&profile) {
        return image;
    }
    let srgb = ColorProfile::new_srgb();
    let transform = match profile.create_transform_8bit(
        Layout::Rgba,
        &srgb,
        Layout::Rgba,
        TransformOptions::default(),
    ) {
        Ok(transform) => transform,
        Err(err) => {
            tracing::debug!(%err, "ICC transform not possible; showing unconverted colours");
            return image;
        }
    };
    let (width, height) = (image.width(), image.height());
    let mut out = vec![0u8; image.data().len()];
    if let Err(err) = transform.transform(image.data(), &mut out) {
        tracing::debug!(%err, "ICC transform failed; showing unconverted colours");
        return image;
    }
    Rgba8Image::new(width, height, out).unwrap_or(image)
}

/// Most embedded profiles are sRGB itself; converting would only cost time.
fn is_srgb(profile: &ColorProfile) -> bool {
    let text = match &profile.description {
        Some(ProfileText::PlainString(text)) => text.clone(),
        Some(ProfileText::Localizable(strings)) => {
            strings.first().map(|s| s.value.clone()).unwrap_or_default()
        }
        Some(ProfileText::Description(desc)) => desc.ascii_string.clone(),
        None => String::new(),
    };
    text.to_ascii_lowercase().contains("srgb")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn garbage_profile_leaves_pixels_unchanged() {
        let img = Rgba8Image::filled(4, 4, [200, 100, 50, 255]).unwrap();
        assert_eq!(to_srgb(img.clone(), b"not an icc profile"), img);
    }

    #[test]
    fn srgb_profile_keeps_colours() {
        let img = Rgba8Image::filled(4, 4, [200, 100, 50, 255]).unwrap();
        let icc = ColorProfile::new_srgb().encode().unwrap();
        let out = to_srgb(img.clone(), &icc);
        for (a, b) in out.data().iter().zip(img.data()) {
            assert!(a.abs_diff(*b) <= 1, "{a} vs {b}");
        }
    }

    #[test]
    fn display_p3_red_becomes_a_more_saturated_srgb_red() {
        let p3 = ColorProfile::new_display_p3();
        let icc = p3.encode().unwrap();
        let img = Rgba8Image::filled(2, 2, [255, 0, 0, 255]).unwrap();
        let out = to_srgb(img, &icc);
        let [r, g, b, a] = out.pixel(0, 0).unwrap();
        assert_eq!((r, a), (255, 255));
        assert!(
            g < 10 && b < 10,
            "P3 red is outside sRGB and clips: {g} {b}"
        );
    }
}
