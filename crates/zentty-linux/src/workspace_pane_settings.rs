use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use zentty_core::{
    FocusFollowsMouseDelay, NewWorklanePlacement, PaneConfig, PaneLayoutConfig,
    PaneRightBehaviorMode, WorklaneConfig,
};

pub(crate) type ApplyWorkspacePanes =
    Rc<dyn Fn(WorklaneConfig, PaneLayoutConfig, PaneConfig) -> Result<(), String>>;

struct State {
    worklanes: WorklaneConfig,
    layout: PaneLayoutConfig,
    panes: PaneConfig,
    apply_changes: ApplyWorkspacePanes,
    status: gtk::Label,
}

#[allow(clippy::too_many_lines)] // Declarative construction of one focused settings page.
pub(crate) fn build(
    worklanes: WorklaneConfig,
    layout: PaneLayoutConfig,
    panes: PaneConfig,
    apply_changes: ApplyWorkspacePanes,
) -> gtk::Widget {
    eprintln!(
        "zentty-linux: workspace-pane-settings loaded placement={} right-behavior={} threshold={} labels={} borders={} icons={} smooth={} focus={} focus-delay={} inactive-opacity={}",
        worklanes.new_worklane_placement.config_value(),
        layout.right_split_behavior.config_value(),
        layout.visible_split_window_width,
        panes.show_labels,
        panes.show_borders,
        panes.show_project_icons,
        panes.smooth_scroll_enabled,
        panes.focus_follows_mouse,
        panes.focus_follows_mouse_delay.config_value(),
        panes.inactive_opacity_percent,
    );
    let root = gtk::Box::new(gtk::Orientation::Vertical, 16);
    root.set_margin_top(28);
    root.set_margin_bottom(28);
    root.set_margin_start(30);
    root.set_margin_end(30);

    let title = gtk::Label::new(Some("Worklanes & Panes"));
    title.add_css_class("title-1");
    title.set_halign(gtk::Align::Start);
    root.append(&title);
    let subtitle = gtk::Label::new(Some(
        "Fine-tune worklane placement, pane context, and focus cues.",
    ));
    subtitle.add_css_class("dim-label");
    subtitle.set_halign(gtk::Align::Start);
    root.append(&subtitle);

    let state = Rc::new(RefCell::new(State {
        worklanes,
        layout,
        panes,
        apply_changes,
        status: gtk::Label::new(None),
    }));

    let worklane_card = card("Worklanes");
    let placement = gtk::DropDown::from_strings(&["Top", "After current", "End"]);
    placement.set_widget_name("settings-worklane-placement");
    instrument_focus(&placement, "new-worklane-placement");
    install_dropdown_boundary_keys(&placement);
    placement.set_selected(match worklanes.new_worklane_placement {
        NewWorklanePlacement::Top => 0,
        NewWorklanePlacement::AfterCurrent => 1,
        NewWorklanePlacement::End => 2,
    });
    worklane_card.append(&setting_row(
        "_New worklane placement",
        "Choose where newly created worklanes appear in the sidebar.",
        &placement,
    ));
    root.append(&worklane_card);
    {
        let state = Rc::clone(&state);
        placement.connect_selected_notify(move |control| {
            let mut next = state.borrow().worklanes;
            next.new_worklane_placement = match control.selected() {
                0 => NewWorklanePlacement::Top,
                2 => NewWorklanePlacement::End,
                _ => NewWorklanePlacement::AfterCurrent,
            };
            emit_changes(&state, Some(next), None, None, "new-worklane-placement");
        });
    }

    let split_card = card("Pane right behavior");
    let split = gtk::DropDown::from_strings(&["Adaptive", "Always Split", "Always Add"]);
    split.set_widget_name("settings-pane-right-behavior");
    instrument_focus(&split, "pane-right-behavior");
    install_dropdown_boundary_keys(&split);
    split.set_selected(match layout.right_split_behavior {
        PaneRightBehaviorMode::Adaptive => 0,
        PaneRightBehaviorMode::AlwaysSplit => 1,
        PaneRightBehaviorMode::AlwaysAdd => 2,
    });
    split_card.append(&setting_row(
        "_Right-pane command",
        "Adaptive splits visibly at the selected window width and adds offscreen below it.",
        &split,
    ));
    let threshold =
        gtk::DropDown::from_strings(&["1200 pt", "1440 pt", "1680 pt", "1920 pt", "2560 pt"]);
    threshold.set_widget_name("settings-pane-split-threshold");
    instrument_focus(&threshold, "pane-split-threshold");
    install_dropdown_boundary_keys(&threshold);
    threshold.set_selected(match layout.visible_split_window_width {
        1200 => 0,
        1440 => 1,
        1680 => 2,
        2560 => 4,
        _ => 3,
    });
    threshold.set_sensitive(layout.right_split_behavior == PaneRightBehaviorMode::Adaptive);
    split_card.append(&setting_row(
        "Adaptive split _threshold",
        "At this window width or wider, the primary right-pane action splits visibly.",
        &threshold,
    ));
    root.append(&split_card);
    {
        let state = Rc::clone(&state);
        let threshold = threshold.clone();
        split.connect_selected_notify(move |control| {
            let mode = match control.selected() {
                1 => PaneRightBehaviorMode::AlwaysSplit,
                2 => PaneRightBehaviorMode::AlwaysAdd,
                _ => PaneRightBehaviorMode::Adaptive,
            };
            let mut next = state.borrow().layout;
            next.right_split_behavior = mode;
            if emit_changes(&state, None, Some(next), None, "pane-right-behavior") {
                threshold.set_sensitive(mode == PaneRightBehaviorMode::Adaptive);
            }
        });
    }
    {
        let state = Rc::clone(&state);
        threshold.connect_selected_notify(move |control| {
            let mut next = state.borrow().layout;
            next.visible_split_window_width = match control.selected() {
                0 => 1200,
                1 => 1440,
                2 => 1680,
                4 => 2560,
                _ => 1920,
            };
            emit_changes(&state, None, Some(next), None, "pane-split-threshold");
        });
    }

    let display_card = card("Display & focus");
    append_switch(
        &display_card,
        "Show pane _labels",
        "Show the compact path label at the top left of each pane.",
        "settings-pane-labels",
        panes.show_labels,
        &state,
        |panes, value| panes.show_labels = value,
    );
    append_switch(
        &display_card,
        "Show pane _borders",
        "When off, only the focused pane keeps its border.",
        "settings-pane-borders",
        panes.show_borders,
        &state,
        |panes, value| panes.show_borders = value,
    );
    append_switch(
        &display_card,
        "Show project _icons",
        "Show discovered project icons in the worklane sidebar.",
        "settings-pane-project-icons",
        panes.show_project_icons,
        &state,
        |panes, value| panes.show_project_icons = value,
    );
    append_unavailable_switch(
        &display_card,
        "Smooth terminal scrolling",
        "Unavailable until Ghostty scroll events have a qualified Linux smoothing path.",
        "settings-pane-smooth-scroll",
        panes.smooth_scroll_enabled,
    );

    let focus_delay = gtk::DropDown::from_strings(&["Immediate", "Short"]);
    focus_delay.set_widget_name("settings-pane-focus-delay");
    instrument_focus(&focus_delay, "focus-follows-mouse-delay");
    install_dropdown_boundary_keys(&focus_delay);
    focus_delay.set_selected(match panes.focus_follows_mouse_delay {
        FocusFollowsMouseDelay::Immediate => 0,
        FocusFollowsMouseDelay::Short => 1,
    });
    let focus_available = layout.right_split_behavior != PaneRightBehaviorMode::AlwaysAdd;
    focus_delay.set_sensitive(focus_available && panes.focus_follows_mouse);
    let focus_switch = gtk::Switch::builder()
        .active(panes.focus_follows_mouse)
        .sensitive(focus_available)
        .valign(gtk::Align::Center)
        .build();
    focus_switch.set_widget_name("settings-pane-focus-follows-mouse");
    instrument_focus(&focus_switch, "focus-follows-mouse");
    display_card.append(&setting_row(
        "Focus follows _mouse",
        "Move keyboard focus to a pane when the pointer enters it. Unavailable in Always Add mode.",
        &focus_switch,
    ));
    display_card.append(&setting_row(
        "Focus _delay",
        "A short delay prevents accidental focus changes while crossing panes.",
        &focus_delay,
    ));
    {
        let state = Rc::clone(&state);
        let focus_delay = focus_delay.clone();
        focus_switch.connect_active_notify(move |control| {
            let mut next = state.borrow().panes;
            next.focus_follows_mouse = control.is_active();
            if emit_changes(&state, None, None, Some(next), "focus-follows-mouse") {
                focus_delay.set_sensitive(control.is_sensitive() && control.is_active());
            }
        });
    }
    {
        let state = Rc::clone(&state);
        focus_delay.connect_selected_notify(move |control| {
            let mut next = state.borrow().panes;
            next.focus_follows_mouse_delay = if control.selected() == 0 {
                FocusFollowsMouseDelay::Immediate
            } else {
                FocusFollowsMouseDelay::Short
            };
            emit_changes(&state, None, None, Some(next), "focus-follows-mouse-delay");
        });
    }
    {
        let focus_switch = focus_switch.clone();
        let focus_delay = focus_delay.clone();
        split.connect_selected_notify(move |control| {
            let available = control.selected() != 2;
            focus_switch.set_sensitive(available);
            focus_delay.set_sensitive(available && focus_switch.is_active());
        });
    }

    let opacity = gtk::Scale::with_range(gtk::Orientation::Horizontal, 60.0, 100.0, 1.0);
    opacity.set_value(f64::from(panes.inactive_opacity_percent));
    opacity.set_draw_value(true);
    opacity.set_value_pos(gtk::PositionType::Right);
    opacity.set_hexpand(true);
    opacity.set_widget_name("settings-pane-inactive-opacity");
    opacity.set_sensitive(false);
    display_card.append(&setting_row(
        "Non-focused pane opacity",
        "Unavailable: whole-surface GTK opacity washed out terminal content during Linux dogfood.",
        &opacity,
    ));
    root.append(&display_card);

    let status = state.borrow().status.clone();
    status.set_halign(gtk::Align::Start);
    status.set_wrap(true);
    status.add_css_class("dim-label");
    root.append(&status);

    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&root)
        .build();
    scroll.update_property(&[gtk::accessible::Property::Label(
        "Worklanes & Panes Settings",
    )]);
    scroll.upcast()
}

