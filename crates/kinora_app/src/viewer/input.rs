//! Keyboard shortcuts (brief §5.3–§5.5). Pure mapping from a key press to an action, so it
//! can be unit-tested; the viewer decides what an action means in the current state
//! (e.g. the arrow keys pan when zoomed in and change the picture otherwise).

use slint::platform::Key;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    Next,
    Previous,
    First,
    Last,
    /// Arrow keys: pan when zoomed in, otherwise Left/Right navigate.
    Arrow(i8, i8),
    ZoomIn,
    ZoomOut,
    Fit,
    ActualSize,
    ToggleFullscreen,
    LeaveFullscreen,
    PlayPause,
    StepFrame(i8),
    Open,
}

/// Maps Slint's key text and modifiers to an action.
pub fn map_key(text: &str, ctrl: bool, _shift: bool, alt: bool) -> Option<KeyAction> {
    let mut chars = text.chars();
    let key = chars.next()?;
    if chars.next().is_some() || alt {
        return None;
    }
    let is = |special: Key| key == char::from(special);
    let action = if is(Key::RightArrow) {
        KeyAction::Arrow(1, 0)
    } else if is(Key::LeftArrow) {
        KeyAction::Arrow(-1, 0)
    } else if is(Key::UpArrow) {
        KeyAction::Arrow(0, -1)
    } else if is(Key::DownArrow) {
        KeyAction::Arrow(0, 1)
    } else if is(Key::PageDown) {
        KeyAction::Next
    } else if is(Key::PageUp) {
        KeyAction::Previous
    } else if is(Key::Home) {
        KeyAction::First
    } else if is(Key::End) {
        KeyAction::Last
    } else if is(Key::F11) {
        KeyAction::ToggleFullscreen
    } else if is(Key::Escape) {
        KeyAction::LeaveFullscreen
    } else {
        match (key, ctrl) {
            ('+' | '=', _) => KeyAction::ZoomIn,
            ('-' | '_', _) => KeyAction::ZoomOut,
            ('0', true) => KeyAction::Fit,
            ('1', true) => KeyAction::ActualSize,
            ('o' | 'O' | '\u{f}', true) => KeyAction::Open,
            (' ', false) => KeyAction::PlayPause,
            (',', false) => KeyAction::StepFrame(-1),
            ('.', false) => KeyAction::StepFrame(1),
            _ => return None,
        }
    };
    Some(action)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(special: Key) -> String {
        char::from(special).to_string()
    }

    #[test]
    fn navigation_keys() {
        assert_eq!(
            map_key(&key(Key::PageDown), false, false, false),
            Some(KeyAction::Next)
        );
        assert_eq!(
            map_key(&key(Key::PageUp), false, false, false),
            Some(KeyAction::Previous)
        );
        assert_eq!(
            map_key(&key(Key::Home), false, false, false),
            Some(KeyAction::First)
        );
        assert_eq!(
            map_key(&key(Key::End), false, false, false),
            Some(KeyAction::Last)
        );
        assert_eq!(
            map_key(&key(Key::RightArrow), false, false, false),
            Some(KeyAction::Arrow(1, 0))
        );
    }

    #[test]
    fn zoom_keys() {
        assert_eq!(map_key("+", false, true, false), Some(KeyAction::ZoomIn));
        assert_eq!(map_key("=", false, false, false), Some(KeyAction::ZoomIn));
        assert_eq!(map_key("-", false, false, false), Some(KeyAction::ZoomOut));
        assert_eq!(map_key("0", true, false, false), Some(KeyAction::Fit));
        assert_eq!(
            map_key("1", true, false, false),
            Some(KeyAction::ActualSize)
        );
        assert_eq!(map_key("0", false, false, false), None);
    }

    #[test]
    fn other_keys() {
        assert_eq!(
            map_key(&key(Key::F11), false, false, false),
            Some(KeyAction::ToggleFullscreen)
        );
        assert_eq!(map_key("o", true, false, false), Some(KeyAction::Open));
        assert_eq!(
            map_key(" ", false, false, false),
            Some(KeyAction::PlayPause)
        );
        assert_eq!(
            map_key(".", false, false, false),
            Some(KeyAction::StepFrame(1))
        );
        assert_eq!(map_key("x", false, false, false), None);
        assert_eq!(map_key("", false, false, false), None);
        assert_eq!(
            map_key("+", false, false, true),
            None,
            "Alt combinations are not ours"
        );
    }
}
