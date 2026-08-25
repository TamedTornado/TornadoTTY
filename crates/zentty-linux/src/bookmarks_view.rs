use gtk::glib::variant::ToVariant;
use gtk::prelude::*;
use std::cell::RefCell;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::rc::Rc;
use zentty_core::{TemplateKind, WorkspaceTemplate};

const BUTTON_NAME: &str = "zentty-bookmarks-button";
const POPOVER_NAME_PREFIX: &str = "zentty-bookmarks-popover-";
const SEARCH_NAME: &str = "zentty-bookmarks-search";

pub(crate) fn open_from(root: &gtk::Widget) -> bool {
    let Some(button) = find_named_widget(root, BUTTON_NAME)
        .and_then(|widget| widget.downcast::<gtk::MenuButton>().ok())
    else {
        return false;
    };
    button.popup();
    // Keyboard activation can reopen a popover which GTK kept realized while
    // it was hidden. In that case neither realization nor mapping is a useful
    // per-open focus boundary, so explicitly focus its search field after the
    // popup request has reached the main loop.
    if let Some(search) = button
        .popover()
        .and_then(|popover| find_named_widget(popover.upcast_ref(), SEARCH_NAME))
        .and_then(|widget| widget.downcast::<gtk::SearchEntry>().ok())
    {
        gtk::glib::idle_add_local_once(move || {
            let focused = search.grab_focus();
            eprintln!("zentty-linux: bookmarks-search-focused={focused}");
        });
    }
    true
}

fn is_context_menu_shortcut(key: gtk::gdk::Key, modifiers: gtk::gdk::ModifierType) -> bool {
    key == gtk::gdk::Key::Menu
        || (key == gtk::gdk::Key::F10 && modifiers.contains(gtk::gdk::ModifierType::SHIFT_MASK))
}

pub(crate) fn configure_header(
    header: &gtk::Box,
    window: &gtk::Window,
    templates: &[WorkspaceTemplate],
    active_origin_id: Option<&str>,
) {
    let button = find_named_widget(header.upcast_ref(), BUTTON_NAME)
        .and_then(|widget| widget.downcast::<gtk::MenuButton>().ok())
        .unwrap_or_else(|| {
            let button = gtk::MenuButton::new();
            button.set_widget_name(BUTTON_NAME);
            button.set_icon_name("starred-symbolic");
            button.set_tooltip_text(Some("Bookmarks & presets"));
            button.set_accessible_role(gtk::AccessibleRole::Button);
            button.update_property(&[gtk::accessible::Property::Label("Bookmarks and presets")]);
            button.connect_active_notify(|button| {
                eprintln!(
                    "zentty-linux: bookmarks-popover visible={}",
                    button.is_active()
                );
            });
            header.append(&button);
            button
        });
    let signature = popover_signature(templates, active_origin_id);
    if button
        .popover()
        .is_some_and(|popover| popover.widget_name() == format!("{POPOVER_NAME_PREFIX}{signature}"))
    {
        return;
    }
    // Project-context and terminal-title updates can redraw the header while
    // the user is traversing this popover. Replacing an open popover destroys
    // its focused child and routes the following physical key back to the
    // terminal. Mutations pop the menu down before changing the store, so the
    // next closed-state redraw still installs the refreshed contents.
    if button.is_active() {
        return;
    }
    let popover = make_popover(window, templates, active_origin_id);
    popover.set_widget_name(&format!("{POPOVER_NAME_PREFIX}{signature}"));
    button.set_popover(Some(&popover));
}

fn popover_signature(templates: &[WorkspaceTemplate], active_origin_id: Option<&str>) -> u64 {
    let mut hasher = DefaultHasher::new();
    active_origin_id.hash(&mut hasher);
    // WorkspaceTemplate deliberately contains floating-point geometry and
    // therefore does not implement Hash. Its canonical persisted form covers
    // every field rendered by this popover and lets unrelated sidebar redraws
    // retain the live GTK popover and its keyboard focus.
    serde_json::to_string(templates)
        .expect("workspace templates serialize")
        .hash(&mut hasher);
    hasher.finish()
}

