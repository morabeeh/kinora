//! Next / previous / first / last navigation through the folder list.
//!
//! The navigator also remembers the direction the user is moving in (preloading looks
//! further ahead in that direction) and a **generation** number that changes on every
//! move. Background decode jobs carry the generation they were started for, so results
//! that arrive after the user has moved on can be recognised as stale and dropped.

use crate::folder::FolderModel;

/// Which way the user is moving through the folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    /// Towards the end of the list (Right arrow, Page Down, …).
    #[default]
    Forward,
    /// Towards the start of the list.
    Backward,
}

/// A navigation request from the keyboard, mouse or UI buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavCommand {
    /// The next item.
    Next,
    /// The previous item.
    Previous,
    /// The first item (Home).
    First,
    /// The last item (End).
    Last,
    /// A specific index (e.g. a filmstrip click).
    GoTo(usize),
}

/// What a navigation request did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavOutcome {
    /// The current item changed.
    Moved {
        /// The previous index, if there was one.
        from: Option<usize>,
        /// The new current index.
        to: usize,
        /// `true` when the move wrapped around an end of the list.
        wrapped: bool,
    },
    /// Already at the end (wrap-around is off): show the subtle "end of folder" hint.
    EndReached(Direction),
    /// Nothing to do (empty list, or already there).
    Unchanged,
}

/// Navigation state: wrap-around setting, direction and generation.
#[derive(Debug, Clone, Default)]
pub struct Navigator {
    wrap: bool,
    direction: Direction,
    generation: u64,
}

impl Navigator {
    /// A navigator with the given wrap-around setting.
    #[must_use]
    pub fn new(wrap: bool) -> Self {
        Self {
            wrap,
            direction: Direction::Forward,
            generation: 0,
        }
    }

    /// Changes the wrap-around setting.
    pub fn set_wrap(&mut self, wrap: bool) {
        self.wrap = wrap;
    }

    /// Whether moving past an end wraps around.
    #[must_use]
    pub fn wraps(&self) -> bool {
        self.wrap
    }

    /// The direction of the last move.
    #[must_use]
    pub fn direction(&self) -> Direction {
        self.direction
    }

