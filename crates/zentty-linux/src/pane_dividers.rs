use std::cell::Cell;
use std::rc::Rc;

use gtk::gdk;
use gtk::prelude::*;
pub(crate) use zentty_linux::pane_divider_model::PaneDivider;
use zentty_linux::pane_divider_model::{DividerAxis, DividerKey, adjusted_vertical_margin};

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
    handle.set_widget_name(&divider.widget_name());
    handle.set_focusable(true);
    handle.set_can_focus(true);
    handle.set_accessible_role(gtk::AccessibleRole::Separator);
    handle.update_property(&[gtk::accessible::Property::Label(
        &divider.accessible_label(),
    )]);
    match axis {
        DividerAxis::Horizontal => {
            handle.set_width_request(9);
            handle.set_halign(gtk::Align::End);
            handle.set_valign(gtk::Align::Fill);
            handle.set_cursor_from_name(Some("col-resize"));
        }
        DividerAxis::Vertical => {
            handle.set_height_request(9);
            handle.set_halign(gtk::Align::Fill);
            handle.set_valign(gtk::Align::Start);
            handle.set_cursor_from_name(Some("row-resize"));
        }
    }

    let on_delta: Rc<dyn Fn(f64) -> f64> = Rc::new(on_delta);
    let last_offset = Rc::new(Cell::new(0.0));
    let drag = gtk::GestureDrag::new();
    let begin_offset = Rc::clone(&last_offset);
    drag.connect_drag_begin(move |_, _, _| begin_offset.set(0.0));
    let update_offset = Rc::clone(&last_offset);
    let update_callback = Rc::clone(&on_delta);
    let drag_handle = handle.clone();
    let drag_divider = divider.clone();
    drag.connect_drag_update(move |_, x, y| {
        let offset = drag_divider.pointer_offset(x, y);
        let delta = offset - update_offset.replace(offset);
        if let Some(request) = drag_divider.resize_request(delta) {
            let applied = request.apply(|payload| update_callback(payload.delta));
            if axis == DividerAxis::Vertical && applied.abs() > f64::EPSILON {
                drag_handle
                    .set_margin_top(adjusted_vertical_margin(drag_handle.margin_top(), applied));
            }
        }
    });
    handle.add_controller(drag);

    let pointer_identity = divider.widget_name();
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
    let key_divider = divider.clone();
    let keys = gtk::EventControllerKey::new();
    keys.connect_key_pressed(move |_, key, _, _| {
        let key = match key {
            gdk::Key::Left => Some(DividerKey::Left),
            gdk::Key::Right => Some(DividerKey::Right),
            gdk::Key::Up => Some(DividerKey::Up),
            gdk::Key::Down => Some(DividerKey::Down),
            _ => None,
        };
        if let Some(request) = key.and_then(|key| key_divider.keyboard_request(key)) {
            let applied = request.apply(|payload| key_callback(payload.delta));
            if axis == DividerAxis::Vertical && applied.abs() > f64::EPSILON {
                // Keyboard movement must keep the visible handle attached to
                // the resized boundary just like pointer movement.
                key_handle
                    .set_margin_top(adjusted_vertical_margin(key_handle.margin_top(), applied));
            }
            gtk::glib::Propagation::Stop
        } else {
            gtk::glib::Propagation::Proceed
        }
    });
    handle.add_controller(keys);
    handle
}