fn make_popover(
    window: &gtk::Window,
    templates: &[WorkspaceTemplate],
    active_origin_id: Option<&str>,
) -> gtk::Popover {
    let popover = gtk::Popover::new();
    popover.add_css_class("bookmark-popover");
    let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
    content.set_width_request(340);
    content.set_margin_top(10);
    content.set_margin_bottom(10);
    content.set_margin_start(10);
    content.set_margin_end(10);

    let search_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let search = gtk::SearchEntry::new();
    search.set_widget_name(SEARCH_NAME);
    search.set_hexpand(true);
    search.set_placeholder_text(Some("Search bookmarks and presets"));
    search_row.append(&search);
    search_row.append(&create_menu(window, &popover));
    content.append(&search_row);
    let search_focus = search.clone();
    popover.connect_show(move |_| {
        let search_focus = search_focus.clone();
        gtk::glib::idle_add_local_once(move || {
            let focused = search_focus.grab_focus();
            eprintln!("zentty-linux: bookmarks-search-focused={focused}");
        });
    });

    let list = gtk::Box::new(gtk::Orientation::Vertical, 4);
    if templates.is_empty() {
        list.append(&empty_state(window, &popover));
    } else {
        for kind in [TemplateKind::Bookmark, TemplateKind::Preset] {
            if let Some(section) = template_section(window, &popover, templates, kind) {
                list.append(&section);
            }
        }
    }
    let scroller = gtk::ScrolledWindow::builder()
        .min_content_height(80)
        .max_content_height(360)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&list)
        .build();
    content.append(&scroller);

    if let Some(origin_id) = active_origin_id {
        let linked = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        linked.set_homogeneous(true);
        let update = gtk::Button::with_label("Update linked bookmark");
        update.set_action_name(Some("workspace.update-linked-template"));
        connect_action_focus(&update, "update-linked-template");
        let update_popover = popover.clone();
        update.connect_clicked(move |_| update_popover.popdown());
        let unlink = gtk::Button::with_label("Unlink worklane");
        unlink.set_action_name(Some("workspace.unlink-template"));
        connect_action_focus(&unlink, "unlink-template");
        let unlink_popover = popover.clone();
        unlink.connect_clicked(move |_| unlink_popover.popdown());
        linked.append(&update);
        linked.append(&unlink);
        linked.set_tooltip_text(Some(&format!("Linked to {origin_id}")));
        content.append(&linked);
    }
    popover.set_child(Some(&content));

    let rows = list.clone();
    search.connect_search_changed(move |search| {
        let query = search.text().to_lowercase();
        eprintln!("zentty-linux: bookmarks-search query={query:?}");
        let mut section = rows.first_child();
        while let Some(group) = section {
            section = group.next_sibling();
            let Some(section_rows) = group.last_child() else {
                continue;
            };
            let mut any_visible = false;
            let mut child = section_rows.first_child();
            while let Some(row) = child {
                child = row.next_sibling();
                let haystack = row
                    .tooltip_text()
                    .map_or_else(String::new, |value| value.to_lowercase());
                let visible = query.is_empty() || haystack.contains(&query);
                row.set_visible(visible);
                any_visible |= visible;
            }
            group.set_visible(any_visible);
        }
    });
    popover
}

