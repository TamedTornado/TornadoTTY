use gtk::glib::variant::ToVariant;
use gtk::prelude::*;
use zentty_core::{FleetPaneSnapshot, FleetState, FleetSummary};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct IndicatorLayout {
    size: i32,
    horizontal_alignment: gtk::Align,
    vertical_alignment: gtk::Align,
    expand_horizontally: bool,
    expand_vertically: bool,
}

const INDICATOR_LAYOUT: IndicatorLayout = IndicatorLayout {
    size: 8,
    horizontal_alignment: gtk::Align::Center,
    vertical_alignment: gtk::Align::Center,
    expand_horizontally: false,
    expand_vertically: false,
};

pub(crate) fn install_styles() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(
        ".agent-fleet { min-width: 360px; padding: 8px; }\n\
         .agent-fleet-header { margin: 2px 4px 8px 4px; }\n\
         .agent-fleet-section { margin: 8px 6px 3px 6px; font-size: 0.82em; font-weight: 700; opacity: 0.72; }\n\
         .agent-fleet-row { padding: 6px 8px; border-radius: 6px; }\n\
         .agent-fleet-primary { font-weight: 650; }\n\
         .agent-fleet-context { opacity: 0.72; font-size: 0.88em; }\n\
         .agent-fleet-status { padding: 3px 7px; border-radius: 10px; font-size: 0.82em; font-weight: 700; }\n\
         .agent-fleet-status.waiting { background: alpha(@warning_bg_color, 0.22); color: @warning_color; }\n\
         .agent-fleet-status.stopped { background: alpha(@error_bg_color, 0.20); color: @error_color; }\n\
         .agent-fleet-status.compacting, .agent-fleet-status.active { background: alpha(@accent_bg_color, 0.18); color: @accent_color; }\n\
         .agent-fleet-status.idle { background: alpha(currentColor, 0.08); opacity: 0.72; }\n\
         .agent-fleet-empty { padding: 28px 20px; opacity: 0.72; }\n\
         .agent-fleet-dot { min-width: 8px; min-height: 8px; border-radius: 999px; background: alpha(currentColor, 0.35); }\n\
         .agent-fleet-dot.waiting { background: @warning_color; }\n\
         .agent-fleet-dot.stopped { background: @error_color; }\n\
         .agent-fleet-dot.compacting { background: @accent_color; }",
    );
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

pub(crate) fn button_content() -> (gtk::Box, gtk::Box) {
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    content.append(&gtk::Image::from_icon_name("system-run-symbolic"));
    let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    dot.add_css_class("agent-fleet-dot");
    dot.set_size_request(INDICATOR_LAYOUT.size, INDICATOR_LAYOUT.size);
    dot.set_halign(INDICATOR_LAYOUT.horizontal_alignment);
    dot.set_valign(INDICATOR_LAYOUT.vertical_alignment);
    dot.set_hexpand(INDICATOR_LAYOUT.expand_horizontally);
    dot.set_vexpand(INDICATOR_LAYOUT.expand_vertically);
    dot.set_visible(false);
    content.append(&dot);
    (content, dot)
}

pub(crate) fn update_indicator(dot: &gtk::Box, summary: FleetSummary) {
    for class in ["waiting", "stopped", "compacting", "active", "idle"] {
        dot.remove_css_class(class);
    }
    let Some(state) = indicator_state(summary) else {
        dot.set_visible(false);
        return;
    };
    dot.add_css_class(state_class(state));
    dot.set_visible(true);
}

fn indicator_state(summary: FleetSummary) -> Option<FleetState> {
    match summary.aggregate_state() {
        FleetState::Waiting => Some(FleetState::Waiting),
        FleetState::Stopped => Some(FleetState::Stopped),
        FleetState::Compacting => Some(FleetState::Compacting),
        FleetState::Active | FleetState::Idle => None,
    }
}

pub(crate) fn popover(snapshots: &[FleetPaneSnapshot]) -> gtk::Popover {
    let popover = gtk::Popover::new();
    render_popover(&popover, snapshots);
    popover
}

