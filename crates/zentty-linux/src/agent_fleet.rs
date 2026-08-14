use gtk::glib::variant::ToVariant;
use gtk::prelude::*;
use zentty_core::{FleetPaneSnapshot, FleetState, FleetSummary};

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
         .agent-fleet-dot.compacting, .agent-fleet-dot.active { background: @accent_color; }",
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
    dot.set_visible(false);
    content.append(&dot);
    (content, dot)
}

pub(crate) fn update_indicator(dot: &gtk::Box, summary: FleetSummary) {
    for class in ["waiting", "stopped", "compacting", "active", "idle"] {
        dot.remove_css_class(class);
    }
    if summary.total_count() == 0 {
        dot.set_visible(false);
        return;
    }
    dot.add_css_class(state_class(summary.aggregate_state()));
    dot.set_visible(summary.aggregate_state() != FleetState::Idle);
}

pub(crate) fn popover(snapshots: &[FleetPaneSnapshot]) -> gtk::Popover {
    let popover = gtk::Popover::new();
    let root = gtk::Box::new(gtk::Orientation::Vertical, 2);
    root.add_css_class("agent-fleet");
    let summary = FleetSummary::from_snapshots(snapshots);

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
            popover.connect_map(move |_| {
                initial_focus.grab_focus();
            });
        }
    }

    let footer = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    footer.set_margin_top(8);
    let settings = gtk::Button::with_label("Settings…");
    settings.set_action_name(Some("workspace.open-settings-section"));
    settings.set_action_target_value(Some(&"agents".to_variant()));
    settings.set_hexpand(true);
    let quit = gtk::Button::with_label("Quit Zentty");
    quit.set_action_name(Some("workspace.quit-application"));
    footer.append(&settings);
    footer.append(&quit);
    root.append(&footer);
    popover.set_child(Some(&root));
    popover
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

    let content = gtk::Box::new(gtk::Orientation::Horizontal, 10);
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
    content.append(&text);

    let status_text = snapshot.progress.map_or_else(
        || snapshot.status_label.clone(),
        |progress| {
            format!(
                "{} · {}/{}",
                snapshot.status_label, progress.done, progress.total
            )
        },
    );
    let status = gtk::Label::new(Some(&status_text));
    status.add_css_class("agent-fleet-status");
    status.add_css_class(state_class(snapshot.state));
    status.set_valign(gtk::Align::Center);
    content.append(&status);
    button.set_child(Some(&content));
    button.update_property(&[gtk::accessible::Property::Label(&format!(
        "{}, {}, {}",
        snapshot.primary_text, snapshot.status_label, snapshot.context_text
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
