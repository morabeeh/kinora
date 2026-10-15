//! Which neighbours to decode in the background, and in which order (brief §5.3).
//!
//! Preloading is direction-aware: when the user moves forward we look further ahead than
//! behind, and the other way round. The number of items comes from the hardware profile.

use crate::navigation::Direction;

/// How many items to preload on each side of the current one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreloadPolicy {
    /// Items to preload in the direction the user is moving.
    pub ahead: usize,
    /// Items to preload in the other direction.
    pub behind: usize,
}

/// One item to preload. Lower `priority` values are more urgent; the current item itself
/// is not included (it is always decoded first, with priority 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreloadTarget {
    /// Index in the folder list.
    pub index: usize,
    /// 1 = next in the moving direction, 2 = previous, 3 = second next, …
    pub priority: u8,
}

/// The items to preload around `current` in a list of `len` items.
///
/// ```
/// use kinora_core::navigation::Direction;
/// use kinora_core::preload::{preload_targets, PreloadPolicy};
/// let policy = PreloadPolicy { ahead: 2, behind: 1 };
/// let targets = preload_targets(10, 5, Direction::Forward, false, policy);
/// let indices: Vec<usize> = targets.iter().map(|t| t.index).collect();
/// assert_eq!(indices, [6, 4, 7]);
/// ```
#[must_use]
pub fn preload_targets(
    len: usize,
    current: usize,
    direction: Direction,
    wrap: bool,
    policy: PreloadPolicy,
) -> Vec<PreloadTarget> {
    let mut targets: Vec<PreloadTarget> = Vec::new();
    if current >= len {
        return targets;
    }
    let reach = policy.ahead.max(policy.behind);
    for distance in 1..=reach {
        let forward_first = direction == Direction::Forward;
        let candidates = [
            (distance <= policy.ahead, forward_first, 1),
            (distance <= policy.behind, !forward_first, 0),
        ];
        for (wanted, towards_end, rank) in candidates {
            if !wanted {
                continue;
            }
            let Some(index) = step(len, current, distance, towards_end, wrap) else {
                continue;
            };
            if index == current || targets.iter().any(|t| t.index == index) {
                continue;
            }
            // distance 1 ahead → 1, distance 1 behind → 2, distance 2 ahead → 3, …
            let priority = distance
                .saturating_mul(2)
                .saturating_sub(rank)
                .min(usize::from(u8::MAX));
            targets.push(PreloadTarget {
                index,
                priority: u8::try_from(priority).unwrap_or(u8::MAX),
            });
        }
    }
    targets
}

/// `current ± distance`, wrapping around if allowed.
fn step(
    len: usize,
    current: usize,
    distance: usize,
    towards_end: bool,
    wrap: bool,
) -> Option<usize> {
    if towards_end {
        let index = current.checked_add(distance)?;
        if index < len {
            Some(index)
        } else if wrap {
            Some(index % len)
        } else {
            None
        }
    } else if let Some(index) = current.checked_sub(distance) {
        Some(index)
    } else if wrap {
        let back = distance % len;
        Some((current + len - back) % len)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TYPICAL: PreloadPolicy = PreloadPolicy {
        ahead: 2,
        behind: 1,
    };
    const LITE: PreloadPolicy = PreloadPolicy {
        ahead: 1,
        behind: 0,
    };

    fn indices(targets: &[PreloadTarget]) -> Vec<usize> {
        targets.iter().map(|t| t.index).collect()
    }

    #[test]
    fn forward_looks_further_ahead() {
        let targets = preload_targets(10, 5, Direction::Forward, false, TYPICAL);
        assert_eq!(indices(&targets), [6, 4, 7]);
        let priorities: Vec<u8> = targets.iter().map(|t| t.priority).collect();
        assert_eq!(priorities, [1, 2, 3]);
    }

    #[test]
    fn backward_looks_further_behind() {
        let targets = preload_targets(10, 5, Direction::Backward, false, TYPICAL);
        assert_eq!(indices(&targets), [4, 6, 3]);
    }

    #[test]
    fn lite_preloads_one_item_in_the_moving_direction() {
        assert_eq!(
            indices(&preload_targets(10, 5, Direction::Backward, false, LITE)),
            [4]
        );
    }

    #[test]
    fn ends_are_clipped_without_wrap_and_wrap_with_it() {
        assert_eq!(
            indices(&preload_targets(10, 9, Direction::Forward, false, TYPICAL)),
            [8]
        );
        assert_eq!(
            indices(&preload_targets(10, 9, Direction::Forward, true, TYPICAL)),
            [0, 8, 1]
        );
        assert_eq!(
            indices(&preload_targets(10, 0, Direction::Backward, true, TYPICAL)),
            [9, 1, 8]
        );
    }

    #[test]
    fn tiny_lists_have_no_duplicates() {
        assert_eq!(
            indices(&preload_targets(2, 0, Direction::Forward, true, TYPICAL)),
            [1]
        );
        assert_eq!(preload_targets(1, 0, Direction::Forward, true, TYPICAL), []);
        assert_eq!(preload_targets(0, 0, Direction::Forward, true, TYPICAL), []);
        assert_eq!(preload_targets(3, 7, Direction::Forward, true, TYPICAL), []);
    }
}
