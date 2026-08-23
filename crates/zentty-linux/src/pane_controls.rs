use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::gdk;
use gtk::prelude::*;
use zentty_core::{PaneRightInsertionBehavior, WorklaneColor};

use crate::source_ui;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PaneControlAction {
    SplitRight,
    AddPaneRight,
    NewPaneBelow,
    MoveToNewWindow,
    ClosePane,
}

impl PaneControlAction {
    #[cfg(test)]
    const ALL: [Self; 5] = [
        Self::SplitRight,
        Self::AddPaneRight,
        Self::NewPaneBelow,
        Self::MoveToNewWindow,
        Self::ClosePane,
    ];

    pub(crate) const fn id(self) -> &'static str {
        match self {
            Self::SplitRight => "split-right",
            Self::AddPaneRight => "add-pane-right",
            Self::NewPaneBelow => "new-pane-below",
            Self::MoveToNewWindow => "move-pane-to-new-window",
            Self::ClosePane => "close-pane",
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::SplitRight => source_ui::SPLIT_RIGHT,
            Self::AddPaneRight => source_ui::ADD_PANE_RIGHT,
            Self::NewPaneBelow => source_ui::NEW_PANE_BELOW,
            Self::MoveToNewWindow => source_ui::MOVE_PANE_TO_NEW_WINDOW,
            Self::ClosePane => source_ui::CLOSE_PANE,
        }
    }

    const fn icon(self) -> &'static str {
        match self {
            Self::SplitRight | Self::AddPaneRight => "go-next-symbolic",
            Self::NewPaneBelow => "go-down-symbolic",
            Self::MoveToNewWindow => "window-new-symbolic",
            Self::ClosePane => "window-close-symbolic",
        }
    }
}

pub(crate) struct PaneFrame {
    root: gtk::Overlay,
    pane_id: String,
    label: gtk::Label,
    right_button: gtk::Button,
    move_to_window_button: gtk::Button,
    right_action: Rc<Cell<PaneControlAction>>,
    drag_zone: gtk::Label,
    drag_source: RefCell<Option<gtk::DragSource>>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PanePresentation {
    pub(crate) focused: bool,
    pub(crate) worklane_color: Option<WorklaneColor>,
    pub(crate) show_borders: bool,
    pub(crate) label: Option<String>,
}

impl PanePresentation {
    // GTK opacity applies to the fully composited Ghostty surface, not merely
    // its emphasis treatment. Keep terminal content opaque until Linux owns a
    // backdrop-aware equivalent of Zentty's inactive-pane presentation.
    const SURFACE_OPACITY: f64 = 1.0;

