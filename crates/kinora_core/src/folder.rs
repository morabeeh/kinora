//! The list of photos and videos in one folder, its sort order, and the current item.

use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::media::MediaKind;
use crate::natural_sort::natural_cmp;

/// One photo or video in the folder list. Filled from directory metadata only — listing a
/// folder never reads file contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaItem {
    /// Full path of the file.
    pub path: PathBuf,
    /// File name for display and sorting (lossy UTF-8 for odd Linux names).
    pub name: String,
    /// Photo or video.
    pub kind: MediaKind,
    /// Size in bytes (0 if unknown).
    pub size: u64,
    /// Last-modified time, if the file system reports one.
    pub modified: Option<SystemTime>,
    /// EXIF "date taken", once it has been read (lazily, in the background).
    pub date_taken: Option<SystemTime>,
}

impl MediaItem {
    /// An item with only a path and kind; size and dates unknown.
    #[must_use]
    pub fn new(path: PathBuf, kind: MediaKind) -> Self {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        Self {
            path,
            name,
            kind,
            size: 0,
            modified: None,
            date_taken: None,
        }
    }

    /// The extension without the dot (`""` if there is none).
    #[must_use]
    pub fn extension(&self) -> &str {
        match self.name.rsplit_once('.') {
            Some((stem, ext)) if !stem.is_empty() => ext,
            _ => "",
        }
    }
}

/// What the folder is sorted by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SortKey {
    /// Natural sort by file name, like the file manager (default).
    #[default]
    Name,
    /// Last-modified time.
    Modified,
    /// EXIF date taken, falling back to the modified time.
    DateTaken,
    /// File size.
    Size,
    /// File type (extension), then name.
    Type,
}

/// Sort key plus direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct SortOrder {
    /// What to sort by.
    pub key: SortKey,
    /// `true` for descending order.
    pub descending: bool,
}

/// Compares two items for `order`. Ties fall back to the natural name order and then the
/// full path, so the order is always total and stable across runs.
#[must_use]
pub fn compare_items(a: &MediaItem, b: &MediaItem, order: SortOrder) -> Ordering {
    let primary = match order.key {
        SortKey::Name => Ordering::Equal,
        SortKey::Modified => a.modified.cmp(&b.modified),
        SortKey::DateTaken => a
            .date_taken
            .or(a.modified)
            .cmp(&b.date_taken.or(b.modified)),
        SortKey::Size => a.size.cmp(&b.size),
        SortKey::Type => natural_cmp(a.extension(), b.extension()),
    };
    let ordering = primary
        .then_with(|| natural_cmp(&a.name, &b.name))
        .then_with(|| a.path.cmp(&b.path));
    if order.descending {
        ordering.reverse()
    } else {
        ordering
    }
}

/// The folder list and which item is current.
#[derive(Debug, Clone, Default)]
pub struct FolderModel {
    items: Vec<MediaItem>,
    current: Option<usize>,
    order: SortOrder,
}

impl FolderModel {
    /// An empty list.
    #[must_use]
    pub fn new(order: SortOrder) -> Self {
        Self {
            items: Vec::new(),
            current: None,
            order,
        }
    }

    /// A list holding only the file the user opened. Used while the folder is still being
    /// scanned, so the opened file shows immediately (brief §7.5).
    #[must_use]
    pub fn with_single(item: MediaItem, order: SortOrder) -> Self {
        Self {
            items: vec![item],
            current: Some(0),
            order,
        }
    }

    /// Number of items.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// `true` when there are no items.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// All items in display order.
    #[must_use]
    pub fn items(&self) -> &[MediaItem] {
        &self.items
    }

