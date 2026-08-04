use gtk::glib::variant::ToVariant;
use gtk::prelude::*;
use zentty_core::SidebarWorklaneSummary;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PaneActionSpec {
    label: &'static str,
    icon: &'static str,
    action: &'static str,
}

const PANE_ACTIONS: [PaneActionSpec; 7] = [
    PaneActionSpec {
        label: "New Pane Right",
        icon: "go-next-symbolic",
        action: "split-pane-right",
    },
    PaneActionSpec {
        label: "New Pane Below",
        icon: "go-down-symbolic",
        action: "split-pane-below",
    },
    PaneActionSpec {
        label: "Move Pane Left",
        icon: "go-previous-symbolic",
        action: "move-pane-left",
    },
    PaneActionSpec {
        label: "Move Pane Right",
        icon: "go-next-symbolic",
        action: "move-pane-right",
    },
    PaneActionSpec {
        label: "Move Pane Up",
        icon: "go-up-symbolic",
        action: "move-pane-up",
    },
    PaneActionSpec {
        label: "Move Pane Down",
        icon: "go-down-symbolic",
        action: "move-pane-down",
    },
    PaneActionSpec {
        label: "Close Pane",
        icon: "edit-delete-symbolic",
        action: "close-pane",
    },
];

fn pane_action_specs() -> &'static [PaneActionSpec] {
    &PANE_ACTIONS
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WorklaneSelectionState {
    Active,
    Inactive,
}

impl WorklaneSelectionState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Inactive => "inactive",
        }
    }

    fn css_class(self) -> &'static str {
        match self {
            Self::Active => "worklane-tint-active",
            Self::Inactive => "worklane-tint-inactive",
        }
    }
}

fn selection_state(is_active: bool) -> WorklaneSelectionState {
    if is_active {
        WorklaneSelectionState::Active
    } else {
        WorklaneSelectionState::Inactive
    }
}