    fn color_class(&self) -> Option<String> {
        self.worklane_color
            .map(|color| format!("zentty-pane-color-{}", color.as_str()))
    }
}

impl PaneFrame {
    #[allow(clippy::too_many_lines)] // Declarative construction of one pane frame and its controls.
    pub(crate) fn new(
        pane_id: &str,
        terminal: &gtk::Widget,
        on_action: impl Fn(PaneControlAction) + 'static,
        on_pointer_presence: impl Fn(bool) + 'static,
    ) -> Self {
        let root = gtk::Overlay::new();
        root.add_css_class("zentty-pane-frame");
        root.set_hexpand(true);
        root.set_vexpand(true);
        root.set_child(Some(terminal));

        let label = gtk::Label::new(None);
        label.add_css_class("zentty-pane-label");
        label.set_halign(gtk::Align::Start);
        label.set_valign(gtk::Align::Start);
        label.set_margin_top(5);
        label.set_margin_start(6);
        label.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        label.set_max_width_chars(48);
        label.set_visible(false);
        root.add_overlay(&label);

        let controls = gtk::Box::new(gtk::Orientation::Horizontal, 2);
        controls.add_css_class("zentty-pane-controls");
        controls.set_halign(gtk::Align::End);
        controls.set_valign(gtk::Align::Start);
        controls.set_margin_top(8);
        controls.set_margin_end(8);
        controls.set_opacity(0.0);

        let on_action: Rc<dyn Fn(PaneControlAction)> = Rc::new(on_action);
        let right_action = Rc::new(Cell::new(PaneControlAction::SplitRight));
        let right_button = pane_control_button(PaneControlAction::SplitRight, pane_id);
        let clicked_action = Rc::clone(&right_action);
        let clicked_callback = Rc::clone(&on_action);
        right_button.connect_clicked(move |_| clicked_callback(clicked_action.get()));
        controls.append(&right_button);

        let move_to_window_button =
            pane_control_button(PaneControlAction::MoveToNewWindow, pane_id);
        let move_callback = Rc::clone(&on_action);
        move_to_window_button
            .connect_clicked(move |_| move_callback(PaneControlAction::MoveToNewWindow));
        controls.append(&move_to_window_button);

        for action in [
            PaneControlAction::NewPaneBelow,
            PaneControlAction::ClosePane,
        ] {
            let button = gtk::Button::new();
            configure_pane_control_button(&button, action, pane_id);
            let on_action = Rc::clone(&on_action);
            button.connect_clicked(move |_| on_action(action));
            controls.append(&button);
        }
        let drag_zone = gtk::Label::new(Some("•••"));
        drag_zone.set_widget_name(&format!("pane-drag-zone-{pane_id}"));
        drag_zone.add_css_class("zentty-pane-drag-zone");
        drag_zone.set_halign(gtk::Align::Fill);
        drag_zone.set_valign(gtk::Align::Start);
        drag_zone.set_height_request(15);
        drag_zone.set_hexpand(true);
        drag_zone.set_tooltip_text(Some("Drag pane"));
        drag_zone.update_property(&[gtk::accessible::Property::Label("Drag pane")]);
        let drag_motion = gtk::EventControllerMotion::new();
        let drag_pane_id = pane_id.to_owned();
        drag_motion.connect_enter(move |_, _, _| {
            eprintln!("zentty-linux: pane-drag-zone pane={drag_pane_id} pointer=entered");
        });
        drag_zone.add_controller(drag_motion);
        root.add_overlay(&drag_zone);
        // The full-width drag strip and the controls share the pane's top
        // edge. Keep the buttons above the strip so their complete hit targets
        // remain clickable rather than only the portion below the drag label.
        root.add_overlay(&controls);

        let revealed = Rc::new(Cell::new(false));
        let motion = gtk::EventControllerMotion::new();
        let focus = gtk::EventControllerFocus::new();
        let enter_controls = controls.clone();
        let enter_revealed = Rc::clone(&revealed);
        let enter_pane_id = pane_id.to_owned();
        let on_pointer_presence: Rc<dyn Fn(bool)> = Rc::new(on_pointer_presence);
        let enter_pointer = Rc::clone(&on_pointer_presence);
        motion.connect_enter(move |_, _, _| {
            set_revealed(&enter_controls, &enter_revealed, &enter_pane_id, true);
            enter_pointer(true);
        });
        let leave_controls = controls.clone();
        let leave_revealed = Rc::clone(&revealed);
        let leave_pane_id = pane_id.to_owned();
        let leave_focus = focus.clone();
        let leave_pointer = Rc::clone(&on_pointer_presence);
        motion.connect_leave(move |controller| {
            leave_pointer(false);
            let controller = controller.clone();
            let leave_controls = leave_controls.clone();
            let leave_revealed = Rc::clone(&leave_revealed);
            let leave_pane_id = leave_pane_id.clone();
            let leave_focus = leave_focus.clone();
            gtk::glib::idle_add_local_once(move || {
                if !controller.contains_pointer() && !leave_focus.contains_focus() {
                    set_revealed(&leave_controls, &leave_revealed, &leave_pane_id, false);
                }
            });
        });

        let focus_controls = controls.clone();
        let focus_revealed = Rc::clone(&revealed);
        let focus_pane_id = pane_id.to_owned();
        focus.connect_enter(move |_| {
            set_revealed(&focus_controls, &focus_revealed, &focus_pane_id, true);
        });
        let blur_controls = controls.clone();
        let blur_revealed = Rc::clone(&revealed);
        let blur_pane_id = pane_id.to_owned();
        let blur_motion = motion.clone();
        focus.connect_leave(move |controller| {
            let controller = controller.clone();
            let blur_controls = blur_controls.clone();
            let blur_revealed = Rc::clone(&blur_revealed);
            let blur_pane_id = blur_pane_id.clone();
            let blur_motion = blur_motion.clone();
            gtk::glib::idle_add_local_once(move || {
                if !controller.contains_focus() && !blur_motion.contains_pointer() {
                    set_revealed(&blur_controls, &blur_revealed, &blur_pane_id, false);
                }
            });
        });
        root.add_controller(motion);
        controls.add_controller(focus);

        Self {
            root,
            pane_id: pane_id.to_owned(),
            label,
            right_button,
            move_to_window_button,
            right_action,
            drag_zone,
            drag_source: RefCell::new(None),
        }
    }

