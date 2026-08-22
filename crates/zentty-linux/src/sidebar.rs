use gtk::glib::variant::ToVariant;
use gtk::prelude::*;
use std::cell::Cell;
use std::rc::Rc;
use zentty_core::{ClipboardConfig, RankedServer, ServerRelevanceTier, SidebarWorklaneSummary};

use crate::{agent_status_view, bookmarks_view, global_search_view, source_ui};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PaneActionSpec {
    label: &'static str,
    icon: &'static str,
    action: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WorklaneDestination {
    window_id: String,
    worklane_id: String,
    label: String,
    color: Option<zentty_core::WorklaneColor>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorklaneDestinationGroup {
    pub(crate) window_id: String,
    pub(crate) summaries: Vec<SidebarWorklaneSummary>,
}

fn worklane_destinations(
    groups: &[WorklaneDestinationGroup],
    source_window_id: &str,
    source_worklane_id: &str,
) -> Vec<Vec<WorklaneDestination>> {
    groups
        .iter()
        .filter_map(|group| {
            let destinations = group
                .summaries
                .iter()
                .filter(|summary| {
                    group.window_id != source_window_id || summary.worklane_id != source_worklane_id
                })
                .filter_map(|summary| {
                    let primary = summary.pane_rows.first()?;
                    let additional = summary.pane_rows.len() - 1;
                    Some(WorklaneDestination {
                        window_id: group.window_id.clone(),
                        worklane_id: summary.worklane_id.clone(),
                        label: if additional == 0 {
                            primary.primary_text.clone()
                        } else {
                            format!("{}  +{} more", primary.primary_text, additional)
                        },
                        color: summary.color,
                    })
                })
                .collect::<Vec<_>>();
            if destinations.is_empty() {
                None
            } else {
                Some(destinations)
            }
        })
        .collect()
}

fn local_destination_groups(
    window_id: &str,
    summaries: &[SidebarWorklaneSummary],
) -> Vec<WorklaneDestinationGroup> {
    vec![WorklaneDestinationGroup {
        window_id: window_id.to_owned(),
        summaries: summaries.to_vec(),
    }]
}

const PANE_ACTIONS: [PaneActionSpec; 14] = [
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
        label: source_ui::ADD_PANE_LEFT,
        icon: "go-previous-symbolic",
        action: "add-pane-left",
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
        label: source_ui::MOVE_PANE_TO_NEW_WINDOW,
        icon: "window-new-symbolic",
        action: "move-pane-to-new-window",
    },
    PaneActionSpec {
        label: source_ui::COPY,
        icon: "edit-copy-symbolic",
        action: "copy",
    },
    PaneActionSpec {
        label: source_ui::CLEAN_COPY,
        icon: "edit-copy-symbolic",
        action: "clean-copy",
    },
    PaneActionSpec {
        label: source_ui::COPY_RAW,
        icon: "edit-copy-symbolic",
        action: "copy-raw",
    },
    PaneActionSpec {
        label: source_ui::COPY_AS_MARKDOWN,
        icon: "text-x-generic-symbolic",
        action: "copy-as-markdown",
    },
    PaneActionSpec {
        label: source_ui::CLOSE_PANE,
        icon: "edit-delete-symbolic",
        action: "close-pane",
    },
];

fn pane_action_specs(clipboard: ClipboardConfig) -> impl Iterator<Item = &'static PaneActionSpec> {
    PANE_ACTIONS.iter().filter(move |action| {
        (action.action != "clean-copy" || !clipboard.always_clean_copies)
            && (action.action != "copy-raw" || clipboard.always_clean_copies)
            && (action.action != "copy-as-markdown" || clipboard.show_copy_markdown_command)
    })
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WorklaneDropEdge {
    Before,
    After,
}

impl WorklaneDropEdge {
    fn at(pointer_y: f64, card_height: i32) -> Self {
        if pointer_y < f64::from(card_height) / 2.0 {
            Self::Before
        } else {
            Self::After
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Before => "before",
            Self::After => "after",
        }
    }
}

pub(crate) fn install_styles() {
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };
    let provider = gtk::CssProvider::new();
    provider.load_from_string(
        ".zentty-sidebar { background: #17191d; color: #e7e9ed; padding: 10px; }\n\
         .zentty-sidebar-floating { background: #17191d; border-right: 1px solid #4a5260; box-shadow: 10px 0 24px rgba(0, 0, 0, 0.45); }\n\
         .zentty-sidebar-header { color: #f1f3f5; font-weight: 700; font-size: 15px; }\n\
         .worklane-card { background: #1e2126; border: 1px solid #30343b; border-radius: 10px; padding: 7px; }\n\
         .worklane-drag-preview { border-color: #65a7ff; box-shadow: 0 8px 22px rgba(0, 0, 0, 0.62), inset 0 0 0 1px alpha(#65a7ff, 0.72); }\n\
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
         .sidebar-emphasis-vivid .worklane-tint-active.worklane-card-red { background: rgba(245, 101, 101, 0.20); border-color: rgba(245, 101, 101, 0.62); border-left: 4px solid rgba(245, 101, 101, 0.95); }\n\
         .sidebar-emphasis-vivid .worklane-tint-active.worklane-card-orange { background: rgba(237, 137, 54, 0.20); border-color: rgba(237, 137, 54, 0.62); border-left: 4px solid rgba(237, 137, 54, 0.95); }\n\
         .sidebar-emphasis-vivid .worklane-tint-active.worklane-card-amber { background: rgba(214, 158, 46, 0.20); border-color: rgba(214, 158, 46, 0.62); border-left: 4px solid rgba(214, 158, 46, 0.95); }\n\
         .sidebar-emphasis-vivid .worklane-tint-active.worklane-card-yellow { background: rgba(236, 201, 75, 0.20); border-color: rgba(236, 201, 75, 0.62); border-left: 4px solid rgba(236, 201, 75, 0.95); }\n\
         .sidebar-emphasis-vivid .worklane-tint-active.worklane-card-lime { background: rgba(154, 230, 180, 0.20); border-color: rgba(154, 230, 180, 0.62); border-left: 4px solid rgba(154, 230, 180, 0.95); }\n\
         .sidebar-emphasis-vivid .worklane-tint-active.worklane-card-green { background: rgba(72, 187, 120, 0.20); border-color: rgba(72, 187, 120, 0.62); border-left: 4px solid rgba(72, 187, 120, 0.95); }\n\
         .sidebar-emphasis-vivid .worklane-tint-active.worklane-card-teal { background: rgba(56, 178, 172, 0.20); border-color: rgba(56, 178, 172, 0.62); border-left: 4px solid rgba(56, 178, 172, 0.95); }\n\
         .sidebar-emphasis-vivid .worklane-tint-active.worklane-card-cyan { background: rgba(79, 209, 197, 0.20); border-color: rgba(79, 209, 197, 0.62); border-left: 4px solid rgba(79, 209, 197, 0.95); }\n\
         .sidebar-emphasis-vivid .worklane-tint-active.worklane-card-blue { background: rgba(66, 153, 225, 0.20); border-color: rgba(66, 153, 225, 0.62); border-left: 4px solid rgba(66, 153, 225, 0.95); }\n\
         .sidebar-emphasis-vivid .worklane-tint-active.worklane-card-indigo { background: rgba(102, 126, 234, 0.20); border-color: rgba(102, 126, 234, 0.62); border-left: 4px solid rgba(102, 126, 234, 0.95); }\n\
         .sidebar-emphasis-vivid .worklane-tint-active.worklane-card-purple { background: rgba(159, 122, 234, 0.20); border-color: rgba(159, 122, 234, 0.62); border-left: 4px solid rgba(159, 122, 234, 0.95); }\n\
         .sidebar-emphasis-vivid .worklane-tint-active.worklane-card-pink { background: rgba(237, 100, 166, 0.20); border-color: rgba(237, 100, 166, 0.62); border-left: 4px solid rgba(237, 100, 166, 0.95); }\n\
         .worklane-title { color: #b8bec8; font-weight: 700; }\n\
         .worklane-card-active .worklane-title { color: #ffffff; }\n\
         .worklane-context { color: #a7adb8; font-size: 12px; }\n\
         .project-context-row { color: #c9ced7; padding: 1px 7px 3px 7px; }\n\
         .project-context-branch { color: #8fc7ff; font-size: 11px; font-weight: 600; }\n\
         .project-context-dirty { color: #f6c453; font-size: 11px; }\n\
         .review-chip { border-radius: 8px; padding: 1px 5px; font-size: 10px; font-weight: 700; }\n\
         .review-chip-neutral { color: #c4c9d1; background: rgba(130, 138, 151, 0.20); }\n\
         .review-chip-success { color: #8ee6a3; background: rgba(46, 160, 67, 0.24); }\n\
         .review-chip-warning { color: #f6c453; background: rgba(210, 153, 34, 0.22); }\n\
         .review-chip-danger { color: #ff9b9b; background: rgba(218, 54, 51, 0.24); }\n\
         .review-chip-info { color: #a8c7fa; background: rgba(77, 128, 191, 0.24); }\n\
         .review-context-stale { opacity: 0.62; }\n\
         .pane-row { color: #e7e9ed; border-radius: 6px; padding: 5px 7px; }\n\
         .pane-row-focused { background: #343943; }\n\
         .pane-row-agent-attention { background: rgba(214, 158, 46, 0.16); }\n\
         .pane-marker { color: #727a86; }\n\
         .worklane-card-active .pane-marker { color: #69db7c; }\n\
         .pane-agent-status { color: #a7adb8; font-size: 11px; }\n\
         .pane-agent-status-attention { color: #f6c453; font-weight: 700; }\n\
         .server-row { color: #9bd1ff; border-radius: 6px; padding: 3px 7px; }\n\
         .server-row-primary { background: rgba(64, 130, 190, 0.18); font-weight: 600; }\n\
         .sidebar-create-worklane { color: #d8dbe1; border-radius: 7px; padding: 5px 8px; }\n\
         #zentty-bookmarks-button { color: #d8dbe1; min-width: 32px; min-height: 32px; padding: 0; border-radius: 7px; }\n\
         .bookmark-row { background: #242830; border: 1px solid #373d47; border-radius: 7px; padding: 2px; }\n\
         .bookmark-row:hover { border-color: #596474; background: #2b3039; }\n\
         popover.bookmark-popover > contents { background: #1d2025; color: #edf0f4; border: 1px solid #48505c; border-radius: 10px; }\n\
         popover.bookmark-popover entry { background: #292d34; color: #f1f3f5; border-color: #49515d; }\n\
         popover.bookmark-popover button { background: #2b3038; color: #edf0f4; border-color: #424a56; }\n\
         popover.bookmark-popover button:hover { background: #363d47; border-color: #647083; }\n\
         .restore-notice { background: #392f1d; color: #ffe7ae; border: 1px solid #9a7431; border-radius: 9px; padding: 8px 10px; box-shadow: 0 6px 18px rgba(0,0,0,0.45); }\n\
         .sidebar-global-search { background: #242830; border: 1px solid #3a414d; border-radius: 10px; padding: 4px 6px; }\n\
         .sidebar-global-search entry { background: transparent; color: #eef1f5; border: none; box-shadow: none; }\n\
         .sidebar-global-search-count { color: #a7adb8; font-family: monospace; }\n\
         .sidebar-global-search-button { min-width: 24px; min-height: 24px; padding: 0; }\n\
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
         .zentty-chrome-icon { color: #d5d9df; min-width: 28px; min-height: 28px; padding: 0; border-radius: 7px; }\n\
         .open-with-control { background: #252a31; border: 1px solid #4a515d; border-radius: 15px; }\n\
         .open-with-primary { color: #dfe4eb; min-width: 38px; min-height: 28px; padding: 0; border: 0; border-radius: 14px 0 0 14px; box-shadow: none; }\n\
         .open-with-menu { color: #bdc4cf; min-width: 24px; min-height: 28px; padding: 0; border: 0; border-left: 1px solid #4a515d; border-radius: 0 14px 14px 0; box-shadow: none; }",
    );
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn render(
    sidebar: &gtk::Box,
    window: &gtk::Window,
    summaries: &[SidebarWorklaneSummary],
    clipboard: ClipboardConfig,
    servers: &[RankedServer],
    templates: &[zentty_core::WorkspaceTemplate],
    active_origin_id: Option<&str>,
    current_window_id: &str,
    destination_groups: Option<&[WorklaneDestinationGroup]>,
    selection_emphasis: zentty_core::SidebarSelectionEmphasis,
) {
    sidebar.add_css_class("zentty-sidebar");
    sidebar.remove_css_class("sidebar-emphasis-vivid");
    if selection_emphasis == zentty_core::SidebarSelectionEmphasis::Vivid {
        sidebar.add_css_class("sidebar-emphasis-vivid");
    }
    eprintln!(
        "zentty-linux: sidebar-selection-emphasis value={}",
        selection_emphasis.config_value()
    );
    let header = ensure_header(sidebar);
    bookmarks_view::configure_header(&header, window, templates, active_origin_id);
    let expected_ids = summaries
        .iter()
        .map(|summary| summary.worklane_id.as_str())
        .collect::<Vec<_>>();
    let mut child = header.next_sibling();
    while let Some(widget) = child {
        child = widget.next_sibling();
        let keep = widget.widget_name() == global_search_view::ROW_NAME
            || expected_ids
                .iter()
                .any(|id| widget.widget_name() == widget_name("worklane-card", id));
        if !keep {
            sidebar.remove(&widget);
        }
    }
    let mut previous: gtk::Widget =
        find_named_widget(sidebar.upcast_ref(), global_search_view::ROW_NAME)
            .unwrap_or_else(|| header.upcast());
    for (index, summary) in summaries.iter().enumerate() {
        let name = widget_name("worklane-card", &summary.worklane_id);
        let card = find_named_widget(sidebar.upcast_ref(), &name)
            .filter(|card| card_is_compatible(card, summary, servers))
            .unwrap_or_else(|| {
                if let Some(stale) = find_named_widget(sidebar.upcast_ref(), &name) {
                    sidebar.remove(&stale);
                }
                let card = make_worklane_card(
                    window,
                    summary,
                    summaries,
                    index,
                    clipboard,
                    servers,
                    current_window_id,
                    destination_groups,
                );
                sidebar.append(&card);
                card.upcast()
            });
        refresh_pane_menus(
            &card,
            window,
            summary,
            summaries,
            clipboard,
            current_window_id,
            destination_groups,
        );
        sidebar.reorder_child_after(&card, Some(&previous));
        previous = card;
    }
    let _ = update_metadata(sidebar, summaries);
}

fn ensure_header(sidebar: &gtk::Box) -> gtk::Box {
    if let Some(header) = find_named_widget(sidebar.upcast_ref(), "zentty-sidebar-header")
        .and_then(|widget| widget.downcast::<gtk::Box>().ok())
    {
        return header;
    }
    let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    header.set_widget_name("zentty-sidebar-header");
    let add = gtk::Button::new();
    add.add_css_class("sidebar-create-worklane");
    add.set_hexpand(true);
    let add_content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    add_content.append(&gtk::Image::from_icon_name("list-add-symbolic"));
    let add_label = gtk::Label::new(Some(source_ui::NEW_WORKLANE));
    add_label.set_xalign(0.0);
    add_label.set_hexpand(true);
    add_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    add_content.append(&add_label);
    add.set_child(Some(&add_content));
    add.set_tooltip_text(Some(source_ui::NEW_WORKLANE));
    add.set_accessible_role(gtk::AccessibleRole::Button);
    add.update_property(&[gtk::accessible::Property::Label(source_ui::NEW_WORKLANE)]);
    add.set_action_name(Some("workspace.new-worklane"));
    header.append(&add);
    sidebar.prepend(&header);
    header
}

pub(crate) fn clear(sidebar: &gtk::Box) {
    remove_all_children(sidebar);
}

#[allow(clippy::too_many_arguments)]
fn make_worklane_card(
    window: &gtk::Window,
    summary: &SidebarWorklaneSummary,
    summaries: &[SidebarWorklaneSummary],
    index: usize,
    clipboard: ClipboardConfig,
    servers: &[RankedServer],
    current_window_id: &str,
    destination_groups: Option<&[WorklaneDestinationGroup]>,
) -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 4);
    card.set_widget_name(&widget_name("worklane-card", &summary.worklane_id));
    card.add_css_class("worklane-card");
    if let Some(color) = summary.color {
        card.add_css_class(&format!("worklane-card-{}", color.as_str()));
    }
    apply_worklane_visual_state(card.upcast_ref(), summary);
    let custom_title = gtk::Label::new(summary.top_label.as_deref());
    custom_title.set_widget_name(&widget_name("worklane-custom-title", &summary.worklane_id));
    custom_title.set_visible(false);
    card.append(&custom_title);
    let server_fingerprint =
        gtk::Label::new(Some(&server_fingerprint(&summary.worklane_id, servers)));
    server_fingerprint.set_widget_name(&widget_name(
        "worklane-server-fingerprint",
        &summary.worklane_id,
    ));
    server_fingerprint.set_visible(false);
    card.append(&server_fingerprint);
    let project_fingerprint = gtk::Label::new(Some(&project_fingerprint(summary)));
    project_fingerprint.set_widget_name(&widget_name(
        "worklane-project-fingerprint",
        &summary.worklane_id,
    ));
    project_fingerprint.set_visible(false);
    card.append(&project_fingerprint);

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
        summaries.len(),
    )));
    header.append(&menu);
    install_worklane_drag_source(&header, &card, summary, index);
    install_worklane_drop_target(&card, &summary.worklane_id);
    card.append(&header);

    if let Some(project_row) = make_project_context_row(summary) {
        card.append(&project_row);
    }

    for pane in &summary.pane_rows {
        card.append(&make_pane_row(
            window,
            summary,
            summaries,
            pane,
            clipboard,
            current_window_id,
            destination_groups,
        ));
    }
    for server in servers.iter().filter(|server| {
        server.server.worklane_id == summary.worklane_id
            && server.tier != ServerRelevanceTier::Hidden
    }) {
        card.append(&make_server_row(server));
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

fn card_is_compatible(
    card: &gtk::Widget,
    summary: &SidebarWorklaneSummary,
    servers: &[RankedServer],
) -> bool {
    let custom_title = find_named_label(
        card,
        &widget_name("worklane-custom-title", &summary.worklane_id),
    )
    .map(|label| label.text().to_string());
    if custom_title.as_deref() != Some(summary.top_label.as_deref().unwrap_or("")) {
        return false;
    }
    let current_color = zentty_core::WorklaneColor::ALL
        .into_iter()
        .find(|color| card.has_css_class(&format!("worklane-card-{}", color.as_str())));
    if current_color != summary.color {
        return false;
    }
    let expected_servers = server_fingerprint(&summary.worklane_id, servers);
    let current_servers = find_named_label(
        card,
        &widget_name("worklane-server-fingerprint", &summary.worklane_id),
    )
    .map(|label| label.text().to_string());
    if current_servers.as_deref() != Some(expected_servers.as_str()) {
        return false;
    }
    let mut pane_ids = Vec::new();
    collect_named_ids(card, "zentty-pane-row-", &mut pane_ids);
    pane_ids
        == summary
            .pane_rows
            .iter()
            .map(|pane| pane.pane_id.clone())
            .collect::<Vec<_>>()
}

fn focused_project_context(
    summary: &SidebarWorklaneSummary,
) -> Option<&zentty_core::ProjectContext> {
    summary
        .pane_rows
        .iter()
        .find(|pane| pane.is_focused)
        .and_then(|pane| pane.project_context.as_ref())
}

fn project_fingerprint(summary: &SidebarWorklaneSummary) -> String {
    focused_project_context(summary).map_or_else(String::new, |context| {
        format!(
            "{}|{}|{}|{}|{}",
            context.repository_root.display(),
            context.reference.display(),
            context.dirty,
            context
                .review
                .as_ref()
                .map_or(0, |review| review.pull_request.number),
            context.review_error.as_deref().unwrap_or_default()
        )
    })
}

fn make_project_context_row(summary: &SidebarWorklaneSummary) -> Option<gtk::Box> {
    let context = focused_project_context(summary)?;
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 5);
    row.set_widget_name(&widget_name("project-context", &summary.worklane_id));
    row.add_css_class("project-context-row");
    if context.review_error.is_some() {
        row.add_css_class("review-context-stale");
    }
    row.append(&gtk::Image::from_icon_name("vcs-branch-symbolic"));
    let reference = gtk::Label::new(Some(&context.reference.display()));
    reference.add_css_class("project-context-branch");
    reference.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    reference.set_hexpand(true);
    reference.set_xalign(0.0);
    row.append(&reference);
    if context.dirty {
        let dirty = gtk::Label::new(Some("● dirty"));
        dirty.add_css_class("project-context-dirty");
        row.append(&dirty);
    }
    if let Some(review) = &context.review {
        let pull_request = gtk::Label::new(Some(&format!("PR #{}", review.pull_request.number)));
        pull_request.add_css_class("review-chip");
        pull_request.add_css_class(match review.pull_request.state {
            zentty_core::PullRequestState::Open => "review-chip-success",
            zentty_core::PullRequestState::Draft => "review-chip-neutral",
            zentty_core::PullRequestState::Merged => "review-chip-info",
            zentty_core::PullRequestState::Closed => "review-chip-danger",
        });
        row.append(&pull_request);
        for chip in &review.chips {
            let label = gtk::Label::new(Some(&chip.text));
            label.add_css_class("review-chip");
            label.add_css_class(match chip.style {
                zentty_core::ReviewChipStyle::Neutral => "review-chip-neutral",
                zentty_core::ReviewChipStyle::Success => "review-chip-success",
                zentty_core::ReviewChipStyle::Warning => "review-chip-warning",
                zentty_core::ReviewChipStyle::Danger => "review-chip-danger",
                zentty_core::ReviewChipStyle::Info => "review-chip-info",
            });
            row.append(&label);
        }
    }
    if context.review_error.is_some() {
        let stale = gtk::Label::new(Some("stale"));
        stale.add_css_class("review-chip");
        stale.add_css_class("review-chip-warning");
        row.append(&stale);
    }
    let review_age = context.review.as_ref().map(|review| {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs());
        format!("\nReview status updated {}", review.age_label(now))
    });
    let tooltip = if let Some(error) = &context.review_error {
        format!(
            "{}{}\nLast review refresh failed: {error}",
            context.repository_root.display(),
            review_age.as_deref().unwrap_or_default()
        )
    } else {
        format!(
            "{}{}",
            context.repository_root.display(),
            review_age.as_deref().unwrap_or_default()
        )
    };
    row.set_tooltip_text(Some(&tooltip));
    row.update_property(&[gtk::accessible::Property::Label(&format!(
        "Git {}, {}{}",
        context.reference.display(),
        if context.dirty {
            "dirty working tree"
        } else {
            "clean working tree"
        },
        context
            .review
            .as_ref()
            .map_or_else(String::new, |review| format!(
                ", pull request {}",
                review.pull_request.number
            ))
    ))]);
    Some(row)
}

fn server_fingerprint(worklane_id: &str, servers: &[RankedServer]) -> String {
    servers
        .iter()
        .filter(|server| server.server.worklane_id == worklane_id)
        .map(|server| format!("{}:{:?}", server.server.id, server.tier))
        .collect::<Vec<_>>()
        .join("|")
}

fn make_server_row(server: &RankedServer) -> gtk::Button {
    let button = gtk::Button::new();
    button.set_widget_name(&widget_name(
        "server-row",
        &format!(
            "{}-{}",
            server.server.worklane_id,
            server.server.ports.first().copied().unwrap_or_default()
        ),
    ));
    button.set_has_frame(false);
    button.add_css_class("server-row");
    if server.tier == ServerRelevanceTier::Primary {
        button.add_css_class("server-row-primary");
    }
    button.set_action_name(Some("workspace.open-server"));
    button.set_action_target_value(Some(&server.server.origin.to_variant()));
    button.set_tooltip_text(Some(&format!("Open {}", server.server.url)));
    button.set_accessible_role(gtk::AccessibleRole::Button);
    button.update_property(&[gtk::accessible::Property::Label(&format!(
        "Open development server {}",
        server.server.display
    ))]);
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    content.append(&gtk::Image::from_icon_name("network-server-symbolic"));
    let label = gtk::Label::new(Some(&server.server.display));
    label.set_xalign(0.0);
    label.set_hexpand(true);
    content.append(&label);
    button.set_child(Some(&content));
    button
}

fn collect_named_ids(widget: &gtk::Widget, prefix: &str, ids: &mut Vec<String>) {
    let name = widget.widget_name();
    if let Some(id) = name.strip_prefix(prefix) {
        ids.push(id.to_owned());
    }
    let mut child = widget.first_child();
    while let Some(widget) = child {
        collect_named_ids(&widget, prefix, ids);
        child = widget.next_sibling();
    }
}

fn install_worklane_drag_source(
    header: &gtk::Box,
    card: &gtk::Box,
    summary: &SidebarWorklaneSummary,
    index: usize,
) {
    let motion = gtk::EventControllerMotion::new();
    let hover_id = summary.worklane_id.clone();
    motion.connect_enter(move |_, _, _| {
        eprintln!("zentty-linux: worklane-drag=pointer-target id={hover_id}");
    });
    header.add_controller(motion);

    let source = gtk::DragSource::new();
    source.set_actions(gtk::gdk::DragAction::MOVE);
    source.set_propagation_phase(gtk::PropagationPhase::Capture);
    header.set_cursor_from_name(Some("grab"));
    let worklane_id = summary.worklane_id.clone();
    let prepare_id = worklane_id.clone();
    source.connect_prepare(move |_, _, _| {
        Some(gtk::gdk::ContentProvider::for_value(&prepare_id.to_value()))
    });
    let begin_card = card.clone();
    let begin_header = header.clone();
    let begin_id = worklane_id.clone();
    let mut begin_summary = summary.clone();
    begin_summary.is_active = true;
    source.connect_drag_begin(move |source, _| {
        let selected = begin_card
            .activate_action("workspace.select-worklane", Some(&begin_id.to_variant()))
            .is_ok();
        let preview = make_worklane_drag_preview_card(&begin_summary, index);
        begin_worklane_drag_preview(&begin_card, &preview);
        let paintable = gtk::WidgetPaintable::new(Some(&preview));
        source.set_icon(Some(&paintable), 24, 18);
        begin_card.add_css_class("worklane-dragged");
        begin_header.set_cursor_from_name(Some("grabbing"));
        eprintln!(
            "zentty-linux: worklane-drag=begin id={begin_id} selected={selected} visual=floating ghost=card slot=rendered"
        );
    });
    let end_card = card.clone();
    let end_header = header.clone();
    let end_id = worklane_id.clone();
    source.connect_drag_end(move |_, _, _| {
        clear_worklane_drag_feedback(&end_card);
        end_header.set_cursor_from_name(Some("grab"));
        eprintln!("zentty-linux: worklane-drag=end id={end_id} visual=cleared");
    });
    let cancel_id = worklane_id;
    source.connect_drag_cancel(move |_, _, reason| {
        eprintln!("zentty-linux: worklane-drag=cancel id={cancel_id} reason={reason:?}");
        false
    });
    header.add_controller(source);
}

fn install_worklane_drop_target(card: &gtk::Box, target_id: &str) {
    let target = gtk::DropTarget::new(String::static_type(), gtk::gdk::DragAction::MOVE);
    let enter_id = target_id.to_owned();
    target.connect_enter(move |_, _, _| {
        eprintln!("zentty-linux: worklane-drag=target id={enter_id}");
        gtk::gdk::DragAction::MOVE
    });
    let last_edge = Rc::new(Cell::new(None));
    let motion_edge = Rc::clone(&last_edge);
    let card_motion = card.clone();
    let motion_id = target_id.to_owned();
    target.connect_motion(move |_, _, y| {
        let edge = WorklaneDropEdge::at(y, card_motion.height());
        if motion_edge.get() != Some(edge) {
            move_worklane_drag_preview(&card_motion, edge);
            motion_edge.set(Some(edge));
            eprintln!(
                "zentty-linux: worklane-drag=preview-slot target={motion_id} edge={} reflow=live",
                edge.as_str()
            );
        }
        gtk::gdk::DragAction::MOVE
    });
    let leave_edge = Rc::clone(&last_edge);
    target.connect_leave(move |_| leave_edge.set(None));
    let card_drop = card.clone();
    let target_id = target_id.to_owned();
    target.connect_drop(move |_, value, _, y| {
        let Ok(dragged_id) = value.get::<String>() else {
            return false;
        };
        if dragged_id == target_id {
            return false;
        }
        let edge = WorklaneDropEdge::at(y, card_drop.height());
        let placement = format!("{}:{target_id}", edge.as_str());
        card_drop
            .activate_action(
                "workspace.reorder-worklane",
                Some(&(dragged_id.as_str(), placement.as_str()).to_variant()),
            )
            .is_ok()
    });
    card.add_controller(target);
}

const WORKLANE_REORDER_PREVIEW_NAME: &str = "zentty-worklane-reorder-preview";

pub(crate) fn has_worklane_reorder_preview(sidebar: &gtk::Box) -> bool {
    find_named_widget(sidebar.upcast_ref(), WORKLANE_REORDER_PREVIEW_NAME).is_some()
}

fn sidebar_for_card(card: &gtk::Box) -> Option<gtk::Box> {
    card.parent()
        .and_then(|parent| parent.downcast::<gtk::Box>().ok())
}

fn begin_worklane_drag_preview(card: &gtk::Box, preview: &gtk::Box) {
    let Some(sidebar) = sidebar_for_card(card) else {
        return;
    };
    remove_worklane_drag_preview(&sidebar);
    preview.set_height_request(card.height().max(1));
    sidebar.append(preview);
    sidebar.reorder_child_after(preview, card.prev_sibling().as_ref());
    card.set_visible(false);
}

fn move_worklane_drag_preview(target_card: &gtk::Box, edge: WorklaneDropEdge) {
    let Some(sidebar) = sidebar_for_card(target_card) else {
        return;
    };
    let Some(preview) = find_named_widget(sidebar.upcast_ref(), WORKLANE_REORDER_PREVIEW_NAME)
    else {
        return;
    };
    let previous = match edge {
        WorklaneDropEdge::After => Some(target_card.clone().upcast::<gtk::Widget>()),
        WorklaneDropEdge::Before => {
            let mut previous = target_card.prev_sibling();
            while previous.as_ref().is_some_and(|widget| {
                widget.widget_name() == WORKLANE_REORDER_PREVIEW_NAME
                    || widget.has_css_class("worklane-dragged")
            }) {
                previous = previous.and_then(|widget| widget.prev_sibling());
            }
            previous
        }
    };
    sidebar.reorder_child_after(&preview, previous.as_ref());
}

fn remove_worklane_drag_preview(sidebar: &gtk::Box) {
    if let Some(preview) = find_named_widget(sidebar.upcast_ref(), WORKLANE_REORDER_PREVIEW_NAME) {
        sidebar.remove(&preview);
    }
}

fn clear_worklane_drag_feedback(card: &gtk::Box) {
    card.set_visible(true);
    card.remove_css_class("worklane-dragged");
    if let Some(sidebar) = sidebar_for_card(card) {
        remove_worklane_drag_preview(&sidebar);
    }
}

fn adjacent_worklane_id(mut widget: Option<gtk::Widget>, forward: bool) -> Option<String> {
    while let Some(current) = widget {
        if !current.has_css_class("worklane-dragged")
            && let Some(id) = current.widget_name().strip_prefix("zentty-worklane-card-")
        {
            return Some(id.to_owned());
        }
        widget = if forward {
            current.next_sibling()
        } else {
            current.prev_sibling()
        };
    }
    None
}

fn install_worklane_preview_drop_target(preview: &gtk::Box) {
    let target = gtk::DropTarget::new(String::static_type(), gtk::gdk::DragAction::MOVE);
    target.connect_enter(|_, _, _| gtk::gdk::DragAction::MOVE);
    let drop_preview = preview.clone();
    target.connect_drop(move |_, value, _, _| {
        let Ok(dragged_id) = value.get::<String>() else {
            return false;
        };
        let placement = adjacent_worklane_id(drop_preview.prev_sibling(), false)
            .map(|id| format!("after:{id}"))
            .or_else(|| {
                adjacent_worklane_id(drop_preview.next_sibling(), true)
                    .map(|id| format!("before:{id}"))
            });
        let Some(placement) = placement else {
            return false;
        };
        let accepted = drop_preview
            .activate_action(
                "workspace.reorder-worklane",
                Some(&(dragged_id.as_str(), placement.as_str()).to_variant()),
            )
            .is_ok();
        eprintln!(
            "zentty-linux: worklane-drag=preview-drop id={dragged_id} placement={placement} accepted={accepted}"
        );
        accepted
    });
    preview.add_controller(target);
}

fn make_worklane_drag_preview_card(summary: &SidebarWorklaneSummary, index: usize) -> gtk::Box {
    let preview = gtk::Box::new(gtk::Orientation::Vertical, 4);
    preview.set_widget_name(WORKLANE_REORDER_PREVIEW_NAME);
    preview.add_css_class("worklane-card");
    preview.add_css_class("worklane-drag-preview");
    install_worklane_preview_drop_target(&preview);
    if let Some(color) = summary.color {
        preview.add_css_class(&format!("worklane-card-{}", color.as_str()));
    }
    apply_worklane_visual_state(preview.upcast_ref(), summary);

    let header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let heading = gtk::Box::new(gtk::Orientation::Vertical, 1);
    heading.set_hexpand(true);
    let title_text = summary
        .top_label
        .clone()
        .unwrap_or_else(|| format!("Worklane {}", index + 1));
    let title = gtk::Label::new(Some(&title_text));
    title.add_css_class("worklane-title");
    title.set_xalign(0.0);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    let context = gtk::Label::new(Some(&summary.primary_text));
    context.add_css_class("worklane-context");
    context.set_xalign(0.0);
    context.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    heading.append(&title);
    heading.append(&context);
    header.append(&heading);
    let menu_icon = gtk::Image::from_icon_name("view-more-symbolic");
    menu_icon.set_margin_start(10);
    menu_icon.set_margin_end(10);
    header.append(&menu_icon);
    preview.append(&header);

    for pane in &summary.pane_rows {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        row.add_css_class("pane-row");
        if pane.is_focused {
            row.add_css_class("pane-row-focused");
        }
        let marker = gtk::Label::new(Some(if pane.is_focused { "●" } else { "○" }));
        marker.add_css_class("pane-marker");
        let pane_title = gtk::Label::new(Some(&pane.primary_text));
        pane_title.set_xalign(0.0);
        pane_title.set_hexpand(true);
        pane_title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        row.append(&marker);
        row.append(&pane_title);
        let pane_menu_icon = gtk::Image::from_icon_name("view-more-symbolic");
        pane_menu_icon.set_margin_start(8);
        pane_menu_icon.set_margin_end(8);
        row.append(&pane_menu_icon);
        preview.append(&row);
    }
    preview
}

fn make_pane_row(
    window: &gtk::Window,
    summary: &SidebarWorklaneSummary,
    summaries: &[SidebarWorklaneSummary],
    pane: &zentty_core::SidebarPaneSummary,
    clipboard: ClipboardConfig,
    current_window_id: &str,
    destination_groups: Option<&[WorklaneDestinationGroup]>,
) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 2);
    row.set_widget_name(&widget_name("pane-row", &pane.pane_id));
    row.add_css_class("pane-row");
    if pane.is_focused {
        row.add_css_class("pane-row-focused");
    }
    if pane
        .agent_status
        .as_ref()
        .is_some_and(zentty_core::PaneAgentStatus::requires_attention)
    {
        row.add_css_class("pane-row-agent-attention");
    }
    let select = gtk::Button::new();
    select.set_has_frame(false);
    select.set_hexpand(true);
    select.set_action_name(Some("workspace.select-pane"));
    select.set_action_target_value(Some(
        &(summary.worklane_id.as_str(), pane.pane_id.as_str()).to_variant(),
    ));
    select.set_accessible_role(gtk::AccessibleRole::Button);
    select.update_property(&[gtk::accessible::Property::Label(&pane_accessible_label(
        pane,
    ))]);
    select.update_state(&[gtk::accessible::State::Selected(Some(pane.is_focused))]);
    let pane_content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let marker = gtk::Label::new(Some(if pane.is_focused { "●" } else { "○" }));
    marker.set_widget_name(&widget_name("pane-marker", &pane.pane_id));
    marker.add_css_class("pane-marker");
    let labels = gtk::Box::new(gtk::Orientation::Vertical, 0);
    labels.set_hexpand(true);
    let pane_title = gtk::Label::new(Some(&pane.primary_text));
    pane_title.set_widget_name(&widget_name("pane-title", &pane.pane_id));
    pane_title.set_xalign(0.0);
    pane_title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    let agent_status = gtk::Label::new(None);
    agent_status.set_widget_name(&widget_name("pane-agent-status", &pane.pane_id));
    agent_status.add_css_class("pane-agent-status");
    agent_status.set_xalign(0.0);
    agent_status.set_ellipsize(gtk::pango::EllipsizeMode::End);
    update_agent_status_label(&agent_status, pane.agent_status.as_ref());
    labels.append(&pane_title);
    labels.append(&agent_status);
    pane_content.append(&marker);
    pane_content.append(&labels);
    select.set_child(Some(&pane_content));
    row.append(&select);

    let pane_menu = gtk::MenuButton::new();
    pane_menu.set_widget_name(&widget_name("pane-menu", &pane.pane_id));
    pane_menu.add_css_class("sidebar-pane-actions");
    pane_menu.set_icon_name("view-more-symbolic");
    pane_menu.set_tooltip_text(Some("Pane actions"));
    pane_menu.set_accessible_role(gtk::AccessibleRole::Button);
    pane_menu.update_property(&[gtk::accessible::Property::Label("Pane actions")]);
    let focus_pane_id = pane.pane_id.clone();
    pane_menu.connect_has_focus_notify(move |button| {
        if button.has_focus() {
            eprintln!("zentty-linux: pane-context-focus action=open pane={focus_pane_id}");
        }
    });
    let pointer_receipt = gtk::EventControllerMotion::new();
    let pointer_pane_id = pane.pane_id.clone();
    pointer_receipt.connect_enter(move |_, _, _| {
        eprintln!("zentty-linux: pane-context-pointer action=open pane={pointer_pane_id}");
    });
    pane_menu.add_controller(pointer_receipt);
    pane_menu.set_popover(Some(&make_pane_context_menu(
        window,
        summary,
        summaries,
        pane,
        summary.pane_rows.len() > 1,
        clipboard,
        current_window_id,
        destination_groups,
    )));
    row.append(&pane_menu);
    row
}

fn refresh_pane_menus(
    card: &gtk::Widget,
    window: &gtk::Window,
    summary: &SidebarWorklaneSummary,
    summaries: &[SidebarWorklaneSummary],
    clipboard: ClipboardConfig,
    current_window_id: &str,
    destination_groups: Option<&[WorklaneDestinationGroup]>,
) {
    for pane in &summary.pane_rows {
        let Some(menu) = find_named_widget(card, &widget_name("pane-menu", &pane.pane_id))
            .and_then(|widget| widget.downcast::<gtk::MenuButton>().ok())
        else {
            continue;
        };
        // Preserve the user's open contextual transaction across unrelated
        // metadata refreshes (server discovery, agent status, project state).
        // Replacing a visible popover destroys its focus chain and can route
        // the next real key event to the window beneath it. Its destinations
        // are a coherent snapshot; the action router still validates the
        // selected target when activation commits.
        if menu.popover().is_some_and(|popover| popover.is_visible()) {
            continue;
        }
        menu.set_popover(Some(&make_pane_context_menu(
            window,
            summary,
            summaries,
            pane,
            summary.pane_rows.len() > 1,
            clipboard,
            current_window_id,
            destination_groups,
        )));
    }
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
        let accessible_label = format!("{title_text}, {}", summary.primary_text);
        select.update_property(&[gtk::accessible::Property::Label(&accessible_label)]);
        let Some(move_up) = find_named_widget(
            sidebar.upcast_ref(),
            &widget_name("worklane-move-up", &summary.worklane_id),
        ) else {
            return false;
        };
        move_up.set_sensitive(index > 0);
        let Some(move_down) = find_named_widget(
            sidebar.upcast_ref(),
            &widget_name("worklane-move-down", &summary.worklane_id),
        ) else {
            return false;
        };
        move_down.set_sensitive(index + 1 < summaries.len());

        if !update_project_context_row(&card, summary) {
            return false;
        }

        for pane in &summary.pane_rows {
            if !update_pane_metadata(sidebar, pane) {
                return false;
            }
        }
    }
    true
}

pub(crate) fn update_project_context_metadata(
    sidebar: &gtk::Box,
    summaries: &[SidebarWorklaneSummary],
) {
    for summary in summaries {
        let Some(card) = find_named_widget(
            sidebar.upcast_ref(),
            &widget_name("worklane-card", &summary.worklane_id),
        ) else {
            continue;
        };
        let _ = update_project_context_row(&card, summary);
    }
}

fn update_project_context_row(card: &gtk::Widget, summary: &SidebarWorklaneSummary) -> bool {
    let Some(card_box) = card.downcast_ref::<gtk::Box>() else {
        return false;
    };
    let fingerprint_name = widget_name("worklane-project-fingerprint", &summary.worklane_id);
    let Some(fingerprint) = find_named_label(card, &fingerprint_name) else {
        return false;
    };
    let expected = project_fingerprint(summary);
    if fingerprint.text() == expected {
        return true;
    }
    fingerprint.set_text(&expected);
    if let Some(current) =
        find_named_widget(card, &widget_name("project-context", &summary.worklane_id))
    {
        card_box.remove(&current);
    }
    if let Some(project_row) = make_project_context_row(summary) {
        let Some(header) =
            find_named_widget(card, &widget_name("worklane-select", &summary.worklane_id))
                .and_then(|select| select.parent())
        else {
            return false;
        };
        card_box.insert_child_after(&project_row, Some(&header));
    }
    true
}

fn update_pane_metadata(sidebar: &gtk::Box, pane: &zentty_core::SidebarPaneSummary) -> bool {
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
    if pane
        .agent_status
        .as_ref()
        .is_some_and(zentty_core::PaneAgentStatus::requires_attention)
    {
        row.add_css_class("pane-row-agent-attention");
    } else {
        row.remove_css_class("pane-row-agent-attention");
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
    let Some(agent_status) = find_named_label(
        sidebar.upcast_ref(),
        &widget_name("pane-agent-status", &pane.pane_id),
    ) else {
        return false;
    };
    update_agent_status_label(&agent_status, pane.agent_status.as_ref());
    if let Some(status) = &pane.agent_status {
        eprintln!(
            "zentty-linux: sidebar-agent-status pane={} phase={:?} interaction={:?} attention={} progress={}",
            pane.pane_id,
            status.phase,
            status.interaction,
            status.requires_attention(),
            status.progress.map_or_else(
                || "none".to_owned(),
                |progress| format!("{}/{}", progress.done, progress.total)
            )
        );
    }
    eprintln!(
        "zentty-linux: pane-display id={} label-present={} custom={}",
        pane.pane_id,
        !pane.primary_text.is_empty(),
        pane.custom_title.is_some()
    );
    let Some(select) = row
        .first_child()
        .and_then(|child| child.downcast::<gtk::Button>().ok())
    else {
        return false;
    };
    select.update_property(&[gtk::accessible::Property::Label(&pane_accessible_label(
        pane,
    ))]);
    select.update_state(&[gtk::accessible::State::Selected(Some(pane.is_focused))]);
    true
}

fn update_agent_status_label(label: &gtk::Label, status: Option<&zentty_core::PaneAgentStatus>) {
    label.remove_css_class("pane-agent-status-attention");
    let Some(status) = status else {
        label.set_visible(false);
        label.set_text("");
        return;
    };
    let presentation = agent_status_view::present(status);
    label.set_text(&presentation.text);
    label.set_visible(true);
    if presentation.requires_attention {
        label.add_css_class("pane-agent-status-attention");
    }
}

fn pane_accessible_label(pane: &zentty_core::SidebarPaneSummary) -> String {
    pane.agent_status.as_ref().map_or_else(
        || pane.primary_text.clone(),
        |status| {
            format!(
                "{}, {}",
                pane.primary_text,
                agent_status_view::present(status).text
            )
        },
    )
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

#[allow(clippy::too_many_arguments)]
fn make_pane_context_menu(
    window: &gtk::Window,
    source_worklane: &SidebarWorklaneSummary,
    summaries: &[SidebarWorklaneSummary],
    pane: &zentty_core::SidebarPaneSummary,
    can_close: bool,
    clipboard: ClipboardConfig,
    current_window_id: &str,
    destination_groups: Option<&[WorklaneDestinationGroup]>,
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

    let local_groups;
    let groups = if let Some(groups) = destination_groups {
        groups
    } else {
        local_groups = local_destination_groups(current_window_id, summaries);
        &local_groups
    };
    let destinations =
        worklane_destinations(groups, current_window_id, &source_worklane.worklane_id);
    let can_create_new_worklane = source_worklane.pane_rows.len() > 1;
    for action in pane_action_specs(clipboard) {
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
        let worklane_id = source_worklane.worklane_id.clone();
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
        if action.action == "move-pane-to-new-window"
            && (!destinations.is_empty() || can_create_new_worklane)
        {
            menu.append(&make_move_to_worklane_button(
                &popover,
                window,
                &source_worklane.worklane_id,
                &pane.pane_id,
                &destinations,
                can_create_new_worklane,
                current_window_id,
            ));
        }
    }

    popover.set_child(Some(&menu));
    popover
}

fn make_move_to_worklane_button(
    parent_popover: &gtk::Popover,
    window: &gtk::Window,
    source_worklane_id: &str,
    pane_id: &str,
    destinations: &[Vec<WorklaneDestination>],
    can_create_new_worklane: bool,
    current_window_id: &str,
) -> gtk::Button {
    let button = gtk::Button::new();
    button.add_css_class("pane-context-action");
    button.set_tooltip_text(Some(source_ui::MOVE_PANE_TO_WORKLANE));
    button.update_property(&[gtk::accessible::Property::Label(
        source_ui::MOVE_PANE_TO_WORKLANE,
    )]);
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    content.append(&gtk::Image::from_icon_name("view-list-symbolic"));
    let label = gtk::Label::new(Some(source_ui::MOVE_PANE_TO_WORKLANE));
    label.set_xalign(0.0);
    label.set_hexpand(true);
    content.append(&label);
    content.append(&gtk::Image::from_icon_name("go-next-symbolic"));
    button.set_child(Some(&content));
    let pointer_receipt = gtk::EventControllerMotion::new();
    let pointer_pane_id = pane_id.to_owned();
    pointer_receipt.connect_enter(move |_, _, _| {
        eprintln!(
            "zentty-linux: pane-context-pointer action=move-pane-to-worklane pane={pointer_pane_id}"
        );
    });
    button.add_controller(pointer_receipt);
    let focus_pane_id = pane_id.to_owned();
    button.connect_has_focus_notify(move |button| {
        if button.has_focus() {
            eprintln!(
                "zentty-linux: pane-context-focus action=move-pane-to-worklane pane={focus_pane_id}"
            );
        }
    });
    let parent_popover = parent_popover.clone();
    let window = window.clone();
    let source_worklane_id = source_worklane_id.to_owned();
    let pane_id = pane_id.to_owned();
    let destinations = destinations.to_vec();
    let current_window_id = current_window_id.to_owned();
    button.connect_clicked(move |_| {
        eprintln!(
            "zentty-linux: pane-context action=move-pane-to-worklane pane={pane_id} view=destinations"
        );
        let parent_popover = parent_popover.clone();
        let window = window.clone();
        let source_worklane_id = source_worklane_id.clone();
        let pane_id = pane_id.clone();
        let destinations = destinations.clone();
        let current_window_id = current_window_id.clone();
        gtk::glib::idle_add_local_once(move || {
            parent_popover.set_child(Some(&make_move_to_worklane_content(
                &parent_popover,
                &window,
                &source_worklane_id,
                &pane_id,
                &destinations,
                can_create_new_worklane,
                &current_window_id,
            )));
        });
    });
    button
}

fn make_move_to_worklane_content(
    parent_popover: &gtk::Popover,
    window: &gtk::Window,
    source_worklane_id: &str,
    pane_id: &str,
    destinations: &[Vec<WorklaneDestination>],
    can_create_new_worklane: bool,
    current_window_id: &str,
) -> gtk::Box {
    let menu = gtk::Box::new(gtk::Orientation::Vertical, 2);
    menu.set_margin_top(6);
    menu.set_margin_bottom(6);
    menu.set_margin_start(6);
    menu.set_margin_end(6);
    for (group_index, group) in destinations.iter().enumerate() {
        if group_index > 0 {
            menu.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        }
        for destination in group {
            let button = menu_button(&destination.label, "media-record-symbolic");
            if let Some(color) = destination.color {
                button.add_css_class(&format!("worklane-color-{}", color.as_str()));
            }
            let window = window.clone();
            let source_worklane_id = source_worklane_id.to_owned();
            let pane_id = pane_id.to_owned();
            let target_window_id = destination.window_id.clone();
            let target_worklane_id = destination.worklane_id.clone();
            let pointer_receipt = gtk::EventControllerMotion::new();
            let pointer_window = target_window_id.clone();
            let pointer_target = target_worklane_id.clone();
            pointer_receipt.connect_enter(move |_, _, _| {
                eprintln!(
                    "zentty-linux: pane-context-pointer action=move-pane-to-worklane window={pointer_window} target={pointer_target}"
                );
            });
            button.add_controller(pointer_receipt);
            let focus_window = target_window_id.clone();
            let focus_target = target_worklane_id.clone();
            button.connect_has_focus_notify(move |button| {
                if button.has_focus() {
                    eprintln!(
                        "zentty-linux: pane-context-focus action=move-pane-to-worklane window={focus_window} target={focus_target}"
                    );
                }
            });
            let destination_popover = parent_popover.clone();
            let current_window_id = current_window_id.to_owned();
            button.connect_clicked(move |_| {
                eprintln!(
                    "zentty-linux: pane-context action=move-pane-to-worklane window={target_window_id} target={target_worklane_id} activated=true"
                );
                destination_popover.popdown();
                let _ = window.activate_action(
                    "workspace.select-pane",
                    Some(&(source_worklane_id.as_str(), pane_id.as_str()).to_variant()),
                );
                let window = window.clone();
                let target_window_id = target_window_id.clone();
                let target_worklane_id = target_worklane_id.clone();
                let current_window_id = current_window_id.clone();
                gtk::glib::idle_add_local_once(move || {
                    if target_window_id == current_window_id {
                        let _ = window.activate_action(
                            "workspace.move-pane-to-worklane",
                            Some(&target_worklane_id.to_variant()),
                        );
                    } else {
                        let _ = window.activate_action(
                            "workspace.move-pane-to-window-worklane",
                            Some(
                                &(target_window_id.as_str(), target_worklane_id.as_str())
                                    .to_variant(),
                            ),
                        );
                    }
                });
            });
            menu.append(&button);
        }
    }
    if can_create_new_worklane {
        let button = menu_button(source_ui::NEW_WORKLANE_IN_THIS_WINDOW, "list-add-symbolic");
        let pointer_receipt = gtk::EventControllerMotion::new();
        pointer_receipt.connect_enter(move |_, _, _| {
            eprintln!(
                "zentty-linux: pane-context-pointer action=move-pane-to-new-worklane target=new"
            );
        });
        button.add_controller(pointer_receipt);
        let window = window.clone();
        let source_worklane_id = source_worklane_id.to_owned();
        let pane_id = pane_id.to_owned();
        let destination_popover = parent_popover.clone();
        button.connect_clicked(move |_| {
            destination_popover.popdown();
            let _ = window.activate_action(
                "workspace.select-pane",
                Some(&(source_worklane_id.as_str(), pane_id.as_str()).to_variant()),
            );
            let window = window.clone();
            gtk::glib::idle_add_local_once(move || {
                let _ = window.activate_action("workspace.move-pane-to-new-worklane", None);
            });
        });
        menu.append(&button);
    }
    menu
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

    let move_up = targeted_move_button(
        source_ui::MOVE_WORKLANE_UP,
        "go-up-symbolic",
        &summary.worklane_id,
        "up",
        &popover,
    );
    move_up.set_widget_name(&widget_name("worklane-move-up", &summary.worklane_id));
    move_up.set_sensitive(index > 0);
    menu.append(&move_up);
    let move_down = targeted_move_button(
        source_ui::MOVE_WORKLANE_DOWN,
        "go-down-symbolic",
        &summary.worklane_id,
        "down",
        &popover,
    );
    move_down.set_widget_name(&widget_name("worklane-move-down", &summary.worklane_id));
    move_down.set_sensitive(index + 1 < worklane_count);
    menu.append(&move_down);

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

pub(crate) fn present_rename_dialog(
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
    use super::{
        WorklaneDestinationGroup, WorklaneDropEdge, WorklaneSelectionState,
        local_destination_groups, pane_action_specs, reveal_range, selection_state,
        worklane_destinations,
    };
    use crate::source_ui;
    use zentty_core::{ClipboardConfig, SidebarPaneSummary, SidebarWorklaneSummary, WorklaneColor};

    fn lane(id: &str, panes: &[&str], color: Option<WorklaneColor>) -> SidebarWorklaneSummary {
        SidebarWorklaneSummary {
            worklane_id: id.to_owned(),
            top_label: None,
            primary_text: panes.first().copied().unwrap_or_default().to_owned(),
            pane_rows: panes
                .iter()
                .enumerate()
                .map(|(index, title)| SidebarPaneSummary {
                    pane_id: format!("{id}-pane-{index}"),
                    primary_text: (*title).to_owned(),
                    custom_title: None,
                    is_focused: index == 0,
                    agent_status: None,
                    project_context: None,
                    project_icon_path: None,
                })
                .collect(),
            is_active: false,
            color,
        }
    }

    #[test]
    fn move_destination_catalog_matches_source_order_labels_and_exclusion() {
        let summaries = [
            lane("source", &["source pane"], None),
            lane("target-a", &["vim"], Some(WorklaneColor::Blue)),
            lane("target-b", &["server", "logs", "shell"], None),
        ];
        let groups = local_destination_groups("window-1", &summaries);
        let destinations = worklane_destinations(&groups, "window-1", "source");
        assert_eq!(destinations.len(), 1);
        assert_eq!(destinations[0].len(), 2);
        assert_eq!(destinations[0][0].window_id, "window-1");
        assert_eq!(destinations[0][0].worklane_id, "target-a");
        assert_eq!(destinations[0][0].label, "vim");
        assert_eq!(destinations[0][0].color, Some(WorklaneColor::Blue));
        assert_eq!(destinations[0][1].worklane_id, "target-b");
        assert_eq!(destinations[0][1].label, "server  +2 more");
    }

    #[test]
    fn move_destination_catalog_preserves_source_first_window_groups() {
        let groups = [
            WorklaneDestinationGroup {
                window_id: "window-1".to_owned(),
                summaries: vec![lane("source", &["source"], None)],
            },
            WorklaneDestinationGroup {
                window_id: "window-2".to_owned(),
                summaries: vec![
                    lane("foreign-a", &["server"], None),
                    lane("foreign-b", &["agent", "logs"], Some(WorklaneColor::Purple)),
                ],
            },
        ];
        let destinations = worklane_destinations(&groups, "window-1", "source");
        assert_eq!(destinations.len(), 1);
        assert_eq!(destinations[0].len(), 2);
        assert!(
            destinations[0]
                .iter()
                .all(|destination| destination.window_id == "window-2")
        );
        assert_eq!(destinations[0][0].label, "server");
        assert_eq!(destinations[0][1].label, "agent  +1 more");
    }

    #[test]
    fn pane_actions_are_contextual_and_source_named() {
        let actions = pane_action_specs(ClipboardConfig::default()).collect::<Vec<_>>();
        assert_eq!(
            actions
                .iter()
                .map(|action| (action.label, action.action))
                .collect::<Vec<_>>(),
            [
                (source_ui::SPLIT_RIGHT, "split-pane-right"),
                (source_ui::ADD_PANE_RIGHT, "add-pane-right"),
                (source_ui::ADD_PANE_LEFT, "add-pane-left"),
                (source_ui::NEW_PANE_BELOW, "split-pane-below"),
                (source_ui::MOVE_PANE_LEFT, "move-pane-left"),
                (source_ui::MOVE_PANE_RIGHT, "move-pane-right"),
                (source_ui::MOVE_PANE_UP, "move-pane-up"),
                (source_ui::MOVE_PANE_DOWN, "move-pane-down"),
                (
                    source_ui::MOVE_PANE_TO_NEW_WINDOW,
                    "move-pane-to-new-window"
                ),
                (source_ui::COPY, "copy"),
                (source_ui::CLEAN_COPY, "clean-copy"),
                (source_ui::COPY_AS_MARKDOWN, "copy-as-markdown"),
                (source_ui::CLOSE_PANE, "close-pane"),
            ]
        );
        assert!(actions.iter().all(|action| !action.icon.is_empty()));
    }

    #[test]
    fn automatic_clean_copy_substitutes_the_contextual_raw_escape_hatch() {
        let mut clipboard = ClipboardConfig {
            always_clean_copies: true,
            ..ClipboardConfig::default()
        };
        let actions = pane_action_specs(clipboard)
            .map(|action| (action.label, action.action))
            .collect::<Vec<_>>();
        assert!(actions.contains(&(source_ui::COPY_RAW, "copy-raw")));
        assert!(!actions.contains(&(source_ui::CLEAN_COPY, "clean-copy")));

        clipboard.show_copy_markdown_command = false;
        assert!(pane_action_specs(clipboard).all(|action| action.action != "copy-as-markdown"));
    }

    #[test]
    fn worklane_selection_is_independent_from_persistent_identity_color() {
        assert_eq!(selection_state(true), WorklaneSelectionState::Active);
        assert_eq!(selection_state(false), WorklaneSelectionState::Inactive);
        assert_eq!(selection_state(true).css_class(), "worklane-tint-active");
        assert_eq!(selection_state(false).css_class(), "worklane-tint-inactive");
    }

    #[test]
    fn worklane_drop_edge_changes_at_the_card_midpoint() {
        assert_eq!(WorklaneDropEdge::at(0.0, 100), WorklaneDropEdge::Before);
        assert_eq!(WorklaneDropEdge::at(49.9, 100), WorklaneDropEdge::Before);
        assert_eq!(WorklaneDropEdge::at(50.0, 100), WorklaneDropEdge::After);
        assert_eq!(WorklaneDropEdge::at(100.0, 100), WorklaneDropEdge::After);
        assert_eq!(WorklaneDropEdge::Before.as_str(), "before");
        assert_eq!(WorklaneDropEdge::After.as_str(), "after");
    }

    #[test]
    fn drag_feedback_contract_is_pinned_to_the_source_preview() {
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../Zentty/UI/Sidebar/SidebarView.swift"
        ));
        assert!(source.contains("button.setReorderDragActive(true)"));
        assert!(source.contains("syncReorderSpacer()"));
        assert!(source.contains("positionDraggedWorklaneButton"));
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
