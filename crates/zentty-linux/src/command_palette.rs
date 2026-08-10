use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::glib::{self, variant::ToVariant};
use gtk::prelude::*;
use zentty_core::{
    CommandPaletteItem, CommandPaletteTarget, PaneReference, resolve_command_palette,
};

#[derive(Clone)]
pub(crate) struct CommandPaletteView {
    root: gtk::Box,
    panel: gtk::Box,
    entry: gtk::SearchEntry,
    list: gtk::ListBox,
    items: Rc<RefCell<Vec<CommandPaletteItem>>>,
    recent_panes: Rc<RefCell<Vec<PaneReference>>>,
    current_pane: Rc<RefCell<Option<PaneReference>>>,
    visible: Rc<Cell<bool>>,
}

impl CommandPaletteView {
    pub(crate) fn new() -> Self {
        install_styles();
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("command-palette-backdrop");
        root.set_hexpand(true);
        root.set_vexpand(true);
        root.set_halign(gtk::Align::Fill);
        root.set_valign(gtk::Align::Fill);
        root.set_visible(false);
        let panel = gtk::Box::new(gtk::Orientation::Vertical, 8);
        panel.add_css_class("command-palette");
        panel.set_width_request(620);
        panel.set_halign(gtk::Align::Center);
        panel.set_valign(gtk::Align::Start);
        panel.set_margin_top(110);
        let entry = gtk::SearchEntry::new();
        entry.add_css_class("command-palette-search");
        entry.set_placeholder_text(Some("Search commands, panes, and settings"));
        entry.update_property(&[gtk::accessible::Property::Label("Command Palette Search")]);
        let list = gtk::ListBox::new();
        list.add_css_class("command-palette-results");
        list.set_selection_mode(gtk::SelectionMode::Single);
        panel.append(&entry);
        panel.append(&list);
        root.append(&panel);
        let view = Self {
            root,
            panel,
            entry,
            list,
            items: Rc::new(RefCell::new(Vec::new())),
            recent_panes: Rc::new(RefCell::new(Vec::new())),
            current_pane: Rc::new(RefCell::new(None)),
            visible: Rc::new(Cell::new(false)),
        };
        view.install_handlers();
        view
    }

    pub(crate) fn widget(&self) -> &gtk::Box {
        &self.root
    }

    pub(crate) fn is_visible(&self) -> bool {
        self.visible.get()
    }

    pub(crate) fn show(
        &self,
        items: Vec<CommandPaletteItem>,
        recent_panes: Vec<PaneReference>,
        current_pane: Option<PaneReference>,
    ) {
        *self.items.borrow_mut() = items;
        *self.recent_panes.borrow_mut() = recent_panes;
        *self.current_pane.borrow_mut() = current_pane;
        self.entry.set_text("");
        self.render("");
        self.visible.set(true);
        self.root.set_visible(true);
        self.entry.grab_focus();
        eprintln!("zentty-linux: command-palette=shown");
    }

    pub(crate) fn hide(&self) {
        self.visible.set(false);
        self.root.set_visible(false);
        eprintln!("zentty-linux: command-palette=hidden");
    }

