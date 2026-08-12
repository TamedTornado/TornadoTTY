use gtk::prelude::*;

pub(crate) fn build(
    appearance: &gtk::Widget,
    appearance_search: &gtk::SearchEntry,
    shortcuts: &gtk::Widget,
    shortcut_search: &gtk::SearchEntry,
) -> gtk::Widget {
    let root = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    let sidebar = gtk::Box::new(gtk::Orientation::Vertical, 6);
    sidebar.set_width_request(190);
    sidebar.set_margin_top(16);
    sidebar.set_margin_bottom(16);
    sidebar.set_margin_start(12);
    sidebar.set_margin_end(12);
    sidebar.add_css_class("zentty-settings-sidebar");

    let heading = gtk::Label::new(Some("Settings"));
    heading.add_css_class("title-2");
    heading.set_halign(gtk::Align::Start);
    heading.set_margin_bottom(8);
    sidebar.append(&heading);
    let appearance_button = gtk::ToggleButton::with_label("Appearance");
    appearance_button.set_widget_name("settings-nav-appearance");
    let shortcuts_button = gtk::ToggleButton::with_label("Shortcuts");
    shortcuts_button.set_widget_name("settings-nav-shortcuts");
    shortcuts_button.set_group(Some(&appearance_button));
    shortcuts_button.set_active(true);
    sidebar.append(&appearance_button);
    sidebar.append(&shortcuts_button);
    root.append(&sidebar);

    let separator = gtk::Separator::new(gtk::Orientation::Vertical);
    root.append(&separator);
    let stack = gtk::Stack::new();
    stack.set_hexpand(true);
    stack.set_vexpand(true);
    stack.add_named(appearance, Some("appearance"));
    stack.add_named(shortcuts, Some("shortcuts"));
    stack.set_visible_child_name("shortcuts");
    root.append(&stack);

    {
        let stack = stack.clone();
        appearance_button.connect_toggled(move |button| {
            if button.is_active() {
                stack.set_visible_child_name("appearance");
                eprintln!("zentty-linux: settings-section selected=appearance");
            }
        });
    }
    shortcuts_button.connect_toggled(move |button| {
        if button.is_active() {
            stack.set_visible_child_name("shortcuts");
            eprintln!("zentty-linux: settings-section selected=shortcuts");
        }
    });
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let appearance_button_for_key = appearance_button.clone();
    let shortcuts_button_for_key = shortcuts_button.clone();
    let appearance_search = appearance_search.clone();
    let shortcut_search = shortcut_search.clone();
    keys.connect_key_pressed(move |_, key, _, modifiers| {
        if !modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK) {
            return gtk::glib::Propagation::Proceed;
        }
        if key == gtk::gdk::Key::_1 {
            appearance_button_for_key.set_active(true);
            return gtk::glib::Propagation::Stop;
        }
        if key == gtk::gdk::Key::_2 {
            shortcuts_button_for_key.set_active(true);
            return gtk::glib::Propagation::Stop;
        }
        if matches!(key, gtk::gdk::Key::f | gtk::gdk::Key::F) {
            if appearance_button_for_key.is_active() {
                appearance_search.grab_focus();
            } else {
                shortcut_search.grab_focus();
                eprintln!("zentty-linux: shortcut-settings search-shortcut");
            }
            return gtk::glib::Propagation::Stop;
        }
        gtk::glib::Propagation::Proceed
    });
    root.add_controller(keys);
    root.upcast()
}
