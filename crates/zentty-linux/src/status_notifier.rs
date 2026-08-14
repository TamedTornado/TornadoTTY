use std::cell::RefCell;
use std::rc::Rc;

use gtk::gio;
use gtk::gio::prelude::DBusProxyExt;
use gtk::glib;
use gtk::glib::variant::{ObjectPath, ToVariant};
use zentty_core::{FleetPaneSnapshot, FleetState, FleetSummary};

const WATCHER_SERVICE: &str = "org.kde.StatusNotifierWatcher";
const WATCHER_PATH: &str = "/StatusNotifierWatcher";
const WATCHER_INTERFACE: &str = "org.kde.StatusNotifierWatcher";
const ITEM_PATH: &str = "/StatusNotifierItem";
const ITEM_INTERFACE: &str = "org.kde.StatusNotifierItem";
const CALL_TIMEOUT_MS: i32 = 2_000;

const ITEM_XML: &str = r#"
<node>
  <interface name="org.kde.StatusNotifierItem">
    <property name="Category" type="s" access="read"/>
    <property name="Id" type="s" access="read"/>
    <property name="Title" type="s" access="read"/>
    <property name="Status" type="s" access="read"/>
    <property name="WindowId" type="u" access="read"/>
    <property name="IconName" type="s" access="read"/>
    <property name="IconPixmap" type="a(iiay)" access="read"/>
    <property name="OverlayIconName" type="s" access="read"/>
    <property name="OverlayIconPixmap" type="a(iiay)" access="read"/>
    <property name="AttentionIconName" type="s" access="read"/>
    <property name="AttentionIconPixmap" type="a(iiay)" access="read"/>
    <property name="AttentionMovieName" type="s" access="read"/>
    <property name="ToolTip" type="(sa(iiay)ss)" access="read"/>
    <property name="ItemIsMenu" type="b" access="read"/>
    <property name="Menu" type="o" access="read"/>
    <method name="ContextMenu"><arg name="x" type="i" direction="in"/><arg name="y" type="i" direction="in"/></method>
    <method name="Activate"><arg name="x" type="i" direction="in"/><arg name="y" type="i" direction="in"/></method>
    <method name="SecondaryActivate"><arg name="x" type="i" direction="in"/><arg name="y" type="i" direction="in"/></method>
    <method name="Scroll"><arg name="delta" type="i" direction="in"/><arg name="orientation" type="s" direction="in"/></method>
    <signal name="NewTitle"/>
    <signal name="NewIcon"/>
    <signal name="NewAttentionIcon"/>
    <signal name="NewOverlayIcon"/>
    <signal name="NewToolTip"/>
    <signal name="NewStatus"><arg name="status" type="s"/></signal>
  </interface>
</node>
"#;

