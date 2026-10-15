//! Loading and saving `settings.toml` (brief §5.9). Saving is atomic, so a crash can never
//! leave a half-written settings file behind.

use std::path::{Path, PathBuf};

use kinora_core::settings::Settings;

/// The settings plus where they came from.
#[derive(Debug)]
pub struct SettingsStore {
    path: Option<PathBuf>,
    /// The loaded settings.
    pub settings: Settings,
    /// What is on disk, to save only real changes. `None` if the file was unreadable: we
    /// then never overwrite it, so a typo in a hand-edited file isn't silently erased.
    on_disk: Option<Settings>,
}

impl SettingsStore {
    /// Reads the settings file; a missing file gives the defaults.
    pub fn load(path: Option<PathBuf>) -> Self {
        let Some(file) = path.clone() else {
            return Self {
                path,
                settings: Settings::default(),
                on_disk: None,
            };
        };
        let (settings, on_disk) = match std::fs::read_to_string(&file) {
            Ok(text) => match Settings::from_toml(&text) {
                Ok(settings) => (settings.clone(), Some(settings)),
                Err(err) => {
                    tracing::warn!(file = %file.display(), %err, "settings file ignored; using defaults");
                    (Settings::default(), None)
                }
            },
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                (Settings::default(), Some(Settings::default()))
            }
            Err(err) => {
                tracing::warn!(file = %file.display(), %err, "could not read the settings file");
                (Settings::default(), None)
            }
        };
        Self {
            path,
            settings,
            on_disk,
        }
    }

    /// Writes the settings if they changed since loading (and the file was readable).
    pub fn save(&mut self) {
        let (Some(path), Some(on_disk)) = (&self.path, &self.on_disk) else {
            return;
        };
        if *on_disk == self.settings {
            return;
        }
        match self.write(path) {
            Ok(()) => self.on_disk = Some(self.settings.clone()),
            Err(err) => tracing::warn!(file = %path.display(), "could not save settings: {err:#}"),
        }
    }

    fn write(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = self.settings.to_toml()?;
        kinora_platform::fs::atomic_write(path, text.as_bytes())?;
        Ok(())
    }
}
