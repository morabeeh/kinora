//! Listing the photos and videos next to the opened file (brief §5.3, §7.6 "Disk").
//!
//! Runs on its own thread. Only directory entries are read — never file contents — and on
//! Linux file sizes/dates are read only when the sort order needs them (each one costs a
//! disk access on a slow HDD). Hidden files are skipped, and symbolic links are followed
//! only when they stay inside the folder (brief §11).

use std::path::{Path, PathBuf};

use kinora_core::folder::{MediaItem, SortKey};
use kinora_core::media::{MediaKind, kind_from_file_name};

/// Lists `folder`, always including `opened` (the file the user asked for, even if its
/// extension is unusual — or if it doesn't exist any more, so the viewer can say so
/// instead of silently showing a different picture).
pub fn scan(
    folder: &Path,
    opened: Option<&Path>,
    sort: SortKey,
) -> std::io::Result<Vec<MediaItem>> {
    let needs_metadata = cfg!(windows) || sort != SortKey::Name;
    let canonical_folder = std::fs::canonicalize(folder).ok();
    let mut items = Vec::new();
    for entry in std::fs::read_dir(folder)? {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        let is_opened = opened.is_some_and(|opened| opened == path);
        let name = entry.file_name().to_string_lossy().into_owned();
        if !is_opened && name.starts_with('.') {
            continue;
        }
        let Some(kind) = kind_from_file_name(&name) else {
            continue;
        };
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            if !is_opened && !link_stays_inside(&path, canonical_folder.as_deref()) {
                continue;
            }
        } else if !file_type.is_file() {
            continue;
        }
        let mut item = MediaItem::new(path, kind);
        if needs_metadata && let Ok(metadata) = entry.metadata() {
            if is_hidden(&metadata) && !is_opened {
                continue;
            }
            item.size = metadata.len();
            item.modified = metadata.modified().ok();
        }
        items.push(item);
    }
    if let Some(opened) = opened
        && !items.iter().any(|item| item.path == opened)
    {
        let kind = opened
            .file_name()
            .and_then(|name| kind_from_file_name(&name.to_string_lossy()))
            .unwrap_or(MediaKind::Image);
        items.push(MediaItem::new(opened.to_path_buf(), kind));
    }
    Ok(items)
}

/// A link inside the folder that points at a file inside the same folder.
fn link_stays_inside(link: &Path, folder: Option<&Path>) -> bool {
    let (Ok(target), Some(folder)) = (std::fs::canonicalize(link), folder) else {
        return false;
    };
    target.parent() == Some(folder) && target.is_file()
}

#[cfg(windows)]
fn is_hidden(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    metadata.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0
}

#[cfg(not(windows))]
fn is_hidden(_metadata: &std::fs::Metadata) -> bool {
    false // Unix: the dot-prefix rule above
}

/// The folder to list for an opened path: its parent folder, or the path itself if it is a
/// folder (a folder dropped onto the window).
pub fn folder_of(path: &Path) -> Option<PathBuf> {
    if path.is_dir() {
        Some(path.to_path_buf())
    } else {
        path.parent().map(Path::to_path_buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);
    impl TempDir {
        fn new(tag: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("kinora-scan-{tag}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
        fn touch(&self, name: &str) -> PathBuf {
            let path = self.0.join(name);
            std::fs::write(&path, b"x").unwrap();
            path
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn names(items: &[MediaItem]) -> Vec<String> {
        let mut names: Vec<String> = items.iter().map(|item| item.name.clone()).collect();
        names.sort();
        names
    }

    #[test]
    fn lists_media_only_and_skips_hidden_files() {
        let dir = TempDir::new("media");
        dir.touch("a.jpg");
        dir.touch("b.MKV");
        dir.touch("notes.txt");
        dir.touch(".hidden.jpg");
        std::fs::create_dir(dir.0.join("sub.jpg")).unwrap();
        let items = scan(&dir.0, None, SortKey::Name).unwrap();
        assert_eq!(names(&items), ["a.jpg", "b.MKV"]);
        let video = items.iter().find(|i| i.name == "b.MKV").unwrap();
        assert_eq!(video.kind, MediaKind::Video);
    }

    #[test]
    fn the_opened_file_is_always_listed() {
        let dir = TempDir::new("opened");
        dir.touch("a.jpg");
        let odd = dir.touch("picture.unknownext");
        let hidden = dir.touch(".secret.png");
        let items = scan(&dir.0, Some(&odd), SortKey::Name).unwrap();
        assert_eq!(names(&items), ["a.jpg", "picture.unknownext"]);
        let items = scan(&dir.0, Some(&hidden), SortKey::Name).unwrap();
        assert_eq!(names(&items), [".secret.png", "a.jpg"]);
    }

    #[test]
    fn a_missing_opened_file_stays_in_the_list() {
        let dir = TempDir::new("missing");
        dir.touch("a.jpg");
        let missing = dir.0.join("gone.jpg");
        let items = scan(&dir.0, Some(&missing), SortKey::Name).unwrap();
        assert_eq!(names(&items), ["a.jpg", "gone.jpg"]);
    }

    #[test]
    fn metadata_is_read_for_size_sorting() {
        let dir = TempDir::new("meta");
        dir.touch("a.jpg");
        let items = scan(&dir.0, None, SortKey::Size).unwrap();
        assert_eq!(items[0].size, 1);
        assert!(items[0].modified.is_some());
    }

    #[test]
    fn folder_of_a_file_and_a_folder() {
        let dir = TempDir::new("folder-of");
        let file = dir.touch("a.jpg");
        assert_eq!(folder_of(&file), Some(dir.0.clone()));
        assert_eq!(folder_of(&dir.0), Some(dir.0.clone()));
    }
}
