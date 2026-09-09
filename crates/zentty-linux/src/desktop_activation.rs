//! The host owns desktop registration; Ghostty remains the terminal engine.
//! Register before workspace persistence/agent startup so secondary launches
//! cannot independently restore or overwrite the primary's workspace.
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use gtk::{gio, prelude::*};

use crate::application::ApplicationCoordinator;

pub(crate) struct DesktopActivation {
    application: gtk::Application,
    target: Rc<RefCell<Weak<RefCell<ApplicationCoordinator>>>>,
    pending: Rc<Cell<bool>>,
}

impl DesktopActivation {
    pub(crate) fn application(&self) -> &gtk::Application {
        &self.application
    }

    pub(crate) fn register(independent: bool) -> Result<Self, String> {
        let flags = if independent {
            gio::ApplicationFlags::NON_UNIQUE
        } else {
            gio::ApplicationFlags::FLAGS_NONE
        };
        let application = gtk::Application::builder()
            .application_id(zentty_core::APPLICATION_ID)
            .flags(flags)
            .build();
        let target = Rc::new(RefCell::new(Weak::<RefCell<ApplicationCoordinator>>::new()));
        let weak_target = Rc::clone(&target);
        let pending = Rc::new(Cell::new(false));
        let pending_activation = Rc::clone(&pending);
        application.connect_activate(move |_| {
            if let Some(coordinator) = weak_target.borrow().upgrade() {
                coordinator.borrow().present_for_desktop_activation();
            } else {
                // Coalesce activations received while the initial workspace
                // is being constructed. No timer and no duplicate restore.
                pending_activation.set(true);
            }
        });
        // Ghostty owns the private process-default GhosttyApplication and its
        // native lifecycle. The host alone registers the desktop identity and
        // associates windows; never replace the engine's private default.
        application
            .register(gio::Cancellable::NONE)
            .map_err(|error| {
                format!("could not register TornadoTTY desktop application: {error}")
            })?;
        if !independent && application.dbus_connection().is_none() {
            return Err("desktop activation requires a session bus; refusing an uncoordinated workspace restore".into());
        }
        Ok(Self {
            application,
            target,
            pending,
        })
    }

    pub(crate) fn forward_if_remote(&self) -> Result<bool, String> {
        if !self.application.is_remote() {
            return Ok(false);
        }
        // GtkApplication carries the desktop activation context. Do not route
        // this through pane IPC or start another workspace just to find one.
        self.application.activate();
        if let Some(bus) = self.application.dbus_connection() {
            bus.flush_sync(gio::Cancellable::NONE)
                .map_err(|error| format!("could not forward desktop activation: {error}"))?;
        }
        Ok(true)
    }

    pub(crate) fn bind(&self, coordinator: &Rc<RefCell<ApplicationCoordinator>>) {
        *self.target.borrow_mut() = Rc::downgrade(coordinator);
        if self.pending.replace(false) {
            coordinator.borrow().present_for_desktop_activation();
        }
    }
}
