//! Choosing the UI renderer and Lite mode at startup (brief §7.6, ADR 0004).
//!
//! Renderer chain per OS: Windows = Skia (GPU) → Slint software; Linux = `FemtoVG` (OpenGL,
//! provisional until the Mint PC measurements) → software; macOS = `FemtoVG` → software.
//! If the GPU renderer can't start, Slint's winit backend itself falls back to the next
//! compiled-in renderer, so a broken GPU driver never stops Kinora from opening.

use anyhow::Context as _;
use kinora_core::hardware::{Gpu, Tier};
use kinora_core::settings::{LiteModeChoice, RendererChoice};

/// Slint's name for the GPU renderer on this OS.
pub const GPU_RENDERER: &str = if cfg!(windows) { "skia" } else { "femtovg" };
/// Slint's CPU-only renderer.
pub const SOFTWARE_RENDERER: &str = "software";

/// The renderer we will ask Slint for, and why (for the log).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererPlan {
    /// `true` for the CPU renderer.
    pub software: bool,
    /// Human-readable reason, logged at startup.
    pub reason: String,
}

impl RendererPlan {
    /// Slint renderer name.
    pub fn slint_name(&self) -> &'static str {
        if self.software {
            SOFTWARE_RENDERER
        } else {
            GPU_RENDERER
        }
    }
}

/// Picks the renderer: command line, then settings, then automatic (software when the PC
/// has no usable GPU).
pub fn choose_renderer(
    from_args: Option<RendererChoice>,
    from_settings: RendererChoice,
    gpu: &Gpu,
) -> RendererPlan {
    let (choice, source) = match from_args {
        Some(choice) => (choice, "command line"),
        None => (from_settings, "settings"),
    };
    match choice {
        RendererChoice::Software => RendererPlan {
            software: true,
            reason: format!("software renderer chosen in the {source}"),
        },
        RendererChoice::Hardware => RendererPlan {
            software: false,
            reason: format!("GPU renderer chosen in the {source}"),
        },
        RendererChoice::Auto => match gpu {
            Gpu::SoftwareOnly(name) => RendererPlan {
                software: true,
                reason: format!(
                    "no usable GPU ({}): software renderer",
                    name.as_deref().unwrap_or("unknown")
                ),
            },
            Gpu::Hardware(name) => RendererPlan {
                software: false,
                reason: format!("GPU: {}", name.as_deref().unwrap_or("unknown")),
            },
            Gpu::Unknown => RendererPlan {
                software: false,
                reason: String::from("GPU not known yet: trying the GPU renderer"),
            },
        },
    }
}

/// Lite mode: forced on by `--lite`, else the setting, else automatic (Lite-tier PC or
/// software rendering).
pub fn lite_mode(forced: bool, setting: LiteModeChoice, tier: Tier, software: bool) -> bool {
    forced
        || match setting {
            LiteModeChoice::On => true,
            LiteModeChoice::Off => false,
            LiteModeChoice::Auto => tier == Tier::Lite || software,
        }
}

/// Activates Slint's winit backend with the chosen renderer. `SLINT_BACKEND` in the
/// environment always wins (developer override, documented in CLAUDE.md).
pub fn select_backend(plan: &RendererPlan) -> anyhow::Result<()> {
    let selector = slint::BackendSelector::new().backend_name(String::from("winit"));
    let selector = if let Ok(value) = std::env::var("SLINT_BACKEND") {
        tracing::info!(SLINT_BACKEND = %value, "renderer chosen by the environment");
        selector
    } else {
        tracing::info!(renderer = plan.slint_name(), reason = %plan.reason, "UI renderer");
        selector.renderer_name(String::from(plan.slint_name()))
    };
    selector
        .select()
        .context("could not start the window system (winit backend)")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automatic_choice_follows_the_gpu() {
        let no_gpu = Gpu::SoftwareOnly(Some(String::from("Microsoft Basic Render Driver")));
        assert!(choose_renderer(None, RendererChoice::Auto, &no_gpu).software);
        let gpu = Gpu::Hardware(Some(String::from("Intel(R) Graphics")));
        assert!(!choose_renderer(None, RendererChoice::Auto, &gpu).software);
        assert!(!choose_renderer(None, RendererChoice::Auto, &Gpu::Unknown).software);
    }

    #[test]
    fn command_line_beats_settings() {
        let gpu = Gpu::Hardware(None);
        let plan = choose_renderer(
            Some(RendererChoice::Software),
            RendererChoice::Hardware,
            &gpu,
        );
        assert!(plan.software);
        assert_eq!(plan.slint_name(), SOFTWARE_RENDERER);
        let plan = choose_renderer(None, RendererChoice::Software, &gpu);
        assert!(plan.reason.contains("settings"));
    }

    #[test]
    fn lite_mode_rules() {
        assert!(lite_mode(false, LiteModeChoice::Auto, Tier::Lite, false));
        assert!(lite_mode(false, LiteModeChoice::Auto, Tier::High, true));
        assert!(!lite_mode(
            false,
            LiteModeChoice::Auto,
            Tier::Typical,
            false
        ));
        assert!(!lite_mode(false, LiteModeChoice::Off, Tier::Lite, true));
        assert!(lite_mode(false, LiteModeChoice::On, Tier::High, false));
        assert!(lite_mode(true, LiteModeChoice::Off, Tier::High, false));
    }
}
