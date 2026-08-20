use std::cell::RefCell;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk::gio;
use gtk::glib;
use gtk::glib::variant::ToVariant;
use serde_json::json;

const SERVICE_PATH: &str = "/org/freedesktop/Notifications";
const SERVICE_INTERFACE: &str = "org.freedesktop.Notifications";

const SERVICE_XML: &str = r#"
<node>
  <interface name="org.freedesktop.Notifications">
    <method name="Notify">
      <arg name="app_name" type="s" direction="in"/>
      <arg name="replaces_id" type="u" direction="in"/>
      <arg name="app_icon" type="s" direction="in"/>
      <arg name="summary" type="s" direction="in"/>
      <arg name="body" type="s" direction="in"/>
      <arg name="actions" type="as" direction="in"/>
      <arg name="hints" type="a{sv}" direction="in"/>
      <arg name="expire_timeout" type="i" direction="in"/>
      <arg name="id" type="u" direction="out"/>
    </method>
    <method name="GetCapabilities"><arg name="capabilities" type="as" direction="out"/></method>
    <method name="GetServerInformation">
      <arg name="name" type="s" direction="out"/>
      <arg name="vendor" type="s" direction="out"/>
      <arg name="version" type="s" direction="out"/>
      <arg name="spec_version" type="s" direction="out"/>
    </method>
    <signal name="NotificationClosed"><arg name="id" type="u"/><arg name="reason" type="u"/></signal>
    <signal name="ActionInvoked"><arg name="id" type="u"/><arg name="action_key" type="s"/></signal>
  </interface>
  <interface name="be.zenjoy.Zentty.TestNotificationService">
    <method name="ActivateLatest"/>
    <method name="CloseLatest"/>
    <method name="Inspect"><arg name="count" type="u" direction="out"/></method>
    <method name="Quit"/>
  </interface>
</node>
"#;

#[derive(Default)]
struct State {
    next_id: u32,
    latest_id: Option<u32>,
    count: u32,
}

fn main() {
    let receipt = parse_receipt_path();
    let main_loop = glib::MainLoop::new(None, false);
    let state = Rc::new(RefCell::new(State::default()));
    let loop_for_bus = main_loop.clone();
    let state_for_bus = Rc::clone(&state);
    let receipt_for_bus = receipt.clone();
    let _owner = gio::bus_own_name(
        gio::BusType::Session,
        "org.freedesktop.Notifications",
        gio::BusNameOwnerFlags::NONE,
        move |connection, _| {
            if let Err(error) =
                register_service(&connection, &state_for_bus, &loop_for_bus, &receipt_for_bus)
            {
                record(&receipt_for_bus, &json!({"event":"error","detail":error}));
                loop_for_bus.quit();
            }
        },
        {
            let receipt = receipt.clone();
            move |_, _| record(&receipt, &json!({"event":"ready"}))
        },
        {
            let receipt = receipt.clone();
            let main_loop = main_loop.clone();
            move |_, _| {
                record(&receipt, &json!({"event":"name-lost"}));
                main_loop.quit();
            }
        },
    );
    main_loop.run();
}

fn parse_receipt_path() -> PathBuf {
    let mut arguments = std::env::args_os();
    let _program = arguments.next();
    assert_eq!(arguments.next().as_deref(), Some("--receipt".as_ref()));
    let receipt = arguments.next().expect("--receipt requires a path");
    assert!(arguments.next().is_none(), "unexpected additional argument");
    PathBuf::from(receipt)
}