pub(crate) fn install_styles() {
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };
    let provider = gtk::CssProvider::new();
    provider.load_from_string(
        ".zentty-sidebar { background: #17191d; color: #e7e9ed; padding: 10px; }\n\
         .zentty-sidebar-header { color: #f1f3f5; font-weight: 700; font-size: 15px; }\n\
         .worklane-card { background: #1e2126; border: 1px solid #30343b; border-radius: 10px; padding: 7px; }\n\
         .worklane-card-active { background: #343a45; border-color: #7c8799; box-shadow: 0 4px 14px rgba(0, 0, 0, 0.35); }\n\
         .worklane-tint-inactive.worklane-card-red { border-left: 4px solid rgba(245, 101, 101, 0.34); }\n\
         .worklane-tint-inactive.worklane-card-orange { border-left: 4px solid rgba(237, 137, 54, 0.34); }\n\
         .worklane-tint-inactive.worklane-card-amber { border-left: 4px solid rgba(214, 158, 46, 0.34); }\n\
         .worklane-tint-inactive.worklane-card-yellow { border-left: 4px solid rgba(236, 201, 75, 0.34); }\n\
         .worklane-tint-inactive.worklane-card-lime { border-left: 4px solid rgba(154, 230, 180, 0.34); }\n\
         .worklane-tint-inactive.worklane-card-green { border-left: 4px solid rgba(72, 187, 120, 0.34); }\n\
         .worklane-tint-inactive.worklane-card-teal { border-left: 4px solid rgba(56, 178, 172, 0.34); }\n\
         .worklane-tint-inactive.worklane-card-cyan { border-left: 4px solid rgba(79, 209, 197, 0.34); }\n\
         .worklane-tint-inactive.worklane-card-blue { border-left: 4px solid rgba(66, 153, 225, 0.34); }\n\
         .worklane-tint-inactive.worklane-card-indigo { border-left: 4px solid rgba(102, 126, 234, 0.34); }\n\
         .worklane-tint-inactive.worklane-card-purple { border-left: 4px solid rgba(159, 122, 234, 0.34); }\n\
         .worklane-tint-inactive.worklane-card-pink { border-left: 4px solid rgba(237, 100, 166, 0.34); }\n\
         .worklane-tint-active.worklane-card-red { background: rgba(245, 101, 101, 0.20); border-color: rgba(245, 101, 101, 0.62); border-left: 4px solid rgba(245, 101, 101, 0.95); }\n\
         .worklane-tint-active.worklane-card-orange { background: rgba(237, 137, 54, 0.20); border-color: rgba(237, 137, 54, 0.62); border-left: 4px solid rgba(237, 137, 54, 0.95); }\n\
         .worklane-tint-active.worklane-card-amber { background: rgba(214, 158, 46, 0.20); border-color: rgba(214, 158, 46, 0.62); border-left: 4px solid rgba(214, 158, 46, 0.95); }\n\
         .worklane-tint-active.worklane-card-yellow { background: rgba(236, 201, 75, 0.20); border-color: rgba(236, 201, 75, 0.62); border-left: 4px solid rgba(236, 201, 75, 0.95); }\n\
         .worklane-tint-active.worklane-card-lime { background: rgba(154, 230, 180, 0.20); border-color: rgba(154, 230, 180, 0.62); border-left: 4px solid rgba(154, 230, 180, 0.95); }\n\
         .worklane-tint-active.worklane-card-green { background: rgba(72, 187, 120, 0.20); border-color: rgba(72, 187, 120, 0.62); border-left: 4px solid rgba(72, 187, 120, 0.95); }\n\
         .worklane-tint-active.worklane-card-teal { background: rgba(56, 178, 172, 0.20); border-color: rgba(56, 178, 172, 0.62); border-left: 4px solid rgba(56, 178, 172, 0.95); }\n\
         .worklane-tint-active.worklane-card-cyan { background: rgba(79, 209, 197, 0.20); border-color: rgba(79, 209, 197, 0.62); border-left: 4px solid rgba(79, 209, 197, 0.95); }\n\
         .worklane-tint-active.worklane-card-blue { background: rgba(66, 153, 225, 0.20); border-color: rgba(66, 153, 225, 0.62); border-left: 4px solid rgba(66, 153, 225, 0.95); }\n\
         .worklane-tint-active.worklane-card-indigo { background: rgba(102, 126, 234, 0.20); border-color: rgba(102, 126, 234, 0.62); border-left: 4px solid rgba(102, 126, 234, 0.95); }\n\
         .worklane-tint-active.worklane-card-purple { background: rgba(159, 122, 234, 0.20); border-color: rgba(159, 122, 234, 0.62); border-left: 4px solid rgba(159, 122, 234, 0.95); }\n\
         .worklane-tint-active.worklane-card-pink { background: rgba(237, 100, 166, 0.20); border-color: rgba(237, 100, 166, 0.62); border-left: 4px solid rgba(237, 100, 166, 0.95); }\n\
         .worklane-title { color: #b8bec8; font-weight: 700; }\n\
         .worklane-card-active .worklane-title { color: #ffffff; }\n\
         .worklane-context { color: #a7adb8; font-size: 12px; }\n\
         .pane-row { color: #e7e9ed; border-radius: 6px; padding: 5px 7px; }\n\
         .pane-row-focused { background: #343943; }\n\
         .pane-marker { color: #727a86; }\n\
         .worklane-card-active .pane-marker { color: #69db7c; }\n\
         .sidebar-create-worklane { color: #d8dbe1; border-radius: 7px; padding: 5px 8px; }\n\
         .sidebar-pane-actions { color: #c7cbd2; min-width: 26px; min-height: 26px; padding: 2px; }\n\
         .pane-context-action { padding: 5px 8px; }\n\
         .zentty-window-chrome { background: #15171a; min-height: 38px; padding: 3px 10px; }\n\
         .zentty-window-context { color: #aeb4be; font-weight: 600; }\n\
         .zentty-chrome-icon { color: #d5d9df; min-width: 28px; min-height: 28px; padding: 0; border-radius: 7px; }",
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
    let add = gtk::Button::new();
    add.add_css_class("sidebar-create-worklane");
    add.set_hexpand(true);
    let add_content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    add_content.append(&gtk::Image::from_icon_name("list-add-symbolic"));
    let add_label = gtk::Label::new(Some("New worklane"));
    add_label.set_xalign(0.0);
    add_label.set_hexpand(true);
    add_content.append(&add_label);
    add.set_child(Some(&add_content));
    add.set_tooltip_text(Some("New worklane"));
    add.set_accessible_role(gtk::AccessibleRole::Button);
    add.update_property(&[gtk::accessible::Property::Label("New worklane")]);
    add.set_action_name(Some("workspace.new-worklane"));
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
    card.set_widget_name(&widget_name("worklane-card", &summary.worklane_id));
    card.add_css_class("worklane-card");
    if let Some(color) = summary.color {
        card.add_css_class(&format!("worklane-card-{}", color.as_str()));
    }
    apply_worklane_visual_state(card.upcast_ref(), summary);

    let header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let select = gtk::Button::new();
    select.set_widget_name(&widget_name("worklane-select", &summary.worklane_id));
    select.set_has_frame(false);
    select.set_hexpand(true);
    select.set_action_name(Some("workspace.select-worklane"));
    select.set_action_target_value(Some(&summary.worklane_id.to_variant()));
    select.set_accessible_role(gtk::AccessibleRole::Button);
    select.update_state(&[gtk::accessible::State::Selected(Some(summary.is_active))]);

    let heading = gtk::Box::new(gtk::Orientation::Vertical, 1);
    let top = summary
        .top_label
        .clone()
        .unwrap_or_else(|| format!("Worklane {}", index + 1));
    let accessible_label = format!("{top}, {}", summary.primary_text);
    select.update_property(&[gtk::accessible::Property::Label(&accessible_label)]);
    let top_label = gtk::Label::new(Some(&top));
    top_label.set_widget_name(&widget_name("worklane-title", &summary.worklane_id));
    top_label.add_css_class("worklane-title");
    top_label.set_xalign(0.0);
    top_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    let context = gtk::Label::new(Some(&summary.primary_text));
    context.set_widget_name(&widget_name("worklane-context", &summary.worklane_id));
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
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 2);
        row.set_widget_name(&widget_name("pane-row", &pane.pane_id));
        row.add_css_class("pane-row");
        if pane.is_focused {
            row.add_css_class("pane-row-focused");
        }
        let select = gtk::Button::new();
        select.set_has_frame(false);
        select.set_hexpand(true);
        select.set_action_name(Some("workspace.select-pane"));
        select.set_action_target_value(Some(
            &(summary.worklane_id.as_str(), pane.pane_id.as_str()).to_variant(),
        ));
        let pane_content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let marker = gtk::Label::new(Some(if pane.is_focused { "●" } else { "○" }));
        marker.set_widget_name(&widget_name("pane-marker", &pane.pane_id));
        marker.add_css_class("pane-marker");
        let pane_title = gtk::Label::new(Some(&pane.primary_text));
        pane_title.set_widget_name(&widget_name("pane-title", &pane.pane_id));
        pane_title.set_xalign(0.0);
        pane_title.set_hexpand(true);
        pane_title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        pane_content.append(&marker);
        pane_content.append(&pane_title);
        select.set_child(Some(&pane_content));
        row.append(&select);

        let pane_menu = gtk::MenuButton::new();
        pane_menu.add_css_class("sidebar-pane-actions");
        pane_menu.set_icon_name("view-more-symbolic");
        pane_menu.set_tooltip_text(Some("Pane actions"));
        pane_menu.set_accessible_role(gtk::AccessibleRole::Button);
        pane_menu.update_property(&[gtk::accessible::Property::Label("Pane actions")]);
        pane_menu.set_popover(Some(&make_pane_context_menu(
            window,
            &summary.worklane_id,
            &pane.pane_id,
        )));
        row.append(&pane_menu);
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