fn create_menu(window: &gtk::Window, parent_popover: &gtk::Popover) -> gtk::MenuButton {
    let menu = gtk::MenuButton::new();
    menu.set_focusable(true);
    menu.set_widget_name("zentty-template-create-menu");
    menu.connect_has_focus_notify(|button| {
        if button.has_focus() {
            eprintln!("zentty-linux: bookmarks-focus=create-menu");
        }
    });
    menu.set_icon_name("list-add-symbolic");
    menu.set_tooltip_text(Some("Create or import bookmark or preset"));
    let popover = gtk::Popover::new();
    let actions = gtk::Box::new(gtk::Orientation::Vertical, 4);
    actions.set_margin_top(6);
    actions.set_margin_bottom(6);
    actions.set_margin_start(6);
    actions.set_margin_end(6);
    let bookmark = gtk::Button::with_label("Bookmark Current Worklane…");
    let preset = gtk::Button::with_label("Save Current as Preset…");
    connect_action_focus(&bookmark, "save-bookmark");
    connect_action_focus(&preset, "save-preset");
    connect_save_button(&bookmark, window, parent_popover, TemplateKind::Bookmark);
    connect_save_button(&preset, window, parent_popover, TemplateKind::Preset);
    actions.append(&bookmark);
    actions.append(&preset);
    actions.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    let import = gtk::Button::with_label("Import Preset…");
    import.set_action_name(Some("workspace.import-template"));
    connect_action_focus(&import, "import-template");
    let import_menu = popover.clone();
    let import_parent = parent_popover.clone();
    import.connect_clicked(move |_| {
        import_menu.popdown();
        import_parent.popdown();
    });
    actions.append(&import);
    popover.set_child(Some(&actions));
    menu.set_popover(Some(&popover));
    // GtkMenuButton delegates input to its internal toggle button. Keep that
    // real focus target in the popover's keyboard traversal rather than only
    // marking the non-interactive wrapper focusable.
    if let Some(toggle) = menu.first_child() {
        toggle.set_focusable(true);
        toggle.connect_has_focus_notify(|toggle| {
            if toggle.has_focus() {
                eprintln!("zentty-linux: bookmarks-focus=create-menu");
            }
        });
    }
    menu
}

fn empty_state(window: &gtk::Window, parent_popover: &gtk::Popover) -> gtk::Box {
    let empty = gtk::Box::new(gtk::Orientation::Vertical, 8);
    empty.set_margin_top(20);
    empty.set_margin_bottom(20);
    empty.append(&gtk::Image::from_icon_name("bookmark-new-symbolic"));
    let title = gtk::Label::new(Some("No bookmarks or presets yet"));
    title.add_css_class("heading");
    empty.append(&title);
    let explanation = gtk::Label::new(Some("Save a worklane setup to relaunch it instantly."));
    explanation.add_css_class("dim-label");
    explanation.set_wrap(true);
    empty.append(&explanation);
    let bookmark = gtk::Button::with_label("Bookmark current worklane");
    bookmark.add_css_class("suggested-action");
    bookmark.set_halign(gtk::Align::Center);
    bookmark.set_widget_name("zentty-save-bookmark");
    bookmark.connect_has_focus_notify(|button| {
        if button.has_focus() {
            eprintln!("zentty-linux: bookmarks-focus=save-bookmark");
        }
    });
    connect_save_button(&bookmark, window, parent_popover, TemplateKind::Bookmark);
    empty.append(&bookmark);
    empty
}

fn template_section(
    window: &gtk::Window,
    parent_popover: &gtk::Popover,
    templates: &[WorkspaceTemplate],
    kind: TemplateKind,
) -> Option<gtk::Box> {
    let mut ordered = templates
        .iter()
        .filter(|template| template.kind == kind)
        .collect::<Vec<_>>();
    if ordered.is_empty() {
        return None;
    }
    ordered.sort_by(|left, right| {
        right
            .pinned
            .cmp(&left.pinned)
            .then_with(|| right.last_used_at.cmp(&left.last_used_at))
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });
    let section = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let heading = gtk::Label::new(Some(match kind {
        TemplateKind::Bookmark => "BOOKMARKS",
        TemplateKind::Preset => "PRESETS",
    }));
    heading.add_css_class("caption");
    heading.add_css_class("dim-label");
    heading.set_xalign(0.0);
    section.append(&heading);
    let rows = gtk::Box::new(gtk::Orientation::Vertical, 4);
    for template in ordered {
        rows.append(&template_row(window, parent_popover, template));
    }
    section.append(&rows);
    Some(section)
}

