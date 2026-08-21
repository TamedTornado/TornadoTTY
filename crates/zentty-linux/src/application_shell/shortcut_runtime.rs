use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use zentty_core::{KeyboardShortcut, ShortcutKey, ShortcutManager, ShortcutModifier};

use super::shortcut_registry::COMMANDS;

pub(crate) fn shortcut_from_event(
    key: gdk::Key,
    modifiers: gdk::ModifierType,
) -> Option<KeyboardShortcut> {
    let key = match key {
        gdk::Key::space => ShortcutKey::Space,
        gdk::Key::Delete | gdk::Key::BackSpace => ShortcutKey::Delete,
        gdk::Key::Return | gdk::Key::KP_Enter => ShortcutKey::Return,
        gdk::Key::Tab | gdk::Key::ISO_Left_Tab => ShortcutKey::Tab,
        gdk::Key::Left => ShortcutKey::Left,
        gdk::Key::Right => ShortcutKey::Right,
        gdk::Key::Up => ShortcutKey::Up,
        gdk::Key::Down => ShortcutKey::Down,
        gdk::Key::F1 => ShortcutKey::Function(1),
        gdk::Key::F2 => ShortcutKey::Function(2),
        gdk::Key::F3 => ShortcutKey::Function(3),
        gdk::Key::F4 => ShortcutKey::Function(4),
        gdk::Key::F5 => ShortcutKey::Function(5),
        gdk::Key::F6 => ShortcutKey::Function(6),
        gdk::Key::F7 => ShortcutKey::Function(7),
        gdk::Key::F8 => ShortcutKey::Function(8),
        gdk::Key::F9 => ShortcutKey::Function(9),
        gdk::Key::F10 => ShortcutKey::Function(10),
        gdk::Key::F11 => ShortcutKey::Function(11),
        gdk::Key::F12 => ShortcutKey::Function(12),
        key => ShortcutKey::Character(key.to_unicode()?.to_lowercase().next()?),
    };
    let mut translated = HashSet::new();
    if modifiers.contains(gdk::ModifierType::CONTROL_MASK) {
        translated.insert(ShortcutModifier::Command);
    }
    if modifiers.intersects(gdk::ModifierType::SUPER_MASK | gdk::ModifierType::META_MASK) {
        translated.insert(ShortcutModifier::Control);
    }
    if modifiers.contains(gdk::ModifierType::ALT_MASK) {
        translated.insert(ShortcutModifier::Option);
    }
    if modifiers.contains(gdk::ModifierType::SHIFT_MASK) {
        translated.insert(ShortcutModifier::Shift);
    }
    Some(KeyboardShortcut {
        key,
        modifiers: translated,
    })
}

pub(crate) fn install(
    window: &gtk::Window,
    manager: Rc<RefCell<ShortcutManager>>,
) -> gtk::EventControllerKey {
    let controller = gtk::EventControllerKey::new();
    controller.set_propagation_phase(gtk::PropagationPhase::Capture);
    let weak_window = window.downgrade();
    controller.connect_key_pressed(move |_, key, _, modifiers| {
        // Control+Tab is owned by the hold/peek state machine. It must see both
        // key-down and key-up rather than being reduced to a one-shot action.
        if matches!(key, gdk::Key::Tab | gdk::Key::ISO_Left_Tab)
            && modifiers.contains(gdk::ModifierType::CONTROL_MASK)
        {
            return glib::Propagation::Proceed;
        }
        let Some(shortcut) = shortcut_from_event(key, modifiers) else {
            return glib::Propagation::Proceed;
        };
        let action = {
            let manager = manager.borrow();
            let Some(command_id) = manager.command_for(&shortcut) else {
                if shortcut.is_eligible_command_binding() {
                    eprintln!(
                        "zentty-linux: shortcut-unbound value={}",
                        shortcut.storage_string()
                    );
                }
                return glib::Propagation::Proceed;
            };
            let Some(command) = COMMANDS
                .iter()
                .find(|command| command.command_id == command_id)
            else {
                return glib::Propagation::Proceed;
            };
            command.action
        };
        let Some(window) = weak_window.upgrade() else {
            return glib::Propagation::Proceed;
        };
        match window.activate_action(&format!("workspace.{action}"), None) {
            Ok(()) => {
                eprintln!(
                    "zentty-linux: shortcut={} action={action}",
                    shortcut.storage_string()
                );
                glib::Propagation::Stop
            }
            Err(error) => {
                eprintln!("zentty-linux: shortcut action={action} failed: {error}");
                glib::Propagation::Proceed
            }
        }
    });
    window.add_controller(controller.clone());
    controller
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gdk_translation_preserves_source_modifier_classes_and_physical_keys() {
        let shortcut = shortcut_from_event(
            gdk::Key::Left,
            gdk::ModifierType::CONTROL_MASK
                | gdk::ModifierType::SUPER_MASK
                | gdk::ModifierType::ALT_MASK
                | gdk::ModifierType::SHIFT_MASK,
        )
        .unwrap();
        assert_eq!(
            shortcut.storage_string(),
            "command+control+option+shift+left"
        );
        assert_eq!(
            shortcut_from_event(gdk::Key::KP_Enter, gdk::ModifierType::CONTROL_MASK)
                .unwrap()
                .storage_string(),
            "command+return"
        );
    }

    #[test]
    fn function_keys_are_physical_command_bindings() {
        assert_eq!(
            shortcut_from_event(gdk::Key::F11, gdk::ModifierType::empty())
                .unwrap()
                .storage_string(),
            "f11"
        );
    }
}
