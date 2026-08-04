use gtk::glib::variant::ToVariant;
use gtk::prelude::*;
use zentty_core::SidebarWorklaneSummary;

use crate::source_ui;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PaneActionSpec {
    label: &'static str,
    icon: &'static str,
    action: &'static str,
}

const PANE_ACTIONS: [PaneActionSpec; 8] = [
    PaneActionSpec {
        label: source_ui::SPLIT_RIGHT,
        icon: "go-next-symbolic",
        action: "split-pane-right",
    },
    PaneActionSpec {
        label: source_ui::ADD_PANE_RIGHT,
        icon: "application-add-symbolic",
        action: "add-pane-right",
    },
    PaneActionSpec {
        label: source_ui::NEW_PANE_BELOW,
        icon: "go-down-symbolic",
        action: "split-pane-below",
    },
    PaneActionSpec {
        label: source_ui::MOVE_PANE_LEFT,
        icon: "go-previous-symbolic",
        action: "move-pane-left",
    },
    PaneActionSpec {
        label: source_ui::MOVE_PANE_RIGHT,
        icon: "go-next-symbolic",
        action: "move-pane-right",
    },
    PaneActionSpec {
        label: source_ui::MOVE_PANE_UP,
        icon: "go-up-symbolic",
        action: "move-pane-up",
    },
    PaneActionSpec {
        label: source_ui::MOVE_PANE_DOWN,
        icon: "go-down-symbolic",
        action: "move-pane-down",
    },
    PaneActionSpec {
        label: source_ui::CLOSE_PANE,
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
         .worklane-context-action { padding: 5px 8px; }\n\
         .worklane-color-choice { min-width: 28px; min-height: 28px; padding: 0; border-radius: 14px; }\n\
         .worklane-color-red { color: #f56565; } .worklane-color-orange { color: #ed8936; }\n\
         .worklane-color-amber { color: #d69e2e; } .worklane-color-yellow { color: #ecc94b; }\n\
         .worklane-color-lime { color: #9ae6b4; } .worklane-color-green { color: #48bb78; }\n\
         .worklane-color-teal { color: #38b2ac; } .worklane-color-cyan { color: #4fd1c5; }\n\
         .worklane-color-blue { color: #4299e1; } .worklane-color-indigo { color: #667eea; }\n\
         .worklane-color-purple { color: #9f7aea; } .worklane-color-pink { color: #ed64a6; }\n\
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
    let add_label = gtk::Label::new(Some(source_ui::NEW_WORKLANE));
    add_label.set_xalign(0.0);
    add_label.set_hexpand(true);
    add_content.append(&add_label);
    add.set_child(Some(&add_content));
    add.set_tooltip_text(Some(source_ui::NEW_WORKLANE));
    add.set_accessible_role(gtk::AccessibleRole::Button);
    add.update_property(&[gtk::accessible::Property::Label(source_ui::NEW_WORKLANE)]);
    add.set_action_name(Some("workspace.new-worklane"));
    header.append(&add);
    sidebar.append(&header);

    for (index, summary) in summaries.iter().enumerate() {
        sidebar.append(&make_worklane_card(window, summary, index, summaries.len()));
    }
}

pub(crate) fn clear(sidebar: &gtk::Box) {
    remove_all_children(sidebar);
}

fn make_worklane_card(
    window: &gtk::Window,
    summary: &SidebarWorklaneSummary,
    index: usize,
    worklane_count: usize,
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
    menu.set_accessible_role(gtk::AccessibleRole::Button);
    menu.update_property(&[gtk::accessible::Property::Label("Worklane actions")]);
    menu.set_popover(Some(&make_context_menu(
        window,
        summary,
        index,
        worklane_count,
    )));
    header.append(&menu);
    card.append(&header);

    for pane in &summary.pane_rows {
        card.append(&make_pane_row(window, summary, pane));
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

fn make_pane_row(
    window: &gtk::Window,
    summary: &SidebarWorklaneSummary,
    pane: &zentty_core::SidebarPaneSummary,
) -> gtk::Box {
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
    select.set_accessible_role(gtk::AccessibleRole::Button);
    select.update_property(&[gtk::accessible::Property::Label(pane.primary_text.as_str())]);
    select.update_state(&[gtk::accessible::State::Selected(Some(pane.is_focused))]);
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
        pane,
        summary.pane_rows.len() > 1,
    )));
    row.append(&pane_menu);
    row
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
            eprintln!(
                "zentty-linux: pane-display id={} label={:?} custom={}",
                pane.pane_id,
                pane.primary_text,
                pane.custom_title.is_some()
            );
            let Some(select) = row
                .first_child()
                .and_then(|child| child.downcast::<gtk::Button>().ok())
            else {
                return false;
            };
            select.update_property(&[gtk::accessible::Property::Label(pane.primary_text.as_str())]);
            select.update_state(&[gtk::accessible::State::Selected(Some(pane.is_focused))]);
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

pub(crate) fn worklane_card(sidebar: &gtk::Box, worklane_id: &str) -> Option<gtk::Widget> {
    find_named_widget(
        sidebar.upcast_ref(),
        &widget_name("worklane-card", worklane_id),
    )
}

pub(crate) fn reveal_range(
    viewport_top: f64,
    viewport_height: f64,
    card_top: f64,
    card_height: f64,
) -> Option<(f64, f64)> {
    let card_bottom = card_top + card_height;
    let viewport_bottom = viewport_top + viewport_height;
    (card_top < viewport_top || card_bottom > viewport_bottom).then_some((card_top, card_bottom))
}

fn make_pane_context_menu(
    window: &gtk::Window,
    worklane_id: &str,
    pane: &zentty_core::SidebarPaneSummary,
    can_close: bool,
) -> gtk::Popover {
    let popover = gtk::Popover::new();
    let menu = gtk::Box::new(gtk::Orientation::Vertical, 2);
    menu.set_margin_top(6);
    menu.set_margin_bottom(6);
    menu.set_margin_start(6);
    menu.set_margin_end(6);

    let rename = menu_button(source_ui::RENAME_PANE, "document-edit-symbolic");
    let rename_window = window.clone();
    let pane_id = pane.pane_id.clone();
    let current_title = pane.custom_title.clone().unwrap_or_default();
    let rename_popover = popover.clone();
    rename.connect_clicked(move |_| {
        rename_popover.popdown();
        present_rename_dialog(
            &rename_window,
            "Rename Pane",
            "workspace.rename-pane",
            &pane_id,
            &current_title,
        );
    });
    menu.append(&rename);

    for action in pane_action_specs() {
        if action.action == "close-pane" && !can_close {
            continue;
        }
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
        let pane_id = pane.pane_id.clone();
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

fn make_context_menu(
    window: &gtk::Window,
    summary: &SidebarWorklaneSummary,
    index: usize,
    worklane_count: usize,
) -> gtk::Popover {
    let popover = gtk::Popover::new();
    let menu = gtk::Box::new(gtk::Orientation::Vertical, 2);
    menu.set_margin_top(6);
    menu.set_margin_bottom(6);
    menu.set_margin_start(6);
    menu.set_margin_end(6);

    let rename = menu_button(source_ui::RENAME_WORKLANE, "document-edit-symbolic");
    let rename_window = window.clone();
    let worklane_id = summary.worklane_id.clone();
    let current_title = summary.top_label.clone().unwrap_or_default();
    let rename_popover = popover.clone();
    rename.connect_clicked(move |_| {
        rename_popover.popdown();
        present_rename_dialog(
            &rename_window,
            "Rename Worklane",
            "workspace.rename-worklane",
            &worklane_id,
            &current_title,
        );
    });
    menu.append(&rename);

    let close = menu_button(source_ui::CLOSE_WORKLANE, "edit-delete-symbolic");
    close.set_sensitive(worklane_count > 1);
    close.set_action_name(Some("workspace.close-worklane"));
    close.set_action_target_value(Some(&summary.worklane_id.to_variant()));
    let close_popover = popover.clone();
    close.connect_clicked(move |_| close_popover.popdown());
    menu.append(&close);

    if index > 0 {
        menu.append(&targeted_move_button(
            source_ui::MOVE_WORKLANE_UP,
            "go-up-symbolic",
            &summary.worklane_id,
            "up",
            &popover,
        ));
    }
    if index + 1 < worklane_count {
        menu.append(&targeted_move_button(
            source_ui::MOVE_WORKLANE_DOWN,
            "go-down-symbolic",
            &summary.worklane_id,
            "down",
            &popover,
        ));
    }

    let color_heading = gtk::Label::new(Some(source_ui::WORKLANE_COLOR));
    color_heading.set_xalign(0.0);
    color_heading.set_margin_top(4);
    color_heading.update_property(&[gtk::accessible::Property::Label(source_ui::WORKLANE_COLOR)]);
    menu.append(&color_heading);
    let colors = gtk::FlowBox::new();
    colors.set_selection_mode(gtk::SelectionMode::None);
    colors.set_max_children_per_line(7);
    colors.set_min_children_per_line(7);
    colors.set_row_spacing(4);
    colors.set_column_spacing(4);
    colors.insert(
        &color_button(summary, None, "No worklane color", "×", &popover),
        -1,
    );
    for color in zentty_core::WorklaneColor::ALL {
        let label = format!("{} worklane color", color.as_str());
        colors.insert(
            &color_button(summary, Some(color), &label, "●", &popover),
            -1,
        );
    }
    menu.append(&colors);
    popover.set_child(Some(&menu));
    popover
}

fn menu_button(label: &str, icon: &str) -> gtk::Button {
    let button = gtk::Button::new();
    button.add_css_class("worklane-context-action");
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    content.append(&gtk::Image::from_icon_name(icon));
    let text = gtk::Label::new(Some(label));
    text.set_xalign(0.0);
    text.set_hexpand(true);
    content.append(&text);
    button.set_child(Some(&content));
    button.set_tooltip_text(Some(label));
    button.update_property(&[gtk::accessible::Property::Label(label)]);
    button
}

fn targeted_move_button(
    label: &str,
    icon: &str,
    worklane_id: &str,
    direction: &str,
    popover: &gtk::Popover,
) -> gtk::Button {
    let button = menu_button(label, icon);
    button.set_action_name(Some("workspace.move-worklane"));
    button.set_action_target_value(Some(&(worklane_id, direction).to_variant()));
    let popover = popover.clone();
    button.connect_clicked(move |_| popover.popdown());
    button
}

fn color_button(
    summary: &SidebarWorklaneSummary,
    color: Option<zentty_core::WorklaneColor>,
    label: &str,
    glyph: &str,
    popover: &gtk::Popover,
) -> gtk::Button {
    let selected = summary.color == color;
    let button = gtk::Button::with_label(if selected { "✓" } else { glyph });
    button.add_css_class("worklane-color-choice");
    if let Some(color) = color {
        button.add_css_class(&format!("worklane-color-{}", color.as_str()));
    }
    button.set_tooltip_text(Some(label));
    button.update_property(&[gtk::accessible::Property::Label(label)]);
    button.update_state(&[gtk::accessible::State::Selected(Some(selected))]);
    button.set_action_name(Some("workspace.set-worklane-color"));
    button.set_action_target_value(Some(
        &(
            summary.worklane_id.as_str(),
            color.map_or("", zentty_core::WorklaneColor::as_str),
        )
            .to_variant(),
    ));
    let popover = popover.clone();
    button.connect_clicked(move |_| popover.popdown());
    button
}

fn present_rename_dialog(
    window: &gtk::Window,
    title: &str,
    action_name: &str,
    target_id: &str,
    current_title: &str,
) {
    let dialog = gtk::Window::builder()
        .title(title)
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
    let target_id = target_id.to_owned();
    let action_name = action_name.to_owned();
    let action_window = window.clone();
    let save_dialog = dialog.clone();
    let save_entry = entry.clone();
    save.connect_clicked(move |_| {
        let _ = action_window.activate_action(
            &action_name,
            Some(&(target_id.as_str(), save_entry.text().as_str()).to_variant()),
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
    use super::{WorklaneSelectionState, pane_action_specs, reveal_range, selection_state};
    use crate::source_ui;

    #[test]
    fn pane_actions_are_contextual_and_source_named() {
        let actions = pane_action_specs();
        assert_eq!(
            actions
                .iter()
                .map(|action| (action.label, action.action))
                .collect::<Vec<_>>(),
            [
                (source_ui::SPLIT_RIGHT, "split-pane-right"),
                (source_ui::ADD_PANE_RIGHT, "add-pane-right"),
                (source_ui::NEW_PANE_BELOW, "split-pane-below"),
                (source_ui::MOVE_PANE_LEFT, "move-pane-left"),
                (source_ui::MOVE_PANE_RIGHT, "move-pane-right"),
                (source_ui::MOVE_PANE_UP, "move-pane-up"),
                (source_ui::MOVE_PANE_DOWN, "move-pane-down"),
                (source_ui::CLOSE_PANE, "close-pane"),
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

    #[test]
    fn active_worklane_reveal_only_scrolls_when_the_whole_card_is_not_visible() {
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../Zentty/UI/Sidebar/SidebarActiveWorklaneAutoScroller.swift"
        ));
        assert!(source.contains("if !isVisible(worklaneID)"));

        assert_eq!(reveal_range(100.0, 300.0, 125.0, 80.0), None);
        assert_eq!(reveal_range(100.0, 300.0, 80.0, 80.0), Some((80.0, 160.0)));
        assert_eq!(
            reveal_range(100.0, 300.0, 360.0, 80.0),
            Some((360.0, 440.0))
        );
    }
}
