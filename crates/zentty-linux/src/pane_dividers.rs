use std::cell::Cell;
use std::rc::Rc;

use gtk::gdk;
use gtk::prelude::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PaneDivider {
    Column {
        after_column_id: String,
    },
    Pane {
        column_id: String,
        after_pane_id: String,
    },
}

impl PaneDivider {
    fn axis(&self) -> gtk::Orientation {
        match self {
            Self::Column { .. } => gtk::Orientation::Horizontal,
            Self::Pane { .. } => gtk::Orientation::Vertical,
        }
    }

    fn name(&self) -> String {
        match self {
            Self::Column { after_column_id } => {
                format!("pane-divider-column-after-{after_column_id}")
            }
            Self::Pane {
                column_id,
                after_pane_id,
            } => format!("pane-divider-{column_id}-after-{after_pane_id}"),
        }
    }

    fn label(&self) -> String {
        match self {
            Self::Column { after_column_id } => {
                format!("Resize columns after {after_column_id}")
            }
            Self::Pane { after_pane_id, .. } => {
                format!("Resize panes after {after_pane_id}")
            }
        }
    }
}

pub(crate) fn install_styles() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(
        ".zentty-pane-divider { background: transparent; }\n\
         .zentty-pane-divider:hover,\n\
         .zentty-pane-divider:focus-visible { background: alpha(@theme_selected_bg_color, 0.55); }",
    );
    if let Some(display) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

pub(crate) fn new_handle(
    divider: &PaneDivider,
    on_delta: impl Fn(f64) -> f64 + 'static,
    on_equalize: impl Fn() + 'static,
) -> gtk::Box {
    let axis = divider.axis();
    let handle = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    handle.add_css_class("zentty-pane-divider");
    handle.set_widget_name(&divider.name());
    handle.set_focusable(true);
    handle.set_can_focus(true);
    handle.set_accessible_role(gtk::AccessibleRole::Separator);
    handle.update_property(&[gtk::accessible::Property::Label(&divider.label())]);
    match axis {
        gtk::Orientation::Horizontal => {
            handle.set_width_request(9);
            handle.set_halign(gtk::Align::End);
            handle.set_valign(gtk::Align::Fill);
            handle.set_cursor_from_name(Some("col-resize"));
        }
        gtk::Orientation::Vertical => {
            handle.set_height_request(9);
            handle.set_halign(gtk::Align::Fill);
            handle.set_valign(gtk::Align::Start);
            handle.set_cursor_from_name(Some("row-resize"));
        }
        _ => unreachable!("GTK orientation is exhaustive for pane dividers"),
    }

    let on_delta: Rc<dyn Fn(f64) -> f64> = Rc::new(on_delta);
    let last_offset = Rc::new(Cell::new(0.0));
    let drag = gtk::GestureDrag::new();
    let begin_offset = Rc::clone(&last_offset);
    drag.connect_drag_begin(move |_, _, _| begin_offset.set(0.0));
    let update_offset = Rc::clone(&last_offset);
    let update_callback = Rc::clone(&on_delta);
    let drag_handle = handle.clone();
    drag.connect_drag_update(move |_, x, y| {
        let offset = if axis == gtk::Orientation::Horizontal {
            x
        } else {
            y
        };
        let delta = offset - update_offset.replace(offset);
        if delta.abs() > f64::EPSILON {
            let applied = update_callback(delta);
            if axis == gtk::Orientation::Vertical && applied.abs() > f64::EPSILON {
                drag_handle.set_margin_top(adjusted_margin(&drag_handle, applied));
            }
        }
    });
    handle.add_controller(drag);

    let pointer_identity = divider.name();
    let pointer = gtk::EventControllerMotion::new();
    pointer.connect_enter(move |_, _, _| {
        eprintln!("zentty-linux: pane-divider-pointer id={pointer_identity} state=enter");
    });
    handle.add_controller(pointer);

    let click = gtk::GestureClick::new();
    click.set_button(gdk::BUTTON_PRIMARY);
    click.connect_released(move |_, presses, _, _| {
        if presses == 2 {
            on_equalize();
        }
    });
    handle.add_controller(click);

    let key_callback = Rc::clone(&on_delta);
    let key_handle = handle.clone();
    let keys = gtk::EventControllerKey::new();
    keys.connect_key_pressed(move |_, key, _, _| {
        let delta = match (axis, key) {
            (gtk::Orientation::Horizontal, gdk::Key::Left)
            | (gtk::Orientation::Vertical, gdk::Key::Up) => Some(-16.0),
            (gtk::Orientation::Horizontal, gdk::Key::Right)
            | (gtk::Orientation::Vertical, gdk::Key::Down) => Some(16.0),
            _ => None,
        };
        if let Some(delta) = delta {
            let applied = key_callback(delta);
            if axis == gtk::Orientation::Vertical && applied.abs() > f64::EPSILON {
                // Keyboard movement must keep the visible handle attached to
                // the resized boundary just like pointer movement.
                key_handle.set_margin_top(adjusted_margin(&key_handle, applied));
            }
            gtk::glib::Propagation::Stop
        } else {
            gtk::glib::Propagation::Proceed
        }
    });
    handle.add_controller(keys);
    handle
}

fn adjusted_margin(handle: &gtk::Box, delta: f64) -> i32 {
    let margin = (f64::from(handle.margin_top()) + delta)
        .round()
        .clamp(0.0, f64::from(i32::MAX));
    #[allow(clippy::cast_possible_truncation)]
    {
        margin as i32
    }
}

#[cfg(test)]
mod tests {
    use super::PaneDivider;

    #[test]
    fn divider_identity_exposes_source_axis_and_stable_accessibility_text() {
        let column = PaneDivider::Column {
            after_column_id: "column-left".to_owned(),
        };
        assert_eq!(column.axis(), gtk::Orientation::Horizontal);
        assert_eq!(column.name(), "pane-divider-column-after-column-left");
        assert_eq!(column.label(), "Resize columns after column-left");

        let pane = PaneDivider::Pane {
            column_id: "column-left".to_owned(),
            after_pane_id: "pane-top".to_owned(),
        };
        assert_eq!(pane.axis(), gtk::Orientation::Vertical);
        assert_eq!(pane.name(), "pane-divider-column-left-after-pane-top");
        assert_eq!(pane.label(), "Resize panes after pane-top");
    }
}
