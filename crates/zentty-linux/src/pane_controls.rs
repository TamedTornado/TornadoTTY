use std::cell::Cell;
use std::rc::Rc;

use gtk::gdk;
use gtk::prelude::*;

use crate::source_ui;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PaneControlAction {
    SplitRight,
    NewPaneBelow,
    ClosePane,
}

impl PaneControlAction {
    const ALL: [Self; 3] = [Self::SplitRight, Self::NewPaneBelow, Self::ClosePane];

    pub(crate) const fn id(self) -> &'static str {
        match self {
            Self::SplitRight => "split-right",
            Self::NewPaneBelow => "new-pane-below",
            Self::ClosePane => "close-pane",
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::SplitRight => source_ui::SPLIT_RIGHT,
            Self::NewPaneBelow => source_ui::NEW_PANE_BELOW,
            Self::ClosePane => source_ui::CLOSE_PANE,
        }
    }

    const fn icon(self) -> &'static str {
        match self {
            Self::SplitRight => "go-next-symbolic",
            Self::NewPaneBelow => "go-down-symbolic",
            Self::ClosePane => "window-close-symbolic",
        }
    }
}

pub(crate) struct PaneFrame {
    root: gtk::Overlay,
}

impl PaneFrame {
    pub(crate) fn new(
        pane_id: &str,
        terminal: &gtk::Widget,
        on_action: impl Fn(PaneControlAction) + 'static,
    ) -> Self {
        let root = gtk::Overlay::new();
        root.add_css_class("zentty-pane-frame");
        root.set_hexpand(true);
        root.set_vexpand(true);
        root.set_child(Some(terminal));

        let controls = gtk::Box::new(gtk::Orientation::Horizontal, 2);
        controls.add_css_class("zentty-pane-controls");
        controls.set_halign(gtk::Align::End);
        controls.set_valign(gtk::Align::Start);
        controls.set_margin_top(8);
        controls.set_margin_end(8);
        controls.set_opacity(0.0);

        let on_action: Rc<dyn Fn(PaneControlAction)> = Rc::new(on_action);
        for action in PaneControlAction::ALL {
            let button = gtk::Button::new();
            button.add_css_class("zentty-pane-control");
            button.set_icon_name(action.icon());
            button.set_tooltip_text(Some(action.label()));
            button.set_accessible_role(gtk::AccessibleRole::Button);
            button.update_property(&[gtk::accessible::Property::Label(action.label())]);
            button.set_widget_name(&format!("pane-control-{}-{pane_id}", action.id()));
            let on_action = Rc::clone(&on_action);
            button.connect_clicked(move |_| on_action(action));
            controls.append(&button);
        }
        root.add_overlay(&controls);

        let revealed = Rc::new(Cell::new(false));
        let motion = gtk::EventControllerMotion::new();
        let focus = gtk::EventControllerFocus::new();
        let enter_controls = controls.clone();
        let enter_revealed = Rc::clone(&revealed);
        let enter_pane_id = pane_id.to_owned();
        motion.connect_enter(move |_, _, _| {
            set_revealed(&enter_controls, &enter_revealed, &enter_pane_id, true);
        });
        let leave_controls = controls.clone();
        let leave_revealed = Rc::clone(&revealed);
        let leave_pane_id = pane_id.to_owned();
        let leave_focus = focus.clone();
        motion.connect_leave(move |controller| {
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

        Self { root }
    }

    pub(crate) fn widget(&self) -> &gtk::Overlay {
        &self.root
    }

    pub(crate) fn detach_terminal(&self) {
        self.root.set_child(gtk::Widget::NONE);
    }
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
        ".zentty-pane-controls {\n\
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
         .zentty-pane-control:active { background: alpha(white, 0.20); }",
    );
    gtk::style_context_add_provider_for_display(
        &gdk::Display::default().expect("GTK display initialized"),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

#[cfg(test)]
mod tests {
    use super::PaneControlAction;
    use crate::source_ui;

    #[test]
    fn pane_local_controls_use_current_source_commands_without_conflation() {
        assert_eq!(
            PaneControlAction::ALL.map(|action| (action.id(), action.label())),
            [
                ("split-right", source_ui::SPLIT_RIGHT),
                ("new-pane-below", source_ui::NEW_PANE_BELOW),
                ("close-pane", source_ui::CLOSE_PANE),
            ]
        );
    }
}
