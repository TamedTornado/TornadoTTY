use gtk::glib::variant::ToVariant;
use gtk::prelude::*;
use zentty_core::SidebarWorklaneSummary;

pub(crate) fn install_styles() {
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };
    let provider = gtk::CssProvider::new();
    provider.load_from_string(
        ".zentty-sidebar { background: #17191d; padding: 10px; }\n\
         .zentty-sidebar-header { font-weight: 700; font-size: 15px; }\n\
         .worklane-card { background: #202329; border: 1px solid #30343b; border-radius: 10px; padding: 7px; }\n\
         .worklane-card-active { background: #272b32; border-color: #596273; }\n\
         .worklane-card-red { border-left: 4px solid #f56565; }\n\
         .worklane-card-orange { border-left: 4px solid #ed8936; }\n\
         .worklane-card-amber { border-left: 4px solid #d69e2e; }\n\
         .worklane-card-yellow { border-left: 4px solid #ecc94b; }\n\
         .worklane-card-lime { border-left: 4px solid #9ae6b4; }\n\
         .worklane-card-green { border-left: 4px solid #48bb78; }\n\
         .worklane-card-teal { border-left: 4px solid #38b2ac; }\n\
         .worklane-card-cyan { border-left: 4px solid #4fd1c5; }\n\
         .worklane-card-blue { border-left: 4px solid #4299e1; }\n\
         .worklane-card-indigo { border-left: 4px solid #667eea; }\n\
         .worklane-card-purple { border-left: 4px solid #9f7aea; }\n\
         .worklane-card-pink { border-left: 4px solid #ed64a6; }\n\
         .worklane-title { font-weight: 700; }\n\
         .worklane-context { color: #a7adb8; font-size: 12px; }\n\
         .pane-row { border-radius: 6px; padding: 5px 7px; }\n\
         .pane-row-focused { background: #343943; }\n\
         .pane-marker { color: #69db7c; }",
    );
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

pub(crate) fn render(
    sidebar: &gtk::Box,
    window: &gtk::Window,
    summaries: &[SidebarWorklaneSummary],
) {
    remove_all_children(sidebar);
    sidebar.add_css_class("zentty-sidebar");

    let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let title = gtk::Label::new(Some("Worklanes"));
    title.add_css_class("zentty-sidebar-header");
    title.set_xalign(0.0);
    title.set_hexpand(true);
    let add = gtk::Button::with_label("+");
    add.set_tooltip_text(Some("New worklane"));
    add.set_accessible_role(gtk::AccessibleRole::Button);
    add.set_action_name(Some("workspace.new-worklane"));
    header.append(&title);
    header.append(&add);
    sidebar.append(&header);

    for (index, summary) in summaries.iter().enumerate() {
        sidebar.append(&make_worklane_card(window, summary, index));
    }
}

pub(crate) fn clear(sidebar: &gtk::Box) {
    remove_all_children(sidebar);
}

fn make_worklane_card(
    window: &gtk::Window,
    summary: &SidebarWorklaneSummary,
    index: usize,
) -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 4);
    card.add_css_class("worklane-card");
    if summary.is_active {
        card.add_css_class("worklane-card-active");
    }
    if let Some(color) = summary.color {
        card.add_css_class(&format!("worklane-card-{}", color.as_str()));
    }

    let header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let select = gtk::Button::new();
    select.set_has_frame(false);
    select.set_hexpand(true);
    select.set_action_name(Some("workspace.select-worklane"));
    select.set_action_target_value(Some(&summary.worklane_id.to_variant()));

    let heading = gtk::Box::new(gtk::Orientation::Vertical, 1);
    let top = summary
        .top_label
        .clone()
        .unwrap_or_else(|| format!("Worklane {}", index + 1));
    let top_label = gtk::Label::new(Some(&top));
    top_label.add_css_class("worklane-title");
    top_label.set_xalign(0.0);
    top_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    let context = gtk::Label::new(Some(&summary.primary_text));
    context.add_css_class("worklane-context");
    context.set_xalign(0.0);
    context.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    heading.append(&top_label);
    heading.append(&context);
    select.set_child(Some(&heading));
    header.append(&select);

    let menu = gtk::MenuButton::new();
    menu.set_icon_name("view-more-symbolic");
    menu.set_tooltip_text(Some("Worklane actions"));
    menu.set_popover(Some(&make_context_menu(window, summary)));
    header.append(&menu);
    card.append(&header);

    for pane in &summary.pane_rows {
        let row = gtk::Button::new();
        row.set_has_frame(false);
        row.add_css_class("pane-row");
        if pane.is_focused {
            row.add_css_class("pane-row-focused");
        }
        row.set_action_name(Some("workspace.select-pane"));
        row.set_action_target_value(Some(
            &(summary.worklane_id.as_str(), pane.pane_id.as_str()).to_variant(),
        ));
        let pane_content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let marker = gtk::Label::new(Some(if pane.is_focused { "●" } else { "○" }));
        marker.add_css_class("pane-marker");
        let pane_title = gtk::Label::new(Some(&pane.primary_text));
        pane_title.set_xalign(0.0);
        pane_title.set_hexpand(true);
        pane_title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        pane_content.append(&marker);
        pane_content.append(&pane_title);
        row.set_child(Some(&pane_content));
        card.append(&row);
    }

    eprintln!(
        "zentty-linux: sidebar-card id={} panes={} active={} title={:?}",
        summary.worklane_id,
        summary.pane_rows.len(),
        summary.is_active,
        summary.top_label
    );
    card
}