fn emit_changes(
    state: &Rc<RefCell<State>>,
    worklanes: Option<WorklaneConfig>,
    layout: Option<PaneLayoutConfig>,
    panes: Option<PaneConfig>,
    control: &str,
) -> bool {
    let (worklanes, layout, panes, apply) = {
        let current = state.borrow();
        (
            worklanes.unwrap_or(current.worklanes),
            layout.unwrap_or(current.layout),
            panes.unwrap_or(current.panes),
            Rc::clone(&current.apply_changes),
        )
    };
    match apply(worklanes, layout, panes) {
        Ok(()) => {
            let mut accepted = state.borrow_mut();
            accepted.worklanes = worklanes;
            accepted.layout = layout;
            accepted.panes = panes;
            accepted.status.set_text("");
            eprintln!("zentty-linux: workspace-pane-settings control={control} result=applied");
            true
        }
        Err(error) => {
            state.borrow().status.set_text(&format!(
                "Could not save Worklanes & Panes settings: {error}"
            ));
            eprintln!(
                "zentty-linux: workspace-pane-settings control={control} result=error error={error}"
            );
            false
        }
    }
}

fn append_switch(
    card: &gtk::Box,
    title: &str,
    subtitle: &str,
    name: &str,
    active: bool,
    state: &Rc<RefCell<State>>,
    update: fn(&mut PaneConfig, bool),
) {
    let control = gtk::Switch::builder()
        .active(active)
        .valign(gtk::Align::Center)
        .build();
    control.set_widget_name(name);
    instrument_focus(&control, name);
    card.append(&setting_row(title, subtitle, &control));
    let state = Rc::clone(state);
    control.connect_active_notify(move |control| {
        let mut next = state.borrow().panes;
        update(&mut next, control.is_active());
        emit_changes(
            &state,
            None,
            None,
            Some(next),
            control.widget_name().as_str(),
        );
    });
}