pub(crate) fn update_metadata(sidebar: &gtk::Box, summaries: &[SidebarWorklaneSummary]) -> bool {
    for (index, summary) in summaries.iter().enumerate() {
        let Some(card) = find_named_widget(
            sidebar.upcast_ref(),
            &widget_name("worklane-card", &summary.worklane_id),
        ) else {
            return false;
        };
        card.remove_css_class("worklane-tint-active");
        card.remove_css_class("worklane-tint-inactive");
        for color in zentty_core::WorklaneColor::ALL {
            card.remove_css_class(&format!("worklane-card-{}", color.as_str()));
        }
        if let Some(color) = summary.color {
            card.add_css_class(&format!("worklane-card-{}", color.as_str()));
        }
        apply_worklane_visual_state(&card, summary);

        let Some(select) = find_named_widget(
            sidebar.upcast_ref(),
            &widget_name("worklane-select", &summary.worklane_id),
        ) else {
            return false;
        };
        select.update_state(&[gtk::accessible::State::Selected(Some(summary.is_active))]);

        let Some(title) = find_named_label(
            sidebar.upcast_ref(),
            &widget_name("worklane-title", &summary.worklane_id),
        ) else {
            return false;
        };
        let title_text = summary
            .top_label
            .clone()
            .unwrap_or_else(|| format!("Worklane {}", index + 1));
        title.set_text(&title_text);
        let Some(context) = find_named_label(
            sidebar.upcast_ref(),
            &widget_name("worklane-context", &summary.worklane_id),
        ) else {
            return false;
        };
        context.set_text(&summary.primary_text);

        for pane in &summary.pane_rows {
            let Some(row) = find_named_widget(
                sidebar.upcast_ref(),
                &widget_name("pane-row", &pane.pane_id),
            ) else {
                return false;
            };
            if pane.is_focused {
                row.add_css_class("pane-row-focused");
            } else {
                row.remove_css_class("pane-row-focused");
            }
            let Some(marker) = find_named_label(
                sidebar.upcast_ref(),
                &widget_name("pane-marker", &pane.pane_id),
            ) else {
                return false;
            };
            marker.set_text(if pane.is_focused { "●" } else { "○" });
            let Some(title) = find_named_label(
                sidebar.upcast_ref(),
                &widget_name("pane-title", &pane.pane_id),
            ) else {
                return false;
            };
            title.set_text(&pane.primary_text);
        }
    }
    true
}