fn make_context_menu(window: &gtk::Window, summary: &SidebarWorklaneSummary) -> gtk::Popover {
    let popover = gtk::Popover::new();
    let menu = gtk::Box::new(gtk::Orientation::Vertical, 3);
    menu.set_margin_top(6);
    menu.set_margin_bottom(6);
    menu.set_margin_start(6);
    menu.set_margin_end(6);

    let rename = gtk::Button::with_label("Rename Worklane…");
    let rename_window = window.clone();
    let worklane_id = summary.worklane_id.clone();
    let current_title = summary.top_label.clone().unwrap_or_default();
    rename.connect_clicked(move |_| {
        present_rename_dialog(&rename_window, &worklane_id, &current_title);
    });
    menu.append(&rename);

    for (label, action) in [
        ("Move Up", "workspace.move-worklane-up"),
        ("Move Down", "workspace.move-worklane-down"),
        ("Next Color", "workspace.cycle-worklane-color"),
    ] {
        let button = gtk::Button::with_label(label);
        let action_window = window.clone();
        let worklane_id = summary.worklane_id.clone();
        button.connect_clicked(move |_| {
            let _ = action_window
                .activate_action("workspace.select-worklane", Some(&worklane_id.to_variant()));
            let _ = action_window.activate_action(action, None);
        });
        menu.append(&button);
    }
    popover.set_child(Some(&menu));
    popover
}

fn present_rename_dialog(window: &gtk::Window, worklane_id: &str, current_title: &str) {
    let dialog = gtk::Window::builder()
        .title("Rename Worklane")
        .transient_for(window)
        .modal(true)
        .default_width(320)
        .build();
    let content = gtk::Box::new(gtk::Orientation::Vertical, 10);
    content.set_margin_top(16);
    content.set_margin_bottom(16);
    content.set_margin_start(16);
    content.set_margin_end(16);
    let instruction = gtk::Label::new(Some("Leave empty to remove the name."));
    instruction.set_xalign(0.0);
    let entry = gtk::Entry::new();
    entry.set_text(current_title);
    entry.set_activates_default(true);
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    buttons.set_halign(gtk::Align::End);
    let cancel = gtk::Button::with_label("Cancel");
    let save = gtk::Button::with_label("Save");
    save.add_css_class("suggested-action");
    buttons.append(&cancel);
    buttons.append(&save);
    content.append(&instruction);
    content.append(&entry);
    content.append(&buttons);
    dialog.set_child(Some(&content));
    dialog.set_default_widget(Some(&save));

    let cancel_dialog = dialog.clone();
    cancel.connect_clicked(move |_| cancel_dialog.close());
    let worklane_id = worklane_id.to_owned();
    let action_window = window.clone();
    let save_dialog = dialog.clone();
    let save_entry = entry.clone();
    save.connect_clicked(move |_| {
        let _ = action_window.activate_action(
            "workspace.rename-worklane",
            Some(&(worklane_id.as_str(), save_entry.text().as_str()).to_variant()),
        );
        save_dialog.close();
    });
    dialog.present();
    entry.grab_focus();
}

fn remove_all_children(container: &gtk::Box) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}