fn append_unavailable_switch(card: &gtk::Box, title: &str, reason: &str, name: &str, active: bool) {
    let control = gtk::Switch::builder()
        .active(active)
        .sensitive(false)
        .valign(gtk::Align::Center)
        .build();
    control.set_widget_name(name);
    control.update_property(&[gtk::accessible::Property::Description(reason)]);
    card.append(&setting_row(title, reason, &control));
}

fn card(title: &str) -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 10);
    card.add_css_class("zentty-settings-card");
    let heading = gtk::Label::new(Some(title));
    heading.add_css_class("heading");
    heading.set_halign(gtk::Align::Start);
    card.append(&heading);
    card
}

fn setting_row(title: &str, subtitle: &str, control: &impl IsA<gtk::Widget>) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    let labels = gtk::Box::new(gtk::Orientation::Vertical, 3);
    labels.set_hexpand(true);
    let title = gtk::Label::new(Some(title));
    title.set_use_underline(true);
    title.set_mnemonic_widget(Some(control));
    title.set_halign(gtk::Align::Start);
    title.add_css_class("heading");
    let subtitle = gtk::Label::new(Some(subtitle));
    subtitle.set_halign(gtk::Align::Start);
    subtitle.set_wrap(true);
    subtitle.add_css_class("dim-label");
    labels.append(&title);
    labels.append(&subtitle);
    row.append(&labels);
    row.append(control);
    row
}