    /// Changes on every move; see the module docs.
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Starts a new generation without moving (e.g. the same file was reopened or reloaded).
    pub fn bump_generation(&mut self) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.generation
    }

    /// Applies `command` to `folder`.
    pub fn apply(&mut self, folder: &mut FolderModel, command: NavCommand) -> NavOutcome {
        let len = folder.len();
        let Some(last) = len.checked_sub(1) else {
            return NavOutcome::Unchanged;
        };
        let from = folder.current_index();
        let (target, direction, wrapped) = match (command, from) {
            (NavCommand::Next, None) | (NavCommand::First, _) => (0, Direction::Forward, false),
            (NavCommand::Previous, None) | (NavCommand::Last, _) => {
                (last, Direction::Backward, false)
            }
            (NavCommand::Next, Some(index)) if index >= last => {
                if !self.wrap {
                    self.direction = Direction::Forward;
                    return NavOutcome::EndReached(Direction::Forward);
                }
                (0, Direction::Forward, true)
            }
            (NavCommand::Next, Some(index)) => (index + 1, Direction::Forward, false),
            (NavCommand::Previous, Some(0)) => {
                if !self.wrap {
                    self.direction = Direction::Backward;
                    return NavOutcome::EndReached(Direction::Backward);
                }
                (last, Direction::Backward, true)
            }
            (NavCommand::Previous, Some(index)) => (index - 1, Direction::Backward, false),
            (NavCommand::GoTo(index), _) if index > last => return NavOutcome::Unchanged,
            (NavCommand::GoTo(index), Some(current)) if index < current => {
                (index, Direction::Backward, false)
            }
            (NavCommand::GoTo(index), _) => (index, Direction::Forward, false),
        };
        if from == Some(target) {
            return NavOutcome::Unchanged;
        }
        folder.set_current(target);
        self.direction = direction;
        self.bump_generation();
        NavOutcome::Moved {
            from,
            to: target,
            wrapped,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::folder::{MediaItem, SortOrder};
    use crate::media::MediaKind;
    use std::path::PathBuf;

    fn folder(count: usize) -> FolderModel {
        let mut model = FolderModel::new(SortOrder::default());
        model.replace_items(
            (0..count)
                .map(|i| MediaItem::new(PathBuf::from(format!("{i}.jpg")), MediaKind::Image))
                .collect(),
        );
        model
    }

    #[test]
    fn next_and_previous_move_one_step() {
        let mut model = folder(3);
        let mut nav = Navigator::new(false);
        let generation = nav.generation();
        assert_eq!(
            nav.apply(&mut model, NavCommand::Next),
            NavOutcome::Moved {
                from: Some(0),
                to: 1,
                wrapped: false
            }
        );
        assert_ne!(nav.generation(), generation);
        assert_eq!(nav.direction(), Direction::Forward);
        nav.apply(&mut model, NavCommand::Previous);
        assert_eq!(model.current_index(), Some(0));
        assert_eq!(nav.direction(), Direction::Backward);
    }

    #[test]
    fn ends_without_wrap_report_end_reached() {
        let mut model = folder(2);
        let mut nav = Navigator::new(false);
        assert_eq!(
            nav.apply(&mut model, NavCommand::Previous),
            NavOutcome::EndReached(Direction::Backward)
        );
        nav.apply(&mut model, NavCommand::Last);
        let generation = nav.generation();
        assert_eq!(
            nav.apply(&mut model, NavCommand::Next),
            NavOutcome::EndReached(Direction::Forward)
        );
        assert_eq!(model.current_index(), Some(1));
        assert_eq!(nav.generation(), generation, "no move, no new generation");
    }

    #[test]
    fn ends_with_wrap_go_around() {
        let mut model = folder(3);
        let mut nav = Navigator::new(true);
        assert_eq!(
            nav.apply(&mut model, NavCommand::Previous),
            NavOutcome::Moved {
                from: Some(0),
                to: 2,
                wrapped: true
            }
        );
        assert_eq!(
            nav.apply(&mut model, NavCommand::Next),
            NavOutcome::Moved {
                from: Some(2),
                to: 0,
                wrapped: true
            }
        );
    }

    #[test]
    fn single_item_with_wrap_is_unchanged() {
        let mut model = folder(1);
        let mut nav = Navigator::new(true);
        assert_eq!(
            nav.apply(&mut model, NavCommand::Next),
            NavOutcome::Unchanged
        );
    }

    #[test]
    fn home_end_and_goto() {
        let mut model = folder(10);
        let mut nav = Navigator::new(false);
        nav.apply(&mut model, NavCommand::Last);
        assert_eq!(model.current_index(), Some(9));
        assert_eq!(nav.direction(), Direction::Backward);
        nav.apply(&mut model, NavCommand::GoTo(4));
        assert_eq!(nav.direction(), Direction::Backward);
        nav.apply(&mut model, NavCommand::GoTo(6));
        assert_eq!(nav.direction(), Direction::Forward);
        assert_eq!(
            nav.apply(&mut model, NavCommand::GoTo(10)),
            NavOutcome::Unchanged
        );
        nav.apply(&mut model, NavCommand::First);
        assert_eq!(model.current_index(), Some(0));
    }

    #[test]
    fn empty_folder_never_moves() {
        let mut model = folder(0);
        let mut nav = Navigator::new(true);
        for command in [
            NavCommand::Next,
            NavCommand::Previous,
            NavCommand::First,
            NavCommand::Last,
            NavCommand::GoTo(0),
        ] {
            assert_eq!(nav.apply(&mut model, command), NavOutcome::Unchanged);
        }
    }
}
