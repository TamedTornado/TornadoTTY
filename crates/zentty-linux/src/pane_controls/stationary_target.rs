//! GTK 4.14 caches the pointer target until motion or button release. After
//! bypass, the first stationary press still targets the terminal. Route only
//! that click to the newly restored toolbar button; never synthesize input or
//! unmap the terminal to refresh GTK's pointer cache.
use super::ControlVisibility;
use gtk::{glib, prelude::*};
use std::{cell::RefCell, rc::Rc};

pub(super) fn install(root: &gtk::Overlay, controls: &gtk::Box, state: &Rc<ControlVisibility>) {
    let motion = gtk::EventControllerMotion::new();
    let current = Rc::clone(state);
    motion.connect_motion(move |_, _, _| current.retarget_after_bypass.set(false));
    root.add_controller(motion);

    let click = gtk::GestureClick::new();
    click.set_button(1);
    click.set_propagation_phase(gtk::PropagationPhase::Capture);
    let pressed = Rc::new(RefCell::new(None::<glib::WeakRef<gtk::Button>>));
    let pending = Rc::clone(&pressed);
    let current = Rc::clone(state);
    let root_weak = root.downgrade();
    let controls_weak = controls.downgrade();
    click.connect_pressed(move |gesture, _, x, y| {
        if current.bypassed.get() || !current.retarget_after_bypass.get() {
            return;
        }
        let (Some(root), Some(controls)) = (root_weak.upgrade(), controls_weak.upgrade()) else {
            return;
        };
        if let Some(button) = button_at(&root, &controls, x, y) {
            pending.replace(Some(button.downgrade()));
            button.set_state_flags(gtk::StateFlags::ACTIVE, false);
            gesture.set_state(gtk::EventSequenceState::Claimed);
        }
    });
    let pending = Rc::clone(&pressed);
    let root_weak = root.downgrade();
    let controls_weak = controls.downgrade();
    let current = Rc::clone(state);
    click.connect_released(move |_, _, x, y| {
        let button = pending.borrow_mut().take().and_then(|weak| weak.upgrade());
        if let Some(button) = button {
            button.unset_state_flags(gtk::StateFlags::ACTIVE);
            current.retarget_after_bypass.set(false);
            let under_pointer = root_weak
                .upgrade()
                .zip(controls_weak.upgrade())
                .and_then(|(root, controls)| button_at(&root, &controls, x, y));
            if !current.bypassed.get() && under_pointer.as_ref() == Some(&button) {
                button.emit_clicked();
            }
        }
    });
    click.connect_cancel(move |_, _| {
        if let Some(button) = pressed.borrow_mut().take().and_then(|weak| weak.upgrade()) {
            button.unset_state_flags(gtk::StateFlags::ACTIVE);
        }
    });
    root.add_controller(click);
}

fn button_at(root: &gtk::Overlay, controls: &gtk::Box, x: f64, y: f64) -> Option<gtk::Button> {
    let picked = root.pick(x, y, gtk::PickFlags::DEFAULT)?;
    let button = picked
        .clone()
        .downcast::<gtk::Button>()
        .ok()
        .or_else(|| picked.ancestor(gtk::Button::static_type())?.downcast().ok())?;
    (button.is_sensitive() && button.is_ancestor(controls)).then_some(button)
}
