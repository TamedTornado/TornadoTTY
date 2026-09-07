//! Wait for the actual transient-unit job result, not merely its enqueue ACK.
use gtk::{gio, glib};
use std::cell::RefCell;
use std::rc::Rc;

pub(super) fn start(
    connection: &gio::DBusConnection,
    parameters: &glib::Variant,
    descriptors: Option<&gio::UnixFDList>,
) -> Result<(), String> {
    let context = glib::MainContext::new();
    context
        .with_thread_default(|| {
            let job = Rc::new(RefCell::new(None::<String>));
            let outcome = Rc::new(RefCell::new(None::<Result<(), String>>));
            let loop_ = glib::MainLoop::new(Some(&context), false);
            let (expected, result, completed) =
                (Rc::clone(&job), Rc::clone(&outcome), loop_.clone());
            let subscription = connection.subscribe_to_signal(
                Some("org.freedesktop.systemd1"),
                Some("org.freedesktop.systemd1.Manager"),
                Some("JobRemoved"),
                Some("/org/freedesktop/systemd1"),
                None,
                gio::DBusSignalFlags::NONE,
                move |signal| {
                    let parameters = signal.parameters;
                    let Some((_, path, _, status)) =
                        parameters.get::<(u32, glib::variant::ObjectPath, String, String)>()
                    else {
                        return;
                    };
                    if expected.borrow().as_deref() != Some(path.as_str()) {
                        return;
                    }
                    *result.borrow_mut() = Some(if status == "done" {
                        Ok(())
                    } else {
                        Err(format!("unit job failed: {status}"))
                    });
                    completed.quit();
                },
            );
            let response = connection.call_with_unix_fd_list_sync(
                Some("org.freedesktop.systemd1"),
                "/org/freedesktop/systemd1",
                "org.freedesktop.systemd1.Manager",
                "StartTransientUnit",
                Some(parameters),
                None,
                gio::DBusCallFlags::NONE,
                5_000,
                descriptors,
                gio::Cancellable::NONE,
            );
            let result = match response {
                Err(error) => Err(format!("transient unit request failed: {error}")),
                Ok((reply, _)) => match reply.get::<(glib::variant::ObjectPath,)>() {
                    None => Err("invalid transient-unit job reply".into()),
                    Some((path,)) => {
                        *job.borrow_mut() = Some(path.as_str().to_owned());
                        let deadline_loop = loop_.clone();
                        let deadline = glib::timeout_source_new(
                            std::time::Duration::from_secs(5),
                            Some("workload-unit-job-deadline"),
                            glib::Priority::DEFAULT,
                            move || {
                                deadline_loop.quit();
                                glib::ControlFlow::Break
                            },
                        );
                        deadline.attach(Some(&context));
                        loop_.run();
                        deadline.destroy();
                        outcome
                            .borrow_mut()
                            .take()
                            .unwrap_or_else(|| Err("transient unit job deadline exceeded".into()))
                    }
                },
            };
            drop(subscription);
            result
        })
        .map_err(|error| format!("cannot own unit-job context: {error}"))?
}