fn connect_save_button(
    button: &gtk::Button,
    window: &gtk::Window,
    popover: &gtk::Popover,
    kind: TemplateKind,
) {
    let window = window.clone();
    let popover = popover.clone();
    button.connect_clicked(move |_| {
        eprintln!(
            "zentty-linux: bookmark-save-activated kind={}",
            kind_label(kind)
        );
        popover.popdown();
        present_name_dialog(&window, "Save workspace", kind, None, "");
    });
}

fn template_row(
    window: &gtk::Window,
    parent_popover: &gtk::Popover,
    template: &WorkspaceTemplate,
) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 5);
    row.add_css_class("bookmark-row");
    row.set_tooltip_text(Some(&format!(
        "{} {}",
        template.name,
        kind_label(template.kind)
    )));

    let activate = gtk::Button::new();
    activate.set_widget_name("zentty-activate-template");
    let focus_id = template.id.clone();
    let focus_name = template.name.clone();
    activate.connect_has_focus_notify(move |button| {
        if button.has_focus() {
            eprintln!(
                "zentty-linux: bookmarks-focus=activate-template id={focus_id} name={focus_name:?}"
            );
        }
    });
    activate.set_has_frame(false);
    activate.set_hexpand(true);
    activate.set_action_name(Some("workspace.activate-template"));
    activate.set_action_target_value(Some(&template.id.to_variant()));
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 7);
    content.append(&gtk::Image::from_icon_name(match template.kind {
        TemplateKind::Bookmark => "bookmark-new-symbolic",
        TemplateKind::Preset => "document-open-recent-symbolic",
    }));
    let labels = gtk::Box::new(gtk::Orientation::Vertical, 1);
    let name = gtk::Label::new(Some(&template.name));
    name.set_xalign(0.0);
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    let kind = gtk::Label::new(Some(kind_label(template.kind)));
    kind.add_css_class("dim-label");
    kind.set_xalign(0.0);
    labels.append(&name);
    labels.append(&kind);
    content.append(&labels);
    if template.pinned {
        content.append(&gtk::Image::from_icon_name("view-pin-symbolic"));
    }
    activate.set_child(Some(&content));
    let popover = parent_popover.clone();
    activate.connect_clicked(move |_| popover.popdown());
    row.append(&activate);

    let menu = gtk::MenuButton::new();
    menu.set_focusable(true);
    menu.set_widget_name("zentty-template-actions");
    menu.connect_has_focus_notify(|button| {
        if button.has_focus() {
            eprintln!("zentty-linux: bookmarks-focus=template-actions");
        }
    });
    menu.set_icon_name("view-more-symbolic");
    menu.set_tooltip_text(Some("Template actions"));
    menu.update_property(&[gtk::accessible::Property::Label(&format!(
        "Actions for {}",
        template.name
    ))]);
    menu.set_popover(Some(&template_actions(window, parent_popover, template)));
    let context_menu = menu.clone();
    let context_id = template.id.clone();
    let context_name = template.name.clone();
    let context_keys = gtk::EventControllerKey::new();
    context_keys.connect_key_pressed(move |_, key, _, modifiers| {
        if !is_context_menu_shortcut(key, modifiers) {
            return gtk::glib::Propagation::Proceed;
        }
        let context_menu = context_menu.clone();
        let context_id = context_id.clone();
        let context_name = context_name.clone();
        gtk::glib::timeout_add_local_once(std::time::Duration::from_millis(50), move || {
            context_menu.popup();
            eprintln!(
                "zentty-linux: bookmarks-context-menu=shown keyboard=true id={context_id} name={context_name:?}"
            );
        });
        gtk::glib::Propagation::Stop
    });
    activate.add_controller(context_keys);
    row.append(&menu);
    row
}

