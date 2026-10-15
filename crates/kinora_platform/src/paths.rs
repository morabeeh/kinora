//! Where Kinora keeps its settings and logs (the OS-standard folders).
//!
//! | | Linux | Windows |
//! |---|---|---|
//! | settings | `~/.config/kinora/settings.toml` | `%APPDATA%\Kinora\config\settings.toml` |
//! | logs | `~/.local/state/kinora/logs/` | `%LOCALAPPDATA%\Kinora\data\logs\` |

use std::path::PathBuf;

/// Kinora's per-user folders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    /// Folder for `settings.toml`.
    pub config_dir: PathBuf,
    /// Folder for rotating log files.
    pub log_dir: PathBuf,
}

impl AppPaths {
    /// The settings file.
    #[must_use]
    pub fn settings_file(&self) -> PathBuf {
        self.config_dir.join("settings.toml")
    }
}

/// The folders for the current user, or `None` if the OS reports no home directory.
#[must_use]
pub fn app_paths() -> Option<AppPaths> {
    let dirs = directories::ProjectDirs::from("", "", "Kinora")?;
    let log_root = dirs.state_dir().unwrap_or_else(|| dirs.data_local_dir());
    Some(AppPaths {
        config_dir: dirs.config_dir().to_path_buf(),
        log_dir: log_root.join("logs"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_per_user_and_distinct() {
        let paths = app_paths().unwrap();
        assert!(paths.settings_file().ends_with("settings.toml"));
        assert_ne!(paths.config_dir, paths.log_dir);
        assert!(paths.config_dir.is_absolute() && paths.log_dir.is_absolute());
    }
}
