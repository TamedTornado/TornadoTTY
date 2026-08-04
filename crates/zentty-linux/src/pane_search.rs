use gtk::gdk;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SearchShortcut {
    Find,
    UseSelection,
    Next,
    Previous,
}

pub(crate) fn resolve_shortcut(
    key: gdk::Key,
    modifiers: gdk::ModifierType,
) -> Option<SearchShortcut> {
    let shortcut_modifiers = modifiers
        & (gdk::ModifierType::CONTROL_MASK
            | gdk::ModifierType::SHIFT_MASK
            | gdk::ModifierType::ALT_MASK
            | gdk::ModifierType::SUPER_MASK
            | gdk::ModifierType::META_MASK
            | gdk::ModifierType::HYPER_MASK);
    match (key, shortcut_modifiers) {
        (gdk::Key::F, value)
            if value == (gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK) =>
        {
            Some(SearchShortcut::Find)
        }
        (gdk::Key::E, value)
            if value == (gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK) =>
        {
            Some(SearchShortcut::UseSelection)
        }
        (gdk::Key::F3, gdk::ModifierType::SHIFT_MASK) => Some(SearchShortcut::Previous),
        (gdk::Key::F3, value) if value.is_empty() => Some(SearchShortcut::Next),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{SearchShortcut, resolve_shortcut};
    use gtk::gdk;

    const SOURCE: &str = include_str!("../../../Zentty/Input/KeyboardShortcutResolver.swift");

    #[test]
    fn linux_search_shortcuts_preserve_shell_control_keys_and_source_commands() {
        let control_shift = gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK;
        assert_eq!(
            resolve_shortcut(gdk::Key::F, control_shift),
            Some(SearchShortcut::Find)
        );
        assert_eq!(
            resolve_shortcut(gdk::Key::E, control_shift),
            Some(SearchShortcut::UseSelection)
        );
        assert_eq!(
            resolve_shortcut(gdk::Key::F3, gdk::ModifierType::empty()),
            Some(SearchShortcut::Next)
        );
        assert_eq!(
            resolve_shortcut(gdk::Key::F3, gdk::ModifierType::SHIFT_MASK),
            Some(SearchShortcut::Previous)
        );
        assert_eq!(
            resolve_shortcut(gdk::Key::F, gdk::ModifierType::CONTROL_MASK),
            None,
            "Ctrl+F must remain terminal input on Linux"
        );
        assert_eq!(
            resolve_shortcut(gdk::Key::F, control_shift | gdk::ModifierType::ALT_MASK),
            None,
            "extra modifiers must not steal a terminal chord"
        );
        assert_eq!(
            resolve_shortcut(gdk::Key::F3, gdk::ModifierType::CONTROL_MASK),
            None,
            "modified F3 must remain terminal input"
        );
        for source_command in [
            "title: \"Find\"",
            "title: \"Use Selection for Find\"",
            "title: \"Find Next\"",
            "title: \"Find Previous\"",
        ] {
            assert!(SOURCE.contains(source_command));
        }
    }
}