    /// The item at `index`, if any.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&MediaItem> {
        self.items.get(index)
    }

    /// Index of the current item.
    #[must_use]
    pub fn current_index(&self) -> Option<usize> {
        self.current
    }

    /// The current item.
    #[must_use]
    pub fn current(&self) -> Option<&MediaItem> {
        self.current.and_then(|index| self.items.get(index))
    }

    /// The sort order in use.
    #[must_use]
    pub fn order(&self) -> SortOrder {
        self.order
    }

    /// Index of the item with this path.
    #[must_use]
    pub fn position_of(&self, path: &Path) -> Option<usize> {
        self.items.iter().position(|item| item.path == path)
    }

    /// Makes `index` current. Returns `false` (and changes nothing) if it is out of range.
    pub fn set_current(&mut self, index: usize) -> bool {
        if index < self.items.len() {
            self.current = Some(index);
            true
        } else {
            false
        }
    }

    /// Replaces the whole list (after a folder scan or a change on disk) and sorts it.
    ///
    /// The current item stays current, found by path. If it is gone, the item that now sits
    /// where it would have been becomes current (like a file manager after a delete).
    pub fn replace_items(&mut self, mut items: Vec<MediaItem>) {
        let order = self.order;
        items.sort_by(|a, b| compare_items(a, b, order));
        let previous = self.current().cloned();
        self.items = items;
        self.current = match previous {
            None => (!self.items.is_empty()).then_some(0),
            Some(old) => self.position_of(&old.path).or_else(|| {
                let insert_at = self
                    .items
                    .partition_point(|item| compare_items(item, &old, order) == Ordering::Less);
                (!self.items.is_empty()).then(|| insert_at.min(self.items.len() - 1))
            }),
        };
    }

    /// Changes the sort order, keeping the current item.
    pub fn set_order(&mut self, order: SortOrder) {
        let current_path = self.current().map(|item| item.path.clone());
        self.order = order;
        self.items.sort_by(|a, b| compare_items(a, b, order));
        self.current = current_path.and_then(|path| self.position_of(&path));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn item(name: &str) -> MediaItem {
        MediaItem::new(PathBuf::from("/photos").join(name), MediaKind::Image)
    }

    fn names(model: &FolderModel) -> Vec<&str> {
        model
            .items()
            .iter()
            .map(|item| item.name.as_str())
            .collect()
    }

    #[test]
    fn scan_result_keeps_the_opened_file_current() {
        let mut model = FolderModel::with_single(item("img10.jpg"), SortOrder::default());
        model.replace_items(vec![item("img2.jpg"), item("img10.jpg"), item("img1.jpg")]);
        assert_eq!(names(&model), ["img1.jpg", "img2.jpg", "img10.jpg"]);
        assert_eq!(model.current_index(), Some(2));
    }

    #[test]
    fn removed_current_moves_to_its_neighbour() {
        let mut model = FolderModel::new(SortOrder::default());
        model.replace_items(vec![item("a.jpg"), item("b.jpg"), item("c.jpg")]);
        assert!(model.set_current(1));
        model.replace_items(vec![item("a.jpg"), item("c.jpg")]);
        assert_eq!(model.current().map(|i| i.name.as_str()), Some("c.jpg"));
        // Removing the last item selects the new last one.
        assert!(model.set_current(1));
        model.replace_items(vec![item("a.jpg")]);
        assert_eq!(model.current_index(), Some(0));
        model.replace_items(Vec::new());
        assert_eq!(model.current_index(), None);
    }

    #[test]
    fn empty_model_gets_a_current_item_after_a_scan() {
        let mut model = FolderModel::new(SortOrder::default());
        assert!(!model.set_current(0));
        model.replace_items(vec![item("b.jpg"), item("a.jpg")]);
        assert_eq!(model.current().map(|i| i.name.as_str()), Some("a.jpg"));
    }

    #[test]
    fn sort_by_size_and_type_with_name_tiebreak() {
        let mut big = item("b.png");
        big.size = 300;
        let mut small = item("a.png");
        small.size = 100;
        let mut mid = item("c.jpg");
        mid.size = 200;
        let mut model = FolderModel::new(SortOrder {
            key: SortKey::Size,
            descending: true,
        });
        model.replace_items(vec![small.clone(), big.clone(), mid.clone()]);
        assert_eq!(names(&model), ["b.png", "c.jpg", "a.png"]);
        model.set_order(SortOrder {
            key: SortKey::Type,
            descending: false,
        });
        assert_eq!(names(&model), ["c.jpg", "a.png", "b.png"]);
    }

    #[test]
    fn date_taken_falls_back_to_modified() {
        let base = SystemTime::UNIX_EPOCH;
        let mut a = item("a.jpg");
        a.modified = Some(base + Duration::from_secs(50));
        let mut b = item("b.jpg");
        b.modified = Some(base + Duration::from_secs(100));
        b.date_taken = Some(base + Duration::from_secs(10));
        let mut model = FolderModel::new(SortOrder {
            key: SortKey::DateTaken,
            descending: false,
        });
        model.replace_items(vec![a, b]);
        assert_eq!(names(&model), ["b.jpg", "a.jpg"]);
    }

    #[test]
    fn set_order_keeps_current_item() {
        let mut model = FolderModel::new(SortOrder::default());
        model.replace_items(vec![item("a.jpg"), item("b.jpg"), item("c.jpg")]);
        assert!(model.set_current(0));
        model.set_order(SortOrder {
            key: SortKey::Name,
            descending: true,
        });
        assert_eq!(names(&model), ["c.jpg", "b.jpg", "a.jpg"]);
        assert_eq!(model.current_index(), Some(2));
    }

    #[test]
    fn extension_of_hidden_file_is_empty() {
        assert_eq!(item(".hidden").extension(), "");
        assert_eq!(item("x.JPG").extension(), "JPG");
    }
}
