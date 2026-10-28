//! A menu button's tooltip must never cover its own open menu.

use gtk::prelude::*;

/// GTK keeps showing a hovered menu button's tooltip after its popover opens.
/// Beside a sidebar row that tooltip lands on the first menu item and swallows
/// the click meant for it (#212), so suppress it while the menu is active.
pub(crate) fn suppress_while_open(button: &gtk::MenuButton) {
    button.connect_active_notify(|button| button.set_has_tooltip(!button.is_active()));
}
