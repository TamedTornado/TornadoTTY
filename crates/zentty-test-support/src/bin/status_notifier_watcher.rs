use std::cell::RefCell;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk::gio;
use gtk::gio::prelude::DBusProxyExt;
use gtk::glib;
use gtk::glib::variant::ToVariant;
use serde_json::json;

const WATCHER_PATH: &str = "/StatusNotifierWatcher";
const WATCHER_INTERFACE: &str = "org.kde.StatusNotifierWatcher";
const ITEM_PATH: &str = "/StatusNotifierItem";
const ITEM_INTERFACE: &str = "org.kde.StatusNotifierItem";

const WATCHER_XML: &str = r#"
<node>
  <interface name="org.kde.StatusNotifierWatcher">
    <property name="RegisteredStatusNotifierItems" type="as" access="read"/>
    <property name="IsStatusNotifierHostRegistered" type="b" access="read"/>
    <property name="ProtocolVersion" type="i" access="read"/>
    <method name="RegisterStatusNotifierItem"><arg name="service" type="s" direction="in"/></method>
    <method name="RegisterStatusNotifierHost"><arg name="service" type="s" direction="in"/></method>
    <signal name="StatusNotifierItemRegistered"><arg name="service" type="s"/></signal>
    <signal name="StatusNotifierItemUnregistered"><arg name="service" type="s"/></signal>
    <signal name="StatusNotifierHostRegistered"/>
  </interface>
  <interface name="be.zenjoy.Zentty.TestStatusNotifierWatcher">
    <method name="Inspect"><arg name="status" type="s" direction="out"/><arg name="tooltip" type="s" direction="out"/></method>
    <method name="Activate"/>
    <method name="Quit"/>
  </interface>
</node>
"#;

#[derive(Default)]
struct State {
    item_service: Option<String>,
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
        "org.kde.StatusNotifierWatcher",
        gio::BusNameOwnerFlags::NONE,
        move |connection, _| {
            if let Err(error) =
                register_watcher(&connection, &state_for_bus, &loop_for_bus, &receipt_for_bus)
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

fn register_watcher(
    connection: &gio::DBusConnection,
    state: &Rc<RefCell<State>>,
    main_loop: &glib::MainLoop,
    receipt: &Path,
) -> Result<(), String> {
    let node = gio::DBusNodeInfo::for_xml(WATCHER_XML).map_err(|error| error.to_string())?;
    let watcher_info = &node.interfaces()[0];
    let state_for_method = Rc::clone(state);
    let receipt_for_method = receipt.to_path_buf();
    connection
        .register_object(WATCHER_PATH, watcher_info)
        .method_call(
            move |connection, _, _, _, method, parameters, invocation| match method {
                "RegisterStatusNotifierItem" => {
                    let Some((service,)) = parameters.get::<(String,)>() else {
                        invocation.return_dbus_error(
                            "org.freedesktop.DBus.Error.InvalidArgs",
                            "service must be a string",
                        );
                        return;
                    };
                    state_for_method.borrow_mut().item_service = Some(service.clone());
                    record(
                        &receipt_for_method,
                        &json!({"event":"registered","service":service}),
                    );
                    let _ = connection.emit_signal(
                        None,
                        WATCHER_PATH,
                        WATCHER_INTERFACE,
                        "StatusNotifierItemRegistered",
                        Some(&(service.as_str(),).to_variant()),
                    );
                    invocation.return_value(None);
                }
                "RegisterStatusNotifierHost" => invocation.return_value(None),
                _ => invocation.return_dbus_error(
                    "org.freedesktop.DBus.Error.UnknownMethod",
                    "unsupported watcher method",
                ),
            },
        )
        .property({
            let state = Rc::clone(state);
            move |_, _, _, _, property| match property {
                "RegisteredStatusNotifierItems" => state
                    .borrow()
                    .item_service
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .to_variant(),
                "IsStatusNotifierHostRegistered" => true.to_variant(),
                "ProtocolVersion" => 0_i32.to_variant(),
                _ => "".to_variant(),
            }
        })
        .build()
        .map_err(|error| error.to_string())?;

    let test_info = &node.interfaces()[1];
    let state_for_test = Rc::clone(state);
    let receipt_for_test = receipt.to_path_buf();
    let loop_for_test = main_loop.clone();
    connection
        .register_object(WATCHER_PATH, test_info)
        .method_call(
            move |connection, _, _, _, method, _, invocation| match method {
                "Inspect" => match inspect_item(&connection, &state_for_test.borrow()) {
                    Ok((status, tooltip)) => {
                        record(
                            &receipt_for_test,
                            &json!({"event":"inspected","status":status,"tooltip":tooltip}),
                        );
                        invocation.return_value(Some(&(status, tooltip).to_variant()));
                    }
                    Err(error) => invocation.return_dbus_error(
                        "be.zenjoy.Zentty.TestStatusNotifierWatcher.Error",
                        &error,
                    ),
                },
                "Activate" => match activate_item(&connection, &state_for_test.borrow()) {
                    Ok(()) => {
                        record(&receipt_for_test, &json!({"event":"activated"}));
                        invocation.return_value(None);
                    }
                    Err(error) => invocation.return_dbus_error(
                        "be.zenjoy.Zentty.TestStatusNotifierWatcher.Error",
                        &error,
                    ),
                },
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

fn item_proxy(connection: &gio::DBusConnection, state: &State) -> Result<gio::DBusProxy, String> {
    let service = state
        .item_service
        .as_deref()
        .ok_or_else(|| "no item has registered".to_owned())?;
    gio::DBusProxy::new_sync(
        connection,
        gio::DBusProxyFlags::NONE,
        None::<&gio::DBusInterfaceInfo>,
        Some(service),
        ITEM_PATH,
        ITEM_INTERFACE,
        gio::Cancellable::NONE,
    )
    .map_err(|error| format!("could not inspect item: {error}"))
}

fn inspect_item(
    connection: &gio::DBusConnection,
    state: &State,
) -> Result<(String, String), String> {
    let proxy = item_proxy(connection, state)?;
    let status = proxy
        .cached_property("Status")
        .and_then(|value| value.str().map(str::to_owned))
        .ok_or_else(|| "item Status property is missing".to_owned())?;
    let tooltip = proxy
        .cached_property("ToolTip")
        .and_then(|value| value.get::<(String, Vec<(i32, i32, Vec<u8>)>, String, String)>())
        .map(|(_, _, _, description)| description)
        .ok_or_else(|| "item ToolTip property is missing".to_owned())?;
    Ok((status, tooltip))
}

fn activate_item(connection: &gio::DBusConnection, state: &State) -> Result<(), String> {
    item_proxy(connection, state)?
        .call_sync(
            "Activate",
            Some(&(0_i32, 0_i32).to_variant()),
            gio::DBusCallFlags::NONE,
            2_000,
            gio::Cancellable::NONE,
        )
        .map(|_| ())
        .map_err(|error| format!("item activation failed: {error}"))
}

fn record(path: &Path, value: &serde_json::Value) {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .expect("could not open status-notifier receipt");
    writeln!(file, "{value}").expect("could not append status-notifier receipt");
    file.flush()
        .expect("could not flush status-notifier receipt");
}
