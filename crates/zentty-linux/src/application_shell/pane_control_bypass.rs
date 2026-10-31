//! Window-owned modifier observation; terminal events are never consumed.
use super::ApplicationShell;
use gtk::{gdk, glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

/// Delivers observed Ctrl state to the shell's pane runtime.
///
/// These handlers can run inside a mutable shell borrow: closing a popover
/// under the pointer makes GTK synthesize a crossing synchronously, so the
/// window's motion controller fires while the code that closed it still holds
/// the shell. A blocked update waits for an idle, which applies the newest
/// state because GDK events outrank idles and may have superseded this one.
struct BypassObserver {
    shell: Weak<RefCell<ApplicationShell>>,
    desired: Cell<bool>,
    idle_pending: Cell<bool>,
}

impl BypassObserver {
    fn observe(self: &Rc<Self>, bypassed: bool) {
        self.desired.set(bypassed);
        let Some(shell) = self.shell.upgrade() else {
            return;
        };
        if let Ok(shell) = shell.try_borrow() {
            shell.pane_runtime.set_controls_bypassed(bypassed);
        } else if !self.idle_pending.replace(true) {
            let observer = Rc::clone(self);
            glib::idle_add_local_once(move || {
                observer.idle_pending.set(false);
                observer.observe(observer.desired.get());
            });
        }
    }
}

pub(super) fn install(shell: &Rc<RefCell<ApplicationShell>>) {
    let window = shell.borrow().window.clone();
    let observer = Rc::new(BypassObserver {
        shell: Rc::downgrade(shell),
        desired: Cell::new(false),
        idle_pending: Cell::new(false),
    });
    let key = gtk::EventControllerKey::new();
    key.set_propagation_phase(gtk::PropagationPhase::Capture);
    // GDK key-event modifier masks describe the state before that key's
    // transition. Observe Ctrl itself as well, so stationary press/release
    // does not wait for a later mouse/key event. Track both sides separately.
    let held_controls = Rc::new(Cell::new(0_u8));
    let held = Rc::clone(&held_controls);
    let on_event = Rc::clone(&observer);
    key.connect_key_pressed(move |_, key, _, _| {
        let bit = control_bit(key);
        if bit != 0 {
            held.set(held.get() | bit);
            on_event.observe(true);
        }
        glib::Propagation::Proceed
    });
    let held = Rc::clone(&held_controls);
    let on_event = Rc::clone(&observer);
    key.connect_key_released(move |_, key, _, _| {
        let bit = control_bit(key);
        if bit != 0 {
            held.set(held.get() & !bit);
            on_event.observe(held.get() != 0);
        }
    });
    let on_event = Rc::clone(&observer);
    key.connect_modifiers(move |_, modifiers| {
        on_event.observe(modifiers.contains(gdk::ModifierType::CONTROL_MASK));
        glib::Propagation::Proceed
    });
    window.add_controller(key);

    // Covers entering a window with Ctrl already held, including when another
    // pane owns keyboard focus. Capture observes motion above toolbar children.
    let motion = gtk::EventControllerMotion::new();
    motion.set_propagation_phase(gtk::PropagationPhase::Capture);
    let on_event = Rc::clone(&observer);
    motion.connect_enter(move |controller, _, _| {
        on_event.observe(
            controller
                .current_event_state()
                .contains(gdk::ModifierType::CONTROL_MASK),
        );
    });
    let on_event = Rc::clone(&observer);
    motion.connect_motion(move |controller, _, _| {
        on_event.observe(
            controller
                .current_event_state()
                .contains(gdk::ModifierType::CONTROL_MASK),
        );
    });
    window.add_controller(motion);

    // A release delivered to another application must not strand this window.
    window.connect_is_active_notify(move |window| {
        if !window.is_active() {
            held_controls.set(0);
        }
        let held = window.is_active()
            && gtk::prelude::WidgetExt::display(window)
                .default_seat()
                .and_then(|seat| seat.keyboard())
                .is_some_and(|keyboard| {
                    keyboard
                        .modifier_state()
                        .contains(gdk::ModifierType::CONTROL_MASK)
                });
        observer.observe(held);
    });
}

fn control_bit(key: gdk::Key) -> u8 {
    match key {
        gdk::Key::Control_L => 1,
        gdk::Key::Control_R => 2,
        _ => 0,
    }
}