    pub(crate) fn widget(&self) -> &gtk::Overlay {
        &self.root
    }

    pub(crate) fn set_presentation(&self, presentation: &PanePresentation) {
        self.root.remove_css_class("zentty-pane-frame-focused");
        self.root.remove_css_class("zentty-pane-frame-unfocused");
        self.root.remove_css_class("zentty-pane-borders-hidden");
        for color in WorklaneColor::ALL {
            self.root
                .remove_css_class(&format!("zentty-pane-color-{}", color.as_str()));
        }

        self.root.add_css_class(if presentation.focused {
            "zentty-pane-frame-focused"
        } else {
            "zentty-pane-frame-unfocused"
        });
        if let Some(color_class) = presentation.color_class() {
            self.root.add_css_class(&color_class);
        }
        if !presentation.show_borders {
            self.root.add_css_class("zentty-pane-borders-hidden");
        }
        self.label
            .set_label(presentation.label.as_deref().unwrap_or_default());
        self.label.set_visible(presentation.label.is_some());
        self.root.set_opacity(PanePresentation::SURFACE_OPACITY);
    }

    pub(crate) fn set_right_behavior(&self, behavior: PaneRightInsertionBehavior) {
        let action = match behavior {
            PaneRightInsertionBehavior::VisibleSplit => PaneControlAction::SplitRight,
            PaneRightInsertionBehavior::WorklaneAdd => PaneControlAction::AddPaneRight,
        };
        if self.right_action.replace(action) != action {
            configure_pane_control_button(&self.right_button, action, &self.pane_id);
            eprintln!(
                "zentty-linux: pane-right-control pane={} action={}",
                self.pane_id,
                action.id()
            );
        }
    }

    pub(crate) fn set_window_transfer_available(&self, available: bool) {
        self.move_to_window_button.set_sensitive(available);
    }

    pub(crate) fn install_drag_source(&self, payload: &crate::pane_drag_drop::PaneDragPayload) {
        if let Some(previous) = self.drag_source.borrow_mut().take() {
            self.drag_zone.remove_controller(&previous);
        }
        self.drag_source
            .replace(Some(crate::pane_drag_view::terminal_source(
                &self.drag_zone,
                payload,
            )));
    }