fn template_actions(
    window: &gtk::Window,
    parent_popover: &gtk::Popover,
    template: &WorkspaceTemplate,
) -> gtk::Popover {
    let popover = gtk::Popover::new();
    let actions = gtk::Box::new(gtk::Orientation::Vertical, 4);
    actions.set_margin_top(6);
    actions.set_margin_bottom(6);
    actions.set_margin_start(6);
    actions.set_margin_end(6);
    let rename = gtk::Button::with_label("Rename…");
    connect_action_focus(&rename, "rename-template");
    let name = template.name.clone();
    let id = template.id.clone();
    let kind = template.kind;
    let window_clone = window.clone();
    let popover_clone = popover.clone();
    let parent_clone = parent_popover.clone();
    rename.connect_clicked(move |_| {
        popover_clone.popdown();
        parent_clone.popdown();
        present_name_dialog(&window_clone, "Rename template", kind, Some(&id), &name);
    });
    actions.append(&rename);
    let edit = gtk::Button::with_label("Edit…");
    connect_action_focus(&edit, "edit-template");
    let editable = template.clone();
    let window_clone = window.clone();
    let popover_clone = popover.clone();
    let parent_clone = parent_popover.clone();
    edit.connect_clicked(move |_| {
        popover_clone.popdown();
        parent_clone.popdown();
        present_edit_dialog(&window_clone, editable.clone());
    });
    actions.append(&edit);
    for (label, action) in [
        ("Duplicate", "duplicate-template"),
        (
            if template.pinned {
                "Unpin"
            } else {
                "Pin to top"
            },
            "toggle-template-pin",
        ),
        (
            if template.kind == TemplateKind::Bookmark {
                "Save as Preset…"
            } else {
                "Bookmark in current worklane…"
            },
            "convert-template",
        ),
        ("Export as Preset…", "export-template"),
        ("Delete", "delete-template"),
    ] {
        let button = gtk::Button::with_label(label);
        connect_action_focus(&button, action);
        if action == "export-template" {
            button.set_widget_name("zentty-export-template");
        }
        let action_menu = popover.clone();
        let action_parent = parent_popover.clone();
        button.connect_clicked(move |_| {
            action_menu.popdown();
            action_parent.popdown();
        });
        button.set_action_name(Some(&format!("workspace.{action}")));
        button.set_action_target_value(Some(&template.id.to_variant()));
        actions.append(&button);
    }
    popover.set_child(Some(&actions));
    popover
}

fn connect_action_focus(button: &gtk::Button, action: &'static str) {
    button.connect_has_focus_notify(move |button| {
        if button.has_focus() {
            eprintln!("zentty-linux: bookmarks-focus={action}");
        }
    });
}