pub(crate) fn render_popover(popover: &gtk::Popover, snapshots: &[FleetPaneSnapshot]) {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 2);
    root.add_css_class("agent-fleet");
    let summary = FleetSummary::from_snapshots(snapshots);
    eprintln!(
        "zentty-linux: fleet-popover-render agents={} waiting={} stopped={} compacting={} active={} idle={} progress={}",
        snapshots.len(),
        summary.waiting_count,
        summary.stopped_count,
        summary.compacting_count,
        summary.active_count,
        summary.idle_count,
        snapshots
            .iter()
            .filter(|pane| pane.progress.is_some())
            .count(),
    );

    let header = gtk::Box::new(gtk::Orientation::Vertical, 2);
    header.add_css_class("agent-fleet-header");
    let title = gtk::Label::new(Some("Agent Status"));
    title.add_css_class("heading");
    title.set_xalign(0.0);
    header.append(&title);
    let summary_label = gtk::Label::new(Some(&summary.header()));
    summary_label.add_css_class("dim-label");
    summary_label.set_xalign(0.0);
    header.append(&summary_label);
    root.append(&header);

    if snapshots.is_empty() {
        let empty = gtk::Box::new(gtk::Orientation::Vertical, 8);
        empty.add_css_class("agent-fleet-empty");
        empty.set_halign(gtk::Align::Center);
        empty.append(&gtk::Image::from_icon_name("system-run-symbolic"));
        empty.append(&gtk::Label::new(Some("No agent panes")));
        root.append(&empty);
    } else {
        let list = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let mut initial_focus = None;
        append_section(
            &list,
            "Waiting",
            snapshots,
            |state| matches!(state, FleetState::Waiting | FleetState::Stopped),
            &mut initial_focus,
        );
        append_section(
            &list,
            "Running",
            snapshots,
            |state| matches!(state, FleetState::Compacting | FleetState::Active),
            &mut initial_focus,
        );
        append_section(
            &list,
            "Idle",
            snapshots,
            |state| state == FleetState::Idle,
            &mut initial_focus,
        );
        let scroll = gtk::ScrolledWindow::new();
        scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        scroll.set_max_content_height(500);
        scroll.set_propagate_natural_height(true);
        scroll.set_child(Some(&list));
        root.append(&scroll);
        if let Some(initial_focus) = initial_focus {
            root.connect_map(move |_| {
                let initial_focus = initial_focus.clone();
                gtk::glib::idle_add_local_once(move || {
                    initial_focus.grab_focus();
                });
            });
        }
    }

    append_footer(&root, snapshots.is_empty());
    popover.set_child(Some(&root));
}

fn append_footer(root: &gtk::Box, initially_empty: bool) {
    let footer = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    footer.set_margin_top(8);
    let settings = gtk::Button::with_mnemonic("_Settings…");
    settings.set_action_name(Some("workspace.open-settings-section"));
    settings.set_action_target_value(Some(&"agents".to_variant()));
    settings.set_hexpand(true);
    settings.connect_has_focus_notify(|button| {
        if button.has_focus() {
            eprintln!("zentty-linux: fleet-footer-focus action=settings");
        }
    });
    settings.connect_clicked(|_| {
        eprintln!("zentty-linux: fleet-footer-activate action=settings");
    });
    let quit = gtk::Button::with_mnemonic("_Quit Tornado TTY");
    quit.set_action_name(Some("workspace.quit-application"));
    quit.connect_has_focus_notify(|button| {
        if button.has_focus() {
            eprintln!("zentty-linux: fleet-footer-focus action=quit");
        }
    });
    quit.connect_clicked(|_| {
        eprintln!("zentty-linux: fleet-footer-activate action=quit");
    });
    footer.append(&settings);
    footer.append(&quit);
    root.append(&footer);
    if initially_empty {
        root.connect_map(move |_| {
            let settings = settings.clone();
            gtk::glib::idle_add_local_once(move || {
                settings.grab_focus();
            });
        });
    }
}

fn append_section(
    list: &gtk::Box,
    title: &str,
    snapshots: &[FleetPaneSnapshot],
    accepts: impl Fn(FleetState) -> bool,
    initial_focus: &mut Option<gtk::Button>,
) {
    let selected = snapshots
        .iter()
        .filter(|snapshot| accepts(snapshot.state))
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return;
    }
    let header = gtk::Label::new(Some(&format!("{title} ({})", selected.len())));
    header.add_css_class("agent-fleet-section");
    header.set_xalign(0.0);
    list.append(&header);
    for snapshot in selected {
        let row = row(snapshot);
        if initial_focus.is_none() {
            *initial_focus = Some(row.clone());
        }
        list.append(&row);
    }
}