    fn install_handlers(&self) {
        let changed = self.clone();
        self.entry
            .connect_search_changed(move |entry| changed.render(entry.text().as_str()));
        self.list.connect_row_activated(move |_, row| {
            if let Some(button) = row
                .child()
                .and_then(|child| child.downcast::<gtk::Button>().ok())
            {
                button.emit_clicked();
            }
        });
        let outside = gtk::GestureClick::new();
        let root = self.root.clone();
        let panel = self.panel.clone();
        outside.connect_released(move |_, _, x, y| {
            let Some(bounds) = panel.compute_bounds(&root) else {
                return;
            };
            if x < f64::from(bounds.x())
                || x > f64::from(bounds.x() + bounds.width())
                || y < f64::from(bounds.y())
                || y > f64::from(bounds.y() + bounds.height())
            {
                let _ = root.activate_action("workspace.dismiss-command-palette", None);
            }
        });
        self.root.add_controller(outside);
        let key = gtk::EventControllerKey::new();
        key.set_propagation_phase(gtk::PropagationPhase::Capture);
        let keyed = self.clone();
        key.connect_key_pressed(move |_, key, _, _| {
            match key {
                gtk::gdk::Key::Escape => {
                    keyed.hide();
                    glib::Propagation::Stop
                }
                gtk::gdk::Key::Down => {
                    if let Some(row) = keyed.list.row_at_index(0) {
                        keyed.list.select_row(Some(&row));
                        row.grab_focus();
                    }
                    glib::Propagation::Stop
                }
                gtk::gdk::Key::Return | gtk::gdk::Key::KP_Enter => {
                    // Filtering replaces every row. GTK can briefly retain a
                    // detached selected-row handle, so Enter must resolve the
                    // first currently rendered result rather than execute a
                    // stale pre-filter selection.
                    let row = keyed.list.row_at_index(0);
                    if let Some(button) = row
                        .and_then(|row| row.child())
                        .and_then(|child| child.downcast::<gtk::Button>().ok())
                    {
                        button.emit_clicked();
                    }
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        });
        self.entry.add_controller(key);
    }

    fn render(&self, query: &str) {
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        let resolved = resolve_command_palette(
            query,
            &self.items.borrow(),
            &self.recent_panes.borrow(),
            self.current_pane.borrow().as_ref(),
        );
        for item in resolved {
            let row = gtk::ListBoxRow::new();
            let button = gtk::Button::new();
            button.add_css_class("command-palette-result");
            let content = gtk::Box::new(gtk::Orientation::Vertical, 1);
            let title = gtk::Label::new(Some(&item.title));
            title.add_css_class("command-palette-title");
            title.set_xalign(0.0);
            let subtitle = gtk::Label::new(Some(&item.subtitle));
            subtitle.add_css_class("command-palette-subtitle");
            subtitle.set_xalign(0.0);
            subtitle.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
            content.append(&title);
            content.append(&subtitle);
            button.set_child(Some(&content));
            button.update_property(&[gtk::accessible::Property::Label(&format!(
                "{}, {}",
                item.title, item.subtitle
            ))]);
            match &item.target {
                CommandPaletteTarget::Pane(reference) => {
                    button.set_action_name(Some("workspace.select-pane"));
                    button.set_action_target_value(Some(
                        &(reference.worklane_id.as_str(), reference.pane_id.as_str()).to_variant(),
                    ));
                }
                CommandPaletteTarget::Action(action) => {
                    button.set_action_name(Some(&format!("workspace.{action}")));
                }
                CommandPaletteTarget::ParameterizedAction { action, parameter } => {
                    button.set_action_name(Some(&format!("workspace.{action}")));
                    button.set_action_target_value(Some(&parameter.to_variant()));
                }
            }
            let hidden = self.clone();
            let target = item.target.clone();
            button.connect_clicked(move |_| {
                eprintln!("zentty-linux: command-palette=execute target={target:?}");
                hidden.hide();
            });
            row.set_child(Some(&button));
            self.list.append(&row);
        }
        if let Some(first) = self.list.row_at_index(0) {
            self.list.select_row(Some(&first));
        }
        eprintln!(
            "zentty-linux: command-palette=query value={query:?} results={}",
            self.list.observe_children().n_items()
        );
    }
}

fn install_styles() {
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };
    let provider = gtk::CssProvider::new();
    provider.load_from_string(
        ".command-palette-backdrop { background: rgba(5, 7, 10, 0.52); }\n\
         .command-palette { background: #20242b; border: 1px solid #596273; border-radius: 10px; padding: 0; box-shadow: 0 18px 48px rgba(0,0,0,0.72); }\n\
         .command-palette-search { min-height: 44px; margin: 10px; padding: 0 12px; background: #15181d; color: #f7f8fa; caret-color: #ffffff; border: 1px solid #717b8c; border-radius: 6px; font-size: 17px; box-shadow: none; }\n\
         .command-palette-search:focus { border-color: #5b9cff; box-shadow: 0 0 0 1px #5b9cff; }\n\
         .command-palette-search text { background: transparent; color: #f7f8fa; caret-color: #ffffff; }\n\
         .command-palette-search text selection { background: #3478f6; color: #ffffff; }\n\
         .command-palette-search image { color: #b8c0cc; }\n\
         .command-palette-search placeholder { color: #8993a1; }\n\
         .command-palette-results { margin: 0; padding: 6px; background: #20242b; border-top: 1px solid #363c46; border-radius: 0 0 10px 10px; }\n\
         .command-palette-results row { margin: 1px 0; padding: 0; background: transparent; border-radius: 5px; }\n\
         .command-palette-result { min-height: 38px; background: transparent; border: 0; border-radius: 5px; padding: 6px 10px; box-shadow: none; }\n\
         .command-palette-results row:hover .command-palette-result { background: #2a2d2e; }\n\
         .command-palette-results row:selected .command-palette-result { background: #094771; }\n\
         .command-palette-results row:selected .command-palette-title { color: #ffffff; }\n\
         .command-palette-results row:selected .command-palette-subtitle { color: #d7e8f5; }\n\
         .command-palette-title { color: #f3f4f6; font-size: 13px; font-weight: 600; }\n\
         .command-palette-subtitle { color: #aab1bd; font-size: 11px; }",
    );
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}