fn present_edit_dialog(window: &gtk::Window, template: WorkspaceTemplate) {
    let dialog = gtk::Window::builder()
        .title("Edit bookmark or preset")
        .transient_for(window)
        .modal(true)
        .default_width(560)
        .default_height(420)
        .build();
    let root = gtk::Box::new(gtk::Orientation::Vertical, 10);
    root.set_margin_top(16);
    root.set_margin_bottom(16);
    root.set_margin_start(16);
    root.set_margin_end(16);
    let heading = gtk::Label::new(Some(&format!(
        "{} · {}",
        template.name,
        kind_label(template.kind)
    )));
    heading.add_css_class("heading");
    heading.set_xalign(0.0);
    root.append(&heading);
    let pane_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let mut editors = Vec::new();
    for (index, pane) in template.all_panes().enumerate() {
        let card = gtk::Box::new(gtk::Orientation::Vertical, 5);
        card.add_css_class("bookmark-row");
        let label = gtk::Label::new(Some(&format!("Pane {}", index + 1)));
        label.set_xalign(0.0);
        let command = gtk::Entry::new();
        command.set_placeholder_text(Some("Command (leave empty for a shell)"));
        command.set_text(pane.command.as_deref().unwrap_or_default());
        command.set_activates_default(true);
        card.append(&label);
        card.append(&command);
        let directory = (template.kind == TemplateKind::Bookmark).then(|| {
            let entry = gtk::Entry::new();
            entry.set_placeholder_text(Some("Working directory"));
            entry.set_text(pane.working_directory.as_deref().unwrap_or_default());
            entry.set_activates_default(true);
            card.append(&entry);
            entry
        });
        pane_box.append(&card);
        editors.push((command, directory));
    }
    let scroller = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&pane_box)
        .build();
    root.append(&scroller);
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    buttons.set_halign(gtk::Align::End);
    let cancel = gtk::Button::with_label("Cancel");
    let save = gtk::Button::with_label("Save changes");
    save.add_css_class("suggested-action");
    buttons.append(&cancel);
    buttons.append(&save);
    root.append(&buttons);
    dialog.set_child(Some(&root));
    dialog.set_default_widget(Some(&save));
    if let Some((first_command, _)) = editors.first() {
        let first_command = first_command.clone();
        dialog.connect_map(move |_| {
            let first_command = first_command.clone();
            gtk::glib::idle_add_local_once(move || {
                let focused = first_command.grab_focus();
                eprintln!("zentty-linux: bookmark-edit-command-focused={focused}");
            });
        });
    }
    connect_dialog_cancel(&dialog, &cancel, window);
    let action_window = window.clone();
    let action_dialog = dialog.downgrade();
    save.connect_clicked(move |_| {
        let mut edited = template.clone();
        for (pane, (command, directory)) in edited.all_panes_mut().zip(&editors) {
            pane.command = trimmed_entry(command);
            if let Some(directory) = directory {
                pane.working_directory = trimmed_entry(directory);
            }
            pane.was_user_edited = true;
        }
        let Ok(json) = serde_json::to_string(&edited) else {
            return;
        };
        if action_window
            .activate_action(
                "workspace.edit-template",
                Some(&(edited.id.as_str(), json.as_str()).to_variant()),
            )
            .is_ok()
            && let Some(dialog) = action_dialog.upgrade()
        {
            close_dialog_and_present_parent(&dialog, &action_window);
        }
    });
    dialog.present();
}