fn row(snapshot: &FleetPaneSnapshot) -> gtk::Button {
    let button = gtk::Button::new();
    button.set_has_frame(false);
    button.add_css_class("agent-fleet-row");
    button.set_action_name(Some("workspace.activate-fleet-pane"));
    button.set_action_target_value(Some(&target_variant(snapshot)));

    let row_content = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
    text.set_hexpand(true);
    let primary = gtk::Label::new(Some(&snapshot.primary_text));
    primary.add_css_class("agent-fleet-primary");
    primary.set_xalign(0.0);
    primary.set_ellipsize(gtk::pango::EllipsizeMode::End);
    let context = gtk::Label::new(Some(&snapshot.context_text));
    context.add_css_class("agent-fleet-context");
    context.set_xalign(0.0);
    context.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    text.append(&primary);
    text.append(&context);
    row_content.append(&text);

    let status_text = fleet_status_text(snapshot);
    let status = gtk::Label::new(Some(&status_text));
    status.add_css_class("agent-fleet-status");
    status.add_css_class(state_class(snapshot.state));
    status.set_valign(gtk::Align::Center);
    row_content.append(&status);
    button.set_child(Some(&row_content));
    button.update_property(&[gtk::accessible::Property::Label(&format!(
        "{}, {}, {}",
        snapshot.primary_text, status_text, snapshot.context_text
    ))]);
    let target = snapshot.target.clone();
    button.connect_has_focus_notify(move |button| {
        if button.has_focus() {
            eprintln!(
                "zentty-linux: fleet-row-focus window={} worklane={} pane={}",
                target.window_id, target.worklane_id, target.pane_id
            );
        }
    });
    button
}

fn fleet_status_text(snapshot: &FleetPaneSnapshot) -> String {
    snapshot.progress.map_or_else(
        || snapshot.status_label.clone(),
        |progress| {
            format!(
                "{} · {}/{}",
                snapshot.status_label, progress.done, progress.total
            )
        },
    )
}

fn target_variant(snapshot: &FleetPaneSnapshot) -> gtk::glib::Variant {
    (
        snapshot.target.window_id.clone(),
        snapshot.target.worklane_id.clone(),
        snapshot.target.pane_id.clone(),
    )
        .to_variant()
}

fn state_class(state: FleetState) -> &'static str {
    match state {
        FleetState::Waiting => "waiting",
        FleetState::Stopped => "stopped",
        FleetState::Compacting => "compacting",
        FleetState::Active => "active",
        FleetState::Idle => "idle",
    }
}

#[cfg(test)]
mod tests {
    use zentty_core::{AgentProgress, AttentionTarget, FleetPaneSnapshot, FleetState, FleetSummary};

    use super::{INDICATOR_LAYOUT, IndicatorLayout, fleet_status_text, indicator_state};

    fn snapshot(progress: Option<AgentProgress>) -> FleetPaneSnapshot {
        FleetPaneSnapshot {
            target: AttentionTarget::new("window", "worklane", "pane"),
            window_title: "Zentty".to_owned(),
            worklane_title: "Worklane".to_owned(),
            agent_name: "Codex".to_owned(),
            primary_text: "Codex".to_owned(),
            context_text: "Worklane — Zentty".to_owned(),
            status_label: "Running".to_owned(),
            state: FleetState::Active,
            updated_at_ms: 1,
            progress,
        }
    }

    #[test]
    fn status_and_accessibility_share_visible_incomplete_progress_copy() {
        assert_eq!(
            fleet_status_text(&snapshot(Some(AgentProgress { done: 2, total: 5 }))),
            "Running · 2/5"
        );
        assert_eq!(fleet_status_text(&snapshot(None)), "Running");
    }

    #[test]
    fn chrome_indicator_is_compact_and_reserved_for_exceptional_states() {
        assert_eq!(
            INDICATOR_LAYOUT,
            IndicatorLayout {
                size: 8,
                horizontal_alignment: gtk::Align::Center,
                vertical_alignment: gtk::Align::Center,
                expand_horizontally: false,
                expand_vertically: false,
            }
        );
        assert_eq!(indicator_state(FleetSummary::default()), None);
        assert_eq!(
            indicator_state(FleetSummary {
                active_count: 1,
                ..FleetSummary::default()
            }),
            None
        );
        assert_eq!(
            indicator_state(FleetSummary {
                idle_count: 1,
                ..FleetSummary::default()
            }),
            None
        );
        assert_eq!(
            indicator_state(FleetSummary {
                compacting_count: 1,
                ..FleetSummary::default()
            }),
            Some(FleetState::Compacting)
        );
        assert_eq!(
            indicator_state(FleetSummary {
                stopped_count: 1,
                active_count: 2,
                ..FleetSummary::default()
            }),
            Some(FleetState::Stopped)
        );
        assert_eq!(
            indicator_state(FleetSummary {
                waiting_count: 1,
                stopped_count: 1,
                ..FleetSummary::default()
            }),
            Some(FleetState::Waiting)
        );
    }
}
