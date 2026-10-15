//! Natural ("human") file-name ordering, close to Windows Explorer and Nemo:
//! `img2.jpg` sorts before `img10.jpg`, case is ignored, and `photo.jpg` comes before
//! `photo (2).jpg` because the name without its extension is compared first.
//!
//! The order is **total**: two different strings never compare as equal (ties such as
//! `File1` / `file1` or `a1` / `a01` are broken consistently), so sorting is deterministic.

use std::cmp::Ordering;

/// Compares two file names the way a file manager sorts them.
///
/// Rules, in priority order:
/// 1. The stem (the name before the last `.`) is compared first, then the extension.
/// 2. Runs of ASCII digits compare by numeric value, of any length (no overflow).
/// 3. Other characters compare case-insensitively; punctuation and spaces come before
///    digits, which come before letters.
/// 4. Ties: fewer leading zeros first (`a1` before `a01`), then plain byte order.
///
/// ```
/// use kinora_core::natural_sort::natural_cmp;
/// let mut names = vec!["img10.jpg", "IMG2.jpg", "img1.jpg"];
/// names.sort_by(|a, b| natural_cmp(a, b));
/// assert_eq!(names, ["img1.jpg", "IMG2.jpg", "img10.jpg"]);
/// ```
#[must_use]
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (stem_a, ext_a) = split_extension(a);
    let (stem_b, ext_b) = split_extension(b);
    // `zeros` remembers the first leading-zero difference, used only if all else is equal.
    let mut zeros = Ordering::Equal;
    let stem = compare_text(stem_a, stem_b, &mut zeros);
    if stem != Ordering::Equal {
        return stem;
    }
    let ext = compare_text(ext_a, ext_b, &mut zeros);
    if ext != Ordering::Equal {
        return ext;
    }
    zeros.then_with(|| a.cmp(b))
}

/// `"photo.tar.gz"` → (`"photo.tar"`, `"gz"`). A leading dot is part of the stem
/// (`".hidden"` has no extension).
fn split_extension(name: &str) -> (&str, &str) {
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => (stem, ext),
        _ => (name, ""),
    }
}

/// Character classes, in sort order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Class {
    Punctuation,
    Digit,
    Other,
}

fn key(c: char) -> (Class, char) {
    let class = if c.is_ascii_digit() {
        Class::Digit
    } else if c.is_alphanumeric() {
        Class::Other
    } else {
        Class::Punctuation
    };
    (class, c.to_lowercase().next().unwrap_or(c))
}

/// Compares two strings with numeric runs. `zeros` records the first leading-zero
/// difference between numerically equal runs (only if it is still `Equal`).
fn compare_text(a: &str, b: &str, zeros: &mut Ordering) -> Ordering {
    let (mut rest_a, mut rest_b) = (a, b);
    loop {
        let (Some(ca), Some(cb)) = (rest_a.chars().next(), rest_b.chars().next()) else {
            // One (or both) ran out: the shorter string sorts first.
            return rest_a.len().cmp(&rest_b.len());
        };
        if ca.is_ascii_digit() && cb.is_ascii_digit() {
            let (run_a, after_a) = split_digits(rest_a);
            let (run_b, after_b) = split_digits(rest_b);
            let ordering = compare_numbers(run_a, run_b);
            if ordering != Ordering::Equal {
                return ordering;
            }
            if *zeros == Ordering::Equal {
                *zeros = run_a.len().cmp(&run_b.len());
            }
            (rest_a, rest_b) = (after_a, after_b);
        } else {
            let ordering = key(ca).cmp(&key(cb));
            if ordering != Ordering::Equal {
                return ordering;
            }
            rest_a = rest_a.get(ca.len_utf8()..).unwrap_or_default();
            rest_b = rest_b.get(cb.len_utf8()..).unwrap_or_default();
        }
    }
}

/// Splits off the leading run of ASCII digits.
fn split_digits(s: &str) -> (&str, &str) {
    let end = s
        .bytes()
        .position(|byte| !byte.is_ascii_digit())
        .unwrap_or(s.len());
    s.split_at_checked(end).unwrap_or((s, ""))
}