fn apply_worklane_visual_state(card: &gtk::Widget, summary: &SidebarWorklaneSummary) {
    let state = selection_state(summary.is_active);
    card.remove_css_class("worklane-card-active");
    card.remove_css_class("worklane-tint-active");
    card.remove_css_class("worklane-tint-inactive");
    card.add_css_class(state.css_class());
    if summary.is_active {
        card.add_css_class("worklane-card-active");
    }
    let tint = summary.color.map_or_else(
        || "none".to_owned(),
        |color| format!("{}-{}", color.as_str(), state.as_str()),
    );
    eprintln!(
        "zentty-linux: worklane-visual id={} selection={} tint={tint}",
        summary.worklane_id,
        state.as_str()
    );
}

fn widget_name(kind: &str, id: &str) -> String {
    format!("zentty-{kind}-{id}")
}

fn find_named_label(root: &gtk::Widget, name: &str) -> Option<gtk::Label> {
    find_named_widget(root, name)?.downcast::<gtk::Label>().ok()
}

fn find_named_widget(root: &gtk::Widget, name: &str) -> Option<gtk::Widget> {
    if root.widget_name() == name {
        return Some(root.clone());
    }
    let mut child = root.first_child();
    while let Some(widget) = child {
        if let Some(found) = find_named_widget(&widget, name) {
            return Some(found);
        }
        child = widget.next_sibling();
    }
    None
}

fn make_pane_context_menu(window: &gtk::Window, worklane_id: &str, pane_id: &str) -> gtk::Popover {
    let popover = gtk::Popover::new();
    let menu = gtk::Box::new(gtk::Orientation::Vertical, 2);
    menu.set_margin_top(6);
    menu.set_margin_bottom(6);
    menu.set_margin_start(6);
    menu.set_margin_end(6);

    for action in pane_action_specs() {
        let button = gtk::Button::new();
        button.add_css_class("pane-context-action");
        button.set_tooltip_text(Some(action.label));
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        content.append(&gtk::Image::from_icon_name(action.icon));
        let label = gtk::Label::new(Some(action.label));
        label.set_xalign(0.0);
        label.set_hexpand(true);
        content.append(&label);
        button.set_child(Some(&content));

        let action_window = window.clone();
        let worklane_id = worklane_id.to_owned();
        let pane_id = pane_id.to_owned();
        let action_name = action.action;
        button.connect_clicked(move |_| {
            let _ = action_window.activate_action(
                "workspace.select-pane",
                Some(&(worklane_id.as_str(), pane_id.as_str()).to_variant()),
            );
            let action_window = action_window.clone();
            gtk::glib::idle_add_local_once(move || {
                let _ = action_window.activate_action(&format!("workspace.{action_name}"), None);
            });
        });
        menu.append(&button);
    }

    popover.set_child(Some(&menu));
    popover
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

#[cfg(test)]
mod tests {
    use super::{WorklaneSelectionState, pane_action_specs, selection_state};

    #[test]
    fn pane_actions_are_contextual_and_source_named() {
        let actions = pane_action_specs();
        assert_eq!(
            actions
                .iter()
                .map(|action| (action.label, action.action))
                .collect::<Vec<_>>(),
            [
                ("New Pane Right", "split-pane-right"),
                ("New Pane Below", "split-pane-below"),
                ("Move Pane Left", "move-pane-left"),
                ("Move Pane Right", "move-pane-right"),
                ("Move Pane Up", "move-pane-up"),
                ("Move Pane Down", "move-pane-down"),
                ("Close Pane", "close-pane"),
            ]
        );
        assert!(actions.iter().all(|action| !action.icon.is_empty()));
    }

    #[test]
    fn worklane_selection_is_independent_from_persistent_identity_color() {
        assert_eq!(selection_state(true), WorklaneSelectionState::Active);
        assert_eq!(selection_state(false), WorklaneSelectionState::Inactive);
        assert_eq!(selection_state(true).css_class(), "worklane-tint-active");
        assert_eq!(selection_state(false).css_class(), "worklane-tint-inactive");
    }
}
