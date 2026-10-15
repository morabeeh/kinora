//! The settings model, stored as a human-readable TOML file in the OS config directory
//! (brief §5.9). Reading and writing the file is the app's job; this module
//! only converts between text and the model, so it stays pure and testable.
//!
//! Every field has a default, and unknown or missing keys are fine, so a settings file
//! from an older or newer Kinora version still loads.

use serde::{Deserialize, Serialize};

use crate::folder::SortOrder;

/// All settings. Phase 4 adds a Settings page; until then the file can be edited by hand.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Settings {
    /// Viewer behaviour.
    pub viewer: ViewerSettings,
    /// App-wide behaviour.
    pub general: GeneralSettings,
    /// Renderer, Lite mode, memory.
    pub performance: PerformanceSettings,
    /// Hardware facts found at run time and remembered for the next start.
    pub detected: DetectedHardware,
}

/// What the mouse wheel does without Ctrl.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WheelAction {
    /// Zoom (default, brief §14 answer).
    #[default]
    Zoom,
    /// Previous / next item.
    Navigate,
}

/// Background behind transparent images.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TransparencyBackground {
    /// Grey checkerboard (default).
    #[default]
    Checkerboard,
    /// Solid light grey.
    Light,
    /// Solid dark grey.
    Dark,
}

/// Viewer settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct ViewerSettings {
    /// Mouse wheel behaviour (Ctrl+wheel always zooms).
    pub wheel: WheelAction,
    /// Wrap around at the ends of the folder.
    pub wrap_around: bool,
    /// Enlarge images smaller than the window to fit it.
    pub enlarge_small_images: bool,
    /// Folder sort order.
    pub sort: SortOrder,
    /// Background for transparent images.
    pub transparency: TransparencyBackground,
}

impl Default for ViewerSettings {
    fn default() -> Self {
        Self {
            wheel: WheelAction::Zoom,
            wrap_around: false,
            enlarge_small_images: false,
            sort: SortOrder::default(),
            transparency: TransparencyBackground::Checkerboard,
        }
    }
}

/// App-wide settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct GeneralSettings {
    /// Reuse the open window when another file is opened (like Windows Photos).
    pub single_instance: bool,
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self {
            single_instance: true,
        }
    }
}

/// Which UI renderer to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RendererChoice {
    /// GPU if usable, otherwise CPU (default).
    #[default]
    Auto,
    /// Always the GPU renderer (falls back to CPU if it can't start).
    Hardware,
    /// Always the CPU renderer.
    Software,
}

/// Lite mode switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LiteModeChoice {
    /// On for Lite-tier PCs and the software renderer (default).
    #[default]
    Auto,
    /// Always on.
    On,
    /// Always off.
    Off,
}

/// Performance settings.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct PerformanceSettings {
    /// UI renderer.
    pub renderer: RendererChoice,
    /// Lite mode.
    pub lite_mode: LiteModeChoice,
    /// Decoded-image cache size in MB; absent = automatic from installed RAM.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_cache_mb: Option<u32>,
}

/// Facts found while running that are expensive or impossible to find before the window
/// exists (the OpenGL renderer name on Linux). Stamped with the app version that found them,
/// so an update re-checks.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct DetectedHardware {
    /// Kinora version that wrote these values.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_version: Option<String>,
    /// OpenGL renderer string, e.g. "Mesa Intel(R) UHD Graphics 620" or "llvmpipe".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gl_renderer: Option<String>,
    /// `true` if that renderer is a software rasteriser.
    pub software_gl: bool,
}

/// The settings file could not be read or written.
#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    /// The file is not valid TOML or has a value of the wrong type.
    #[error("the settings file is not valid: {0}")]
    Parse(#[from] toml::de::Error),
    /// The settings could not be turned into TOML (a bug).
    #[error("could not write settings: {0}")]
    Serialize(#[from] toml::ser::Error),
}

impl Settings {
    /// Parses a settings file.
    ///
    /// # Errors
    /// [`SettingsError::Parse`] if the text is not valid TOML or a value has the wrong type.
    /// Missing keys are not an error (they get their defaults).
    pub fn from_toml(text: &str) -> Result<Self, SettingsError> {
        Ok(toml::from_str(text)?)
    }

    /// Writes the settings as TOML.
    ///
    /// # Errors
    /// [`SettingsError::Serialize`] if serialisation fails (should not happen).
    pub fn to_toml(&self) -> Result<String, SettingsError> {
        Ok(toml::to_string_pretty(self)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::folder::SortKey;

    #[test]
    fn defaults_match_the_brief() {
        let s = Settings::default();
        assert_eq!(s.viewer.wheel, WheelAction::Zoom);
        assert!(!s.viewer.wrap_around);
        assert!(!s.viewer.enlarge_small_images);
        assert!(s.general.single_instance);
        assert_eq!(s.performance.renderer, RendererChoice::Auto);
        assert_eq!(s.performance.lite_mode, LiteModeChoice::Auto);
        assert_eq!(s.performance.image_cache_mb, None);
    }

    #[test]
    fn round_trip() {
        let mut s = Settings::default();
        s.viewer.wheel = WheelAction::Navigate;
        s.viewer.sort.key = SortKey::DateTaken;
        s.viewer.sort.descending = true;
        s.performance.image_cache_mb = Some(300);
        s.detected.gl_renderer = Some(String::from("llvmpipe (LLVM 17.0.6, 256 bits)"));
        s.detected.software_gl = true;
        let text = s.to_toml().unwrap();
        assert!(text.contains("wheel = \"navigate\""), "{text}");
        assert!(text.contains("key = \"date-taken\""), "{text}");
        assert_eq!(Settings::from_toml(&text).unwrap(), s);
    }

    #[test]
    fn missing_and_unknown_keys_are_fine() {
        let s = Settings::from_toml(
            "[viewer]\nwrap-around = true\nfuture-option = 3\n[something-new]\nx = 1\n",
        )
        .unwrap();
        assert!(s.viewer.wrap_around);
        assert_eq!(s.viewer.wheel, WheelAction::Zoom);
        assert_eq!(Settings::from_toml("").unwrap(), Settings::default());
    }

    #[test]
    fn wrong_types_are_reported() {
        assert!(Settings::from_toml("[viewer]\nwrap-around = \"yes\"").is_err());
        assert!(Settings::from_toml("[performance]\nrenderer = \"vulkan\"").is_err());
        assert!(Settings::from_toml("not toml at all [").is_err());
    }
}