pub(crate) fn watcher_available() -> bool {
    create_backend().is_ok_and(|backend| backend.watcher.name_owner().is_some())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Presentation {
    status: &'static str,
    title: String,
    tooltip: String,
}

impl Presentation {
    fn from_snapshots(snapshots: &[FleetPaneSnapshot]) -> Self {
        let summary = FleetSummary::from_snapshots(snapshots);
        let status = match summary.aggregate_state() {
            FleetState::Waiting | FleetState::Stopped => "NeedsAttention",
            FleetState::Compacting | FleetState::Active => "Active",
            FleetState::Idle if summary.total_count() > 0 => "Active",
            FleetState::Idle => "Passive",
        };
        Self {
            status,
            title: "Zentty Agent Fleet".to_owned(),
            tooltip: summary.header(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PublicationState {
    Disabled,
    Unavailable,
    Registered,
}

struct Backend {
    connection: gio::DBusConnection,
    watcher: gio::DBusProxy,
    registration: Option<gio::RegistrationId>,
    service_name: String,
    name_owned: bool,
}

pub(crate) struct StatusNotifierItem {
    backend: Option<Backend>,
    presentation: Rc<RefCell<Presentation>>,
    activate: Rc<dyn Fn()>,
    state: PublicationState,
}

impl StatusNotifierItem {
    pub(crate) fn new(activate: Rc<dyn Fn()>) -> Self {
        let presentation = Rc::new(RefCell::new(Presentation::from_snapshots(&[])));
        let backend = create_backend()
            .map_err(|error| {
                eprintln!("zentty-linux: status-notifier capability=unavailable detail={error}");
            })
            .ok();
        Self {
            backend,
            presentation,
            activate,
            state: PublicationState::Disabled,
        }
    }

    pub(crate) fn refresh(&mut self, enabled: bool, snapshots: &[FleetPaneSnapshot]) {
        let next_presentation = Presentation::from_snapshots(snapshots);
        let presentation_changed = *self.presentation.borrow() != next_presentation;
        let status_changed = self.presentation.borrow().status != next_presentation.status;
        self.presentation.replace(next_presentation);

        if !enabled {
            self.unpublish(PublicationState::Disabled, "setting-disabled");
            return;
        }
        let watcher_available = self
            .backend
            .as_ref()
            .and_then(|backend| backend.watcher.name_owner())
            .is_some();
        if !watcher_available {
            self.unpublish(PublicationState::Unavailable, "watcher-unavailable");
            return;
        }
        if self.state != PublicationState::Registered && self.publish().is_err() {
            self.unpublish(PublicationState::Unavailable, "registration-failed");
            return;
        }
        if presentation_changed && self.state == PublicationState::Registered {
            self.emit_update(status_changed);
        }
    }

    fn publish(&mut self) -> Result<(), String> {
        let backend = self
            .backend
            .as_mut()
            .ok_or_else(|| "session D-Bus is unavailable".to_owned())?;
        let node = gio::DBusNodeInfo::for_xml(ITEM_XML)
            .map_err(|error| format!("invalid item interface: {error}"))?;
        let interface = node
            .interfaces()
            .first()
            .ok_or_else(|| "item interface is missing".to_owned())?;
        let presentation = Rc::clone(&self.presentation);
        let activate = Rc::clone(&self.activate);
        let registration = backend
            .connection
            .register_object(ITEM_PATH, interface)
            .method_call(move |_, _, _, _, method, _, invocation| match method {
                "Activate" | "ContextMenu" | "SecondaryActivate" => {
                    activate();
                    invocation.return_value(None);
                }
                "Scroll" => invocation.return_value(None),
                _ => invocation.return_dbus_error(
                    "org.freedesktop.DBus.Error.UnknownMethod",
                    "unsupported status-notifier method",
                ),
            })
            .property(move |_, _, _, _, property| property_value(property, &presentation.borrow()))
            .build()
            .map_err(|error| format!("could not export item: {error}"))?;
        backend.registration = Some(registration);
        request_name(&backend.connection, &backend.service_name)?;
        backend.name_owned = true;
        let parameters = (backend.service_name.as_str(),).to_variant();
        if let Err(error) = backend.watcher.call_sync(
            "RegisterStatusNotifierItem",
            Some(&parameters),
            gio::DBusCallFlags::NONE,
            CALL_TIMEOUT_MS,
            gio::Cancellable::NONE,
        ) {
            if let Some(registration) = backend.registration.take() {
                let _ = backend.connection.unregister_object(registration);
            }
            release_name(&backend.connection, &backend.service_name);
            backend.name_owned = false;
            return Err(format!("watcher registration failed: {error}"));
        }
        self.state = PublicationState::Registered;
        eprintln!(
            "zentty-linux: status-notifier state=registered service={} path={ITEM_PATH}",
            backend.service_name
        );
        Ok(())
    }

    fn unpublish(&mut self, next: PublicationState, reason: &str) {
        if let Some(backend) = self.backend.as_mut()
            && let Some(registration) = backend.registration.take()
        {
            let _ = backend.connection.unregister_object(registration);
        }
        if let Some(backend) = self.backend.as_mut()
            && backend.name_owned
        {
            release_name(&backend.connection, &backend.service_name);
            backend.name_owned = false;
        }
        if self.state != next {
            eprintln!(
                "zentty-linux: status-notifier state={} reason={reason} fallback=in-window",
                match next {
                    PublicationState::Disabled => "disabled",
                    PublicationState::Unavailable => "unavailable",
                    PublicationState::Registered => "registered",
                }
            );
        }
        self.state = next;
    }

    fn emit_update(&self, status_changed: bool) {
        let Some(backend) = self.backend.as_ref() else {
            return;
        };
        if status_changed {
            let status = (self.presentation.borrow().status,).to_variant();
            let _ = backend.connection.emit_signal(
                None,
                ITEM_PATH,
                ITEM_INTERFACE,
                "NewStatus",
                Some(&status),
            );
        }
        let _ = backend
            .connection
            .emit_signal(None, ITEM_PATH, ITEM_INTERFACE, "NewToolTip", None);
        eprintln!(
            "zentty-linux: status-notifier state=updated status={} tooltip={:?}",
            self.presentation.borrow().status,
            self.presentation.borrow().tooltip,
        );
    }
}

impl Drop for StatusNotifierItem {
    fn drop(&mut self) {
        self.unpublish(PublicationState::Disabled, "application-shutdown");
    }
}

fn create_backend() -> Result<Backend, String> {
    let connection = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE)
        .map_err(|error| format!("session bus connection failed: {error}"))?;
    let watcher = gio::DBusProxy::for_bus_sync(
        gio::BusType::Session,
        gio::DBusProxyFlags::DO_NOT_AUTO_START,
        None::<&gio::DBusInterfaceInfo>,
        WATCHER_SERVICE,
        WATCHER_PATH,
        WATCHER_INTERFACE,
        gio::Cancellable::NONE,
    )
    .map_err(|error| format!("watcher proxy failed: {error}"))?;
    Ok(Backend {
        connection,
        watcher,
        registration: None,
        service_name: format!("org.kde.StatusNotifierItem-{}-1", std::process::id()),
        name_owned: false,
    })
}

fn request_name(connection: &gio::DBusConnection, service: &str) -> Result<(), String> {
    let response = connection
        .call_sync(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "RequestName",
            Some(&(service, 0_u32).to_variant()),
            None,
            gio::DBusCallFlags::NONE,
            CALL_TIMEOUT_MS,
            gio::Cancellable::NONE,
        )
        .map_err(|error| format!("could not own item service: {error}"))?;
    match response.get::<(u32,)>() {
        Some((1 | 4,)) => Ok(()),
        Some((reply,)) => Err(format!("item service ownership was rejected: {reply}")),
        None => Err("item service ownership returned an invalid reply".to_owned()),
    }
}

fn release_name(connection: &gio::DBusConnection, service: &str) {
    let _ = connection.call_sync(
        Some("org.freedesktop.DBus"),
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
        "ReleaseName",
        Some(&(service,).to_variant()),
        None,
        gio::DBusCallFlags::NONE,
        CALL_TIMEOUT_MS,
        gio::Cancellable::NONE,
    );
}

fn property_value(property: &str, presentation: &Presentation) -> glib::Variant {
    let empty_pixmaps = Vec::<(i32, i32, Vec<u8>)>::new();
    match property {
        "Category" => "ApplicationStatus".to_variant(),
        "Id" => "zentty-agent-fleet".to_variant(),
        "Title" => presentation.title.to_variant(),
        "Status" => presentation.status.to_variant(),
        "WindowId" => 0_u32.to_variant(),
        "IconName" => "zentty".to_variant(),
        "IconPixmap" | "OverlayIconPixmap" | "AttentionIconPixmap" => empty_pixmaps.to_variant(),
        "AttentionIconName" => "dialog-warning".to_variant(),
        "ToolTip" => (
            "zentty",
            empty_pixmaps,
            presentation.title.as_str(),
            presentation.tooltip.as_str(),
        )
            .to_variant(),
        "ItemIsMenu" => false.to_variant(),
        "Menu" => ObjectPath::try_from("/")
            .expect("root object path is valid")
            .to_variant(),
        _ => "".to_variant(),
    }
}

#[cfg(test)]
mod tests {
    use zentty_core::{AgentProgress, AttentionTarget, FleetPaneSnapshot, FleetState};

    use super::{Presentation, property_value};

    fn snapshot(state: FleetState) -> FleetPaneSnapshot {
        FleetPaneSnapshot {
            target: AttentionTarget::new("window", "worklane", "pane"),
            window_title: "Zentty".to_owned(),
            worklane_title: "Worklane".to_owned(),
            agent_name: "Codex".to_owned(),
            primary_text: "Codex".to_owned(),
            context_text: "Worklane — Zentty".to_owned(),
            status_label: "Running".to_owned(),
            state,
            updated_at_ms: 1,
            progress: Some(AgentProgress { done: 2, total: 5 }),
        }
    }

    #[test]
    fn aggregate_presentation_uses_protocol_status_without_a_second_fleet_model() {
        assert_eq!(Presentation::from_snapshots(&[]).status, "Passive");
        assert_eq!(
            Presentation::from_snapshots(&[snapshot(FleetState::Idle)]).status,
            "Active"
        );
        assert_eq!(
            Presentation::from_snapshots(&[snapshot(FleetState::Active)]).status,
            "Active"
        );
        assert_eq!(
            Presentation::from_snapshots(&[snapshot(FleetState::Waiting)]).status,
            "NeedsAttention"
        );
        assert_eq!(
            Presentation::from_snapshots(&[snapshot(FleetState::Stopped)]).status,
            "NeedsAttention"
        );
    }

    #[test]
    fn required_item_properties_have_the_declared_dbus_types() {
        let presentation = Presentation::from_snapshots(&[snapshot(FleetState::Waiting)]);
        for (name, signature) in [
            ("Category", "s"),
            ("Id", "s"),
            ("Title", "s"),
            ("Status", "s"),
            ("WindowId", "u"),
            ("IconName", "s"),
            ("IconPixmap", "a(iiay)"),
            ("AttentionIconName", "s"),
            ("ToolTip", "(sa(iiay)ss)"),
            ("ItemIsMenu", "b"),
            ("Menu", "o"),
        ] {
            assert_eq!(
                property_value(name, &presentation).type_().as_str(),
                signature
            );
        }
        assert_eq!(
            property_value("Category", &presentation).get::<String>(),
            Some("ApplicationStatus".to_owned())
        );
        assert_eq!(
            property_value("Id", &presentation).get::<String>(),
            Some("zentty-agent-fleet".to_owned())
        );
        assert_eq!(
            property_value("Title", &presentation).get::<String>(),
            Some("Zentty Agent Fleet".to_owned())
        );
        assert_eq!(
            property_value("Status", &presentation).get::<String>(),
            Some("NeedsAttention".to_owned())
        );
        assert_eq!(
            property_value("WindowId", &presentation).get::<u32>(),
            Some(0)
        );
        assert_eq!(
            property_value("IconName", &presentation).get::<String>(),
            Some("zentty".to_owned())
        );
        assert_eq!(
            property_value("AttentionIconName", &presentation).get::<String>(),
            Some("dialog-warning".to_owned())
        );
        assert_eq!(
            property_value("ItemIsMenu", &presentation).get::<bool>(),
            Some(false)
        );
        let tooltip = property_value("ToolTip", &presentation)
            .get::<(String, Vec<(i32, i32, Vec<u8>)>, String, String)>()
            .expect("tooltip has its declared tuple type");
        assert_eq!(tooltip.0, "zentty");
        assert!(tooltip.1.is_empty());
        assert_eq!(tooltip.2, "Zentty Agent Fleet");
        assert_eq!(tooltip.3, "1 waiting");
    }
}
