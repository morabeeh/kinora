//! File-system helpers that protect user data (brief §8.4).

use std::fs::OpenOptions;
use std::io::{self, Write as _};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

/// Writes `bytes` to `path` **atomically**: the data goes to a temporary file in the same
/// folder, is flushed to disk (`fsync`), and only then renamed over `path`. A crash or a
/// full disk at any moment leaves either the old file or the complete new one — never a
/// half-written file.
///
/// # Errors
/// Any I/O error; the original file is untouched and the temporary file is removed.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no file name"))?;
    let dir = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let temp = dir.join(format!(
        ".{}.{}-{}.tmp",
        name.to_string_lossy(),
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let result = write_and_rename(&temp, path, bytes);
    if result.is_err() {
        // Best effort: the temp file may not exist if creating it was what failed.
        let _ = std::fs::remove_file(&temp);
    } else {
        sync_dir(dir);
    }
    result
}

fn write_and_rename(temp: &Path, path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(temp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    // On Windows `rename` replaces an existing file (MoveFileEx with REPLACE_EXISTING).
    std::fs::rename(temp, path)
}

/// On Unix, also flush the folder entry so the rename itself survives a power cut.
fn sync_dir(dir: &Path) {
    #[cfg(unix)]
    if let Ok(handle) = std::fs::File::open(dir) {
        let _ = handle.sync_all();
    }
    #[cfg(not(unix))]
    let _ = dir;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A fresh folder under the system temp dir, removed on drop.
    struct TempDir(PathBuf);
    impl TempDir {
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "kinora-test-{tag}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn entries(dir: &Path) -> usize {
        std::fs::read_dir(dir).unwrap().count()
    }

    #[test]
    fn writes_new_and_replaces_existing_files() {
        let tmp = TempDir::new("atomic-new");
        let file = tmp.0.join("settings.toml");
        atomic_write(&file, b"one").unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), b"one");
        atomic_write(&file, b"two, longer").unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), b"two, longer");
        assert_eq!(entries(&tmp.0), 1, "no temp files left behind");
    }

    #[test]
    fn failure_leaves_the_original_untouched() {
        let tmp = TempDir::new("atomic-fail");
        // The target is a non-empty directory, so the final rename must fail.
        let target = tmp.0.join("target");
        std::fs::create_dir(&target).unwrap();
        std::fs::write(target.join("keep.txt"), b"precious").unwrap();
        assert!(atomic_write(&target, b"new data").is_err());
        assert_eq!(std::fs::read(target.join("keep.txt")).unwrap(), b"precious");
        assert_eq!(entries(&tmp.0), 1, "temp file was cleaned up");
    }

    #[test]
    fn rejects_paths_without_a_file_name() {
        assert!(atomic_write(Path::new(""), b"x").is_err());
    }
}