    pub(crate) fn detach_terminal(&self) {
        self.root.set_child(gtk::Widget::NONE);
    }
}

fn pane_control_button(action: PaneControlAction, pane_id: &str) -> gtk::Button {
    let button = gtk::Button::new();
    configure_pane_control_button(&button, action, pane_id);
    button
}

fn configure_pane_control_button(button: &gtk::Button, action: PaneControlAction, pane_id: &str) {
    button.add_css_class("zentty-pane-control");
    button.set_icon_name(action.icon());
    button.set_tooltip_text(Some(action.label()));
    button.set_accessible_role(gtk::AccessibleRole::Button);
    button.update_property(&[gtk::accessible::Property::Label(action.label())]);
    button.set_widget_name(&format!("pane-control-{}-{pane_id}", action.id()));
}

fn set_revealed(controls: &gtk::Box, revealed: &Cell<bool>, pane_id: &str, value: bool) {
    if revealed.replace(value) == value {
        return;
    }
    controls.set_opacity(if value { 1.0 } else { 0.0 });
    eprintln!(
        "zentty-linux: pane-controls pane={pane_id} state={}",
        if value { "shown" } else { "hidden" }
    );
}

pub(crate) fn install_styles() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(
        ".zentty-pane-frame {\n\
             border: 2px solid alpha(white, 0.08);\n\
         }\n\
         .zentty-pane-frame-focused {\n\
             border-color: #69a7ff;\n\
             box-shadow: inset 0 0 0 1px alpha(#69a7ff, 0.48), 0 0 14px alpha(#69a7ff, 0.22);\n\
         }\n\
         .zentty-pane-borders-hidden.zentty-pane-frame-unfocused { border-color: transparent; }\n\
         .zentty-pane-label { padding: 2px 6px; border-radius: 5px; color: alpha(white, 0.72); background: alpha(#15171b, 0.80); font-size: 10px; }\n\
         .zentty-pane-frame-focused.zentty-pane-color-red { border-color: #f56565; box-shadow: inset 0 0 0 1px alpha(#f56565, 0.30), 0 0 12px alpha(#f56565, 0.18); }\n\
         .zentty-pane-frame-focused.zentty-pane-color-orange { border-color: #ed8936; box-shadow: inset 0 0 0 1px alpha(#ed8936, 0.30), 0 0 12px alpha(#ed8936, 0.18); }\n\
         .zentty-pane-frame-focused.zentty-pane-color-amber { border-color: #d69e2e; box-shadow: inset 0 0 0 1px alpha(#d69e2e, 0.30), 0 0 12px alpha(#d69e2e, 0.18); }\n\
         .zentty-pane-frame-focused.zentty-pane-color-yellow { border-color: #ecc94b; box-shadow: inset 0 0 0 1px alpha(#ecc94b, 0.30), 0 0 12px alpha(#ecc94b, 0.18); }\n\
         .zentty-pane-frame-focused.zentty-pane-color-lime { border-color: #9ae6b4; box-shadow: inset 0 0 0 1px alpha(#9ae6b4, 0.30), 0 0 12px alpha(#9ae6b4, 0.18); }\n\
         .zentty-pane-frame-focused.zentty-pane-color-green { border-color: #48bb78; box-shadow: inset 0 0 0 1px alpha(#48bb78, 0.30), 0 0 12px alpha(#48bb78, 0.18); }\n\
         .zentty-pane-frame-focused.zentty-pane-color-teal { border-color: #38b2ac; box-shadow: inset 0 0 0 1px alpha(#38b2ac, 0.30), 0 0 12px alpha(#38b2ac, 0.18); }\n\
         .zentty-pane-frame-focused.zentty-pane-color-cyan { border-color: #4fd1c5; box-shadow: inset 0 0 0 1px alpha(#4fd1c5, 0.30), 0 0 12px alpha(#4fd1c5, 0.18); }\n\
         .zentty-pane-frame-focused.zentty-pane-color-blue { border-color: #4299e1; box-shadow: inset 0 0 0 1px alpha(#4299e1, 0.30), 0 0 12px alpha(#4299e1, 0.18); }\n\
         .zentty-pane-frame-focused.zentty-pane-color-indigo { border-color: #667eea; box-shadow: inset 0 0 0 1px alpha(#667eea, 0.30), 0 0 12px alpha(#667eea, 0.18); }\n\
         .zentty-pane-frame-focused.zentty-pane-color-purple { border-color: #9f7aea; box-shadow: inset 0 0 0 1px alpha(#9f7aea, 0.30), 0 0 12px alpha(#9f7aea, 0.18); }\n\
         .zentty-pane-frame-focused.zentty-pane-color-pink { border-color: #ed64a6; box-shadow: inset 0 0 0 1px alpha(#ed64a6, 0.30), 0 0 12px alpha(#ed64a6, 0.18); }\n\
         .zentty-pane-controls {\n\
             padding: 3px;\n\
             border-radius: 7px;\n\
             background: alpha(#15171b, 0.92);\n\
             border: 1px solid alpha(white, 0.16);\n\
         }\n\
         .zentty-pane-control {\n\
             min-width: 26px;\n\
             min-height: 26px;\n\
             padding: 0;\n\
             border-radius: 5px;\n\
             color: #eef0f4;\n\
             background: transparent;\n\
             box-shadow: none;\n\
         }\n\
         .zentty-pane-control:hover { background: alpha(white, 0.12); }\n\
         .zentty-pane-control:active { background: alpha(white, 0.20); }\n\
         .zentty-pane-frame .search-overlay {\n\
             padding: 8px 10px;\n\
             margin: 14px;\n\
             border-radius: 12px;\n\
             outline-style: solid;\n\
             outline-color: #596273;\n\
             outline-width: 1px;\n\
             background: alpha(#20242b, 0.97);\n\
             box-shadow: 0 10px 30px alpha(black, 0.55);\n\
             color: #f3f4f6;\n\
         }\n\
         .zentty-pane-frame .search-overlay entry {\n\
             min-height: 30px;\n\
             background: #15181d;\n\
             color: #f7f8fa;\n\
             caret-color: #ffffff;\n\
             border: 1px solid #717b8c;\n\
             border-radius: 6px;\n\
         }\n\
         .zentty-pane-frame .search-overlay entry:focus {\n\
             border-color: #5b9cff;\n\
             box-shadow: 0 0 0 1px #5b9cff;\n\
         }\n\
         .zentty-pane-frame .search-overlay button {\n\
             min-width: 28px;\n\
             min-height: 28px;\n\
             padding: 0;\n\
             color: #e8ebf0;\n\
             background: transparent;\n\
             box-shadow: none;\n\
         }\n\
         .zentty-pane-frame .search-overlay button:hover { background: alpha(white, 0.12); }",
    );
    gtk::style_context_add_provider_for_display(
        &gdk::Display::default().expect("GTK display initialized"),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

#[cfg(test)]
mod tests {
    use super::{PaneControlAction, PanePresentation};
    use crate::source_ui;
    use zentty_core::WorklaneColor;

    const PANE_SOURCE: &str = include_str!("../../../Zentty/UI/PaneStrip/PaneContainerView.swift");
    const CONFIG_SOURCE: &str = include_str!("../../../Zentty/Config/AppConfig.swift");

    #[test]
    fn pane_local_controls_use_current_source_commands_without_conflation() {
        assert_eq!(
            PaneControlAction::ALL.map(|action| (action.id(), action.label())),
            [
                ("split-right", source_ui::SPLIT_RIGHT),
                ("add-pane-right", source_ui::ADD_PANE_RIGHT),
                ("new-pane-below", source_ui::NEW_PANE_BELOW),
                (
                    "move-pane-to-new-window",
                    source_ui::MOVE_PANE_TO_NEW_WINDOW
                ),
                ("close-pane", source_ui::CLOSE_PANE),
            ]
        );
        assert_eq!(
            PaneControlAction::AddPaneRight.icon(),
            PaneControlAction::SplitRight.icon(),
            "the reviewed pane-local right-arrow glyph must not regress to the rejected generic application-add icon"
        );
    }

    #[test]
    fn pane_presentation_preserves_source_focus_without_washing_terminal_content() {
        let focused = PanePresentation {
            focused: true,
            worklane_color: Some(WorklaneColor::Amber),
            show_borders: true,
            label: Some("~/Projects/zentty".into()),
        };
        let inactive = PanePresentation {
            focused: false,
            worklane_color: None,
            show_borders: false,
            label: None,
        };

        assert!(
            (PanePresentation::SURFACE_OPACITY - 1.0).abs() < f64::EPSILON,
            "terminal content must not be washed out to indicate focus"
        );
        assert!(!inactive.focused);
        assert!(!inactive.show_borders);
        assert_eq!(focused.label.as_deref(), Some("~/Projects/zentty"));
        assert_eq!(
            focused.color_class().as_deref(),
            Some("zentty-pane-color-amber")
        );
        assert!(CONFIG_SOURCE.contains("showBorders: true"));
        assert!(CONFIG_SOURCE.contains("inactiveOpacity: 0.7"));
        assert!(PANE_SOURCE.contains("if isFocused, let worklaneColor"));
        assert!(PANE_SOURCE.contains("theme.paneBorderFocused"));
    }
}