#[allow(clippy::too_many_lines)]
fn register_service(
    connection: &gio::DBusConnection,
    state: &Rc<RefCell<State>>,
    main_loop: &glib::MainLoop,
    receipt: &Path,
) -> Result<(), String> {
    let node = gio::DBusNodeInfo::for_xml(SERVICE_XML).map_err(|error| error.to_string())?;
    let state_for_notify = Rc::clone(state);
    let receipt_for_notify = receipt.to_path_buf();
    connection
        .register_object(SERVICE_PATH, &node.interfaces()[0])
        .method_call(
            move |_, _, _, _, method, parameters, invocation| match method {
                "Notify" => {
                    let Some((app, _, _, summary, body, actions, _, _)) = parameters.get::<(
                        String,
                        u32,
                        String,
                        String,
                        String,
                        Vec<String>,
                        std::collections::HashMap<String, glib::Variant>,
                        i32,
                    )>(
                    ) else {
                        invocation.return_dbus_error(
                            "org.freedesktop.DBus.Error.InvalidArgs",
                            "invalid Notify payload",
                        );
                        return;
                    };
                    let mut state = state_for_notify.borrow_mut();
                    state.next_id = state.next_id.saturating_add(1).max(1);
                    state.latest_id = Some(state.next_id);
                    state.count = state.count.saturating_add(1);
                    record(
                        &receipt_for_notify,
                        &json!({
                            "event":"notify",
                            "id":state.next_id,
                            "app":app,
                            "summary":summary,
                            "body":body,
                            "actions":actions,
                        }),
                    );
                    invocation.return_value(Some(&(state.next_id,).to_variant()));
                }
                "GetCapabilities" => {
                    invocation
                        .return_value(Some(&(vec!["actions", "body", "sound"],).to_variant()));
                }
                "GetServerInformation" => invocation.return_value(Some(
                    &("Zentty Test Notifications", "Zentty", "1", "1.2").to_variant(),
                )),
                _ => invocation.return_dbus_error(
                    "org.freedesktop.DBus.Error.UnknownMethod",
                    "unsupported notification method",
                ),
            },
        )
        .build()
        .map_err(|error| error.to_string())?;

    let state_for_test = Rc::clone(state);
    let receipt_for_test = receipt.to_path_buf();
    let loop_for_test = main_loop.clone();
    connection
        .register_object(SERVICE_PATH, &node.interfaces()[1])
        .method_call(
            move |connection, _, _, _, method, _, invocation| match method {
                "ActivateLatest" => {
                    let Some(id) = state_for_test.borrow().latest_id else {
                        invocation.return_dbus_error(
                            "be.zenjoy.Zentty.TestNotificationService.Error",
                            "no notification has been delivered",
                        );
                        return;
                    };
                    let _ = connection.emit_signal(
                        None,
                        SERVICE_PATH,
                        SERVICE_INTERFACE,
                        "ActionInvoked",
                        Some(&(id, "default").to_variant()),
                    );
                    record(&receipt_for_test, &json!({"event":"activated","id":id}));
                    invocation.return_value(None);
                }
                "CloseLatest" => {
                    let Some(id) = state_for_test.borrow().latest_id else {
                        invocation.return_dbus_error(
                            "be.zenjoy.Zentty.TestNotificationService.Error",
                            "no notification has been delivered",
                        );
                        return;
                    };
                    let _ = connection.emit_signal(
                        None,
                        SERVICE_PATH,
                        SERVICE_INTERFACE,
                        "NotificationClosed",
                        Some(&(id, 2_u32).to_variant()),
                    );
                    record(&receipt_for_test, &json!({"event":"closed","id":id}));
                    invocation.return_value(None);
                }
                "Inspect" => {
                    let count = state_for_test.borrow().count;
                    invocation.return_value(Some(&(count,).to_variant()));
                }
                "Quit" => {
                    invocation.return_value(None);
                    loop_for_test.quit();
                }
                _ => invocation.return_dbus_error(
                    "org.freedesktop.DBus.Error.UnknownMethod",
                    "unsupported test method",
                ),
            },
        )
        .build()
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn record(path: &Path, value: &serde_json::Value) {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .expect("could not open notification receipt");
    writeln!(file, "{value}").expect("could not append notification receipt");
    file.flush().expect("could not flush notification receipt");
}
