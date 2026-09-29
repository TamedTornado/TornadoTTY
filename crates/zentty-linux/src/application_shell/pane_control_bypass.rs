//! Window-owned modifier observation; terminal events are never consumed.
use super::ApplicationShell;
use gtk::{gdk, glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

pub(super) fn install(shell: &Rc<RefCell<ApplicationShell>>) {
    let window = shell.borrow().window.clone();
    let key = gtk::EventControllerKey::new();
    key.set_propagation_phase(gtk::PropagationPhase::Capture);
    // GDK key-event modifier masks describe the state before that key's
    // transition. Observe Ctrl itself as well, so stationary press/release
    // does not wait for a later mouse/key event. Track both sides separately.
    let held_controls = Rc::new(Cell::new(0_u8));
    let held = Rc::clone(&held_controls);
    let weak = Rc::downgrade(shell);
    key.connect_key_pressed(move |_, key, _, _| {
        let bit = control_bit(key);
        if bit != 0 {
            held.set(held.get() | bit);
            if let Some(shell) = weak.upgrade() {
                shell.borrow().pane_runtime.set_controls_bypassed(true);
            }
        }
        glib::Propagation::Proceed
    });
    let held = Rc::clone(&held_controls);
    let weak = Rc::downgrade(shell);
    key.connect_key_released(move |_, key, _, _| {
        let bit = control_bit(key);
        if bit != 0 {
            held.set(held.get() & !bit);
            if let Some(shell) = weak.upgrade() {
                shell
                    .borrow()
                    .pane_runtime
                    .set_controls_bypassed(held.get() != 0);
            }
        }
    });
    let weak = Rc::downgrade(shell);
    key.connect_modifiers(move |_, modifiers| {
        if let Some(shell) = weak.upgrade() {
            shell
                .borrow()
                .pane_runtime
                .set_controls_bypassed(modifiers.contains(gdk::ModifierType::CONTROL_MASK));
        }
        glib::Propagation::Proceed
    });
    window.add_controller(key);

    // Covers entering a window with Ctrl already held, including when another
    // pane owns keyboard focus. Capture observes motion above toolbar children.
    let motion = gtk::EventControllerMotion::new();
    motion.set_propagation_phase(gtk::PropagationPhase::Capture);
    let weak = Rc::downgrade(shell);
    motion.connect_enter(move |controller, _, _| {
        if let Some(shell) = weak.upgrade() {
            shell.borrow().pane_runtime.set_controls_bypassed(
                controller
                    .current_event_state()
                    .contains(gdk::ModifierType::CONTROL_MASK),
            );
        }
    });
    let weak = Rc::downgrade(shell);
    motion.connect_motion(move |controller, _, _| {
        if let Some(shell) = weak.upgrade() {
            shell.borrow().pane_runtime.set_controls_bypassed(
                controller
                    .current_event_state()
                    .contains(gdk::ModifierType::CONTROL_MASK),
            );
        }
    });
    window.add_controller(motion);

    // A release delivered to another application must not strand this window.
    let weak = Rc::downgrade(shell);
    window.connect_is_active_notify(move |window| {
        if !window.is_active() {
            held_controls.set(0);
        }
        if let Some(shell) = weak.upgrade() {
            let held = window.is_active()
                && gtk::prelude::WidgetExt::display(window)
                    .default_seat()
                    .and_then(|seat| seat.keyboard())
                    .is_some_and(|keyboard| {
                        keyboard
                            .modifier_state()
                            .contains(gdk::ModifierType::CONTROL_MASK)
                    });
            shell.borrow().pane_runtime.set_controls_bypassed(held);
        }
    });
}

fn control_bit(key: gdk::Key) -> u8 {
    match key {
        gdk::Key::Control_L => 1,
        gdk::Key::Control_R => 2,
        _ => 0,
    }
}