fn trimmed_entry(entry: &gtk::Entry) -> Option<String> {
    let value = entry.text();
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

fn present_name_dialog(
    window: &gtk::Window,
    title: &str,
    kind: TemplateKind,
    existing_id: Option<&str>,
    current_name: &str,
) {
    let dialog = gtk::Window::builder()
        .title(title)
        .transient_for(window)
        .modal(true)
        .default_width(360)
        .build();
    let content = gtk::Box::new(gtk::Orientation::Vertical, 10);
    content.set_margin_top(16);
    content.set_margin_bottom(16);
    content.set_margin_start(16);
    content.set_margin_end(16);
    let label = gtk::Label::new(Some(if existing_id.is_some() {
        "Template name"
    } else {
        match kind {
            TemplateKind::Bookmark => "Save working directories and commands as a bookmark",
            TemplateKind::Preset => "Save portable commands and layout as a preset",
        }
    }));
    label.set_xalign(0.0);
    label.set_wrap(true);
    let entry = gtk::Entry::new();
    entry.set_text(current_name);
    entry.set_placeholder_text(Some("Name"));
    entry.set_activates_default(true);
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    buttons.set_halign(gtk::Align::End);
    let cancel = gtk::Button::with_label("Cancel");
    let save = gtk::Button::with_label("Save");
    save.add_css_class("suggested-action");
    buttons.append(&cancel);
    buttons.append(&save);
    content.append(&label);
    content.append(&entry);
    content.append(&buttons);
    dialog.set_child(Some(&content));
    dialog.set_default_widget(Some(&save));
    let focus_entry = entry.clone();
    dialog.connect_map(move |_| {
        let focus_entry = focus_entry.clone();
        gtk::glib::idle_add_local_once(move || {
            let focused = focus_entry.grab_focus();
            eprintln!("zentty-linux: bookmark-name-focused={focused}");
        });
    });
    connect_dialog_cancel(&dialog, &cancel, window);
    let action_window = window.clone();
    let action_dialog = dialog.downgrade();
    let action_entry = entry.clone();
    let existing_id = existing_id.map(str::to_owned);
    let original_name = current_name.to_owned();
    save.connect_clicked(move |_| {
        let name = action_entry.text();
        if name.trim().is_empty() {
            return;
        }
        eprintln!(
            "zentty-linux: bookmark-name-submit changed={} chars={}",
            name.as_str() != original_name,
            name.chars().count()
        );
        let activated = if let Some(id) = &existing_id {
            action_window.activate_action(
                "workspace.rename-template",
                Some(&(id.as_str(), name.as_str()).to_variant()),
            )
        } else {
            action_window.activate_action(
                "workspace.save-template",
                Some(&(kind_label(kind), name.as_str()).to_variant()),
            )
        };
        if activated.is_ok()
            && let Some(dialog) = action_dialog.upgrade()
        {
            close_dialog_and_present_parent(&dialog, &action_window);
        }
    });
    dialog.present();
}

fn connect_dialog_cancel(dialog: &gtk::Window, cancel: &gtk::Button, parent: &gtk::Window) {
    let dialog = dialog.downgrade();
    let parent = parent.clone();
    cancel.connect_clicked(move |_| {
        if let Some(dialog) = dialog.upgrade() {
            close_dialog_and_present_parent(&dialog, &parent);
        }
    });
}

fn close_dialog_and_present_parent(dialog: &gtk::Window, parent: &gtk::Window) {
    let handler = Rc::new(RefCell::new(None));
    let callback_handler = Rc::clone(&handler);
    let handler_id = parent.connect_is_active_notify(move |parent| {
        if !parent.is_active() {
            return;
        }
        if let Some(handler_id) = callback_handler.borrow_mut().take() {
            parent.disconnect(handler_id);
        }
        eprintln!("zentty-linux: bookmark-dialog parent-active=true");
    });
    *handler.borrow_mut() = Some(handler_id);
    dialog.close();
    parent.present();
    eprintln!("zentty-linux: bookmark-dialog closed parent-presented=true");
    if parent.is_active() {
        if let Some(handler_id) = handler.borrow_mut().take() {
            parent.disconnect(handler_id);
            eprintln!("zentty-linux: bookmark-dialog parent-active=true");
        }
    } else {
        eprintln!("zentty-linux: bookmark-dialog parent-active=pending");
    }
}

fn kind_label(kind: TemplateKind) -> &'static str {
    match kind {
        TemplateKind::Bookmark => "Bookmark",
        TemplateKind::Preset => "Preset",
    }
}

fn find_named_widget(root: &gtk::Widget, name: &str) -> Option<gtk::Widget> {
    if root.widget_name() == name {
        return Some(root.clone());
    }
    let mut child = root.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        if let Some(found) = find_named_widget(&widget, name) {
            return Some(found);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::is_context_menu_shortcut;
    use gtk::gdk;

    #[test]
    fn template_rows_expose_standard_keyboard_context_menu_shortcuts() {
        assert!(is_context_menu_shortcut(
            gdk::Key::Menu,
            gdk::ModifierType::empty()
        ));
        assert!(is_context_menu_shortcut(
            gdk::Key::F10,
            gdk::ModifierType::SHIFT_MASK
        ));
        assert!(!is_context_menu_shortcut(
            gdk::Key::F10,
            gdk::ModifierType::empty()
        ));
    }
}