fn instrument_focus(control: &impl IsA<gtk::Widget>, name: &str) {
    let focus = gtk::EventControllerFocus::new();
    let name = name.to_owned();
    focus.connect_enter(move |_| {
        eprintln!("zentty-linux: workspace-pane-settings focus={name}");
    });
    control.add_controller(focus);
}

fn install_dropdown_boundary_keys(control: &gtk::DropDown) {
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let dropdown = control.clone();
    keys.connect_key_pressed(move |_, key, _, modifiers| {
        let selected = if modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK)
            && key == gtk::gdk::Key::Home
        {
            Some(0)
        } else if modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK)
            && key == gtk::gdk::Key::End
        {
            dropdown
                .model()
                .and_then(|model| model.n_items().checked_sub(1))
        } else {
            None
        };
        selected.map_or(gtk::glib::Propagation::Proceed, |selected| {
            dropdown.set_selected(selected);
            gtk::glib::Propagation::Stop
        })
    });
    control.add_controller(keys);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_page_controls_are_named_and_owned_by_the_shared_config_models() {
        let source =
            include_str!("../../../Zentty/UI/Settings/SettingsSectionViewControllers.swift");
        for phrase in [
            "New worklane placement",
            "Pane right behavior",
            "Show pane labels",
            "Show pane borders",
            "Show project icons",
            "Smooth terminal scrolling",
            "Focus follows mouse",
            "Non-focused pane opacity",
        ] {
            assert!(
                source.contains(phrase),
                "source no longer contains {phrase}"
            );
        }
        assert_eq!(PaneConfig::default().inactive_opacity_percent, 70);
        assert_eq!(PaneLayoutConfig::default().visible_split_window_width, 1920);
    }
}