/// Numeric comparison of two digit runs of any length: strip leading zeros, then a longer
/// number is bigger, and equal lengths compare digit by digit.
fn compare_numbers(a: &str, b: &str) -> Ordering {
    let a = a.trim_start_matches('0');
    let b = b.trim_start_matches('0');
    a.len().cmp(&b.len()).then_with(|| a.cmp(b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn sorted(names: &[&str]) -> Vec<String> {
        let mut out: Vec<String> = names.iter().map(|s| (*s).to_owned()).collect();
        out.sort_by(|a, b| natural_cmp(a, b));
        out
    }

    #[test]
    fn numbers_sort_by_value() {
        assert_eq!(
            sorted(&["img10.jpg", "img2.jpg", "img1.jpg", "img100.jpg"]),
            ["img1.jpg", "img2.jpg", "img10.jpg", "img100.jpg"]
        );
    }

    #[test]
    fn case_is_ignored() {
        assert_eq!(
            sorted(&["b.jpg", "A.jpg", "c.jpg"]),
            ["A.jpg", "b.jpg", "c.jpg"]
        );
    }

    #[test]
    fn stem_is_compared_before_extension() {
        // Explorer: "New Text Document.txt" before "New Text Document (2).txt".
        assert_eq!(
            sorted(&["photo (2).jpg", "photo.jpg", "photo 1.png"]),
            ["photo.jpg", "photo (2).jpg", "photo 1.png"]
        );
        assert_eq!(sorted(&["a.png", "a.jpg"]), ["a.jpg", "a.png"]);
    }

    #[test]
    fn punctuation_before_digits_before_letters() {
        assert_eq!(
            sorted(&["ab", "a1", "a_", "a b"]),
            ["a b", "a_", "a1", "ab"]
        );
    }

    #[test]
    fn very_long_numbers_do_not_overflow() {
        assert_eq!(
            sorted(&[
                "x100000000000000000000000000",
                "x99999999999999999999999999"
            ]),
            [
                "x99999999999999999999999999",
                "x100000000000000000000000000"
            ]
        );
    }

    #[test]
    fn ties_are_broken_deterministically() {
        assert_eq!(natural_cmp("a1", "a01"), Ordering::Less);
        assert_eq!(natural_cmp("a01", "a1"), Ordering::Greater);
        assert_ne!(natural_cmp("File1", "file1"), Ordering::Equal);
        assert_eq!(natural_cmp("same.jpg", "same.jpg"), Ordering::Equal);
    }

    #[test]
    fn hidden_files_and_no_extension() {
        assert_eq!(split_extension(".bashrc"), (".bashrc", ""));
        assert_eq!(split_extension("archive.tar.gz"), ("archive.tar", "gz"));
        assert_eq!(split_extension("README"), ("README", ""));
        assert_eq!(sorted(&["b", ".a", "a"]), [".a", "a", "b"]);
    }

    #[test]
    fn non_ascii_names() {
        assert_eq!(
            sorted(&["Ölbild 2.jpg", "ölbild 10.jpg", "Ölbild 1.jpg"]),
            ["Ölbild 1.jpg", "Ölbild 2.jpg", "ölbild 10.jpg"]
        );
        assert_ne!(natural_cmp("写真2", "写真10"), Ordering::Greater);
    }

    proptest! {
        #[test]
        fn equal_only_when_identical(a in "\\PC{0,12}", b in "\\PC{0,12}") {
            prop_assert_eq!(natural_cmp(&a, &b) == Ordering::Equal, a == b);
        }

        #[test]
        fn antisymmetric(a in "[a-cA-C0-9 ._()-]{0,10}", b in "[a-cA-C0-9 ._()-]{0,10}") {
            prop_assert_eq!(natural_cmp(&a, &b), natural_cmp(&b, &a).reverse());
        }

        #[test]
        fn transitive(
            a in "[a-bA-B0-9 .]{0,8}",
            b in "[a-bA-B0-9 .]{0,8}",
            c in "[a-bA-B0-9 .]{0,8}",
        ) {
            let mut v = [a, b, c];
            v.sort_by(|x, y| natural_cmp(x, y));
            prop_assert_ne!(natural_cmp(&v[0], &v[1]), Ordering::Greater);
            prop_assert_ne!(natural_cmp(&v[1], &v[2]), Ordering::Greater);
            prop_assert_ne!(natural_cmp(&v[0], &v[2]), Ordering::Greater);
        }

        #[test]
        fn numbers_follow_numeric_order(n in 0u64..1_000_000_000, m in 0u64..1_000_000_000) {
            let a = format!("IMG_{n}.jpg");
            let b = format!("img_{m}.jpg");
            if n != m {
                prop_assert_eq!(natural_cmp(&a, &b), n.cmp(&m));
            }
        }
    }
}
