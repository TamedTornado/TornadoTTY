use super::*;
use zentty_core::{DetectedServer, DetectedServerConfidence, DetectedServerSource};

fn servers() -> Vec<RankedServer> {
    (3000..3007)
        .map(|port| RankedServer {
            server: DetectedServer {
                id: format!("server-{port}"),
                origin: format!("http://localhost:{port}"),
                url: format!("http://localhost:{port}/"),
                display: format!("localhost:{port}"),
                worklane_id: "regulate".into(),
                pane_id: Some("pane".into()),
                source: DetectedServerSource::Scanner,
                ports: vec![port],
                confidence: DetectedServerConfidence::Pid,
                updated_at_ms: 1,
                first_seen_at_ms: 1,
            },
            tier: ServerRelevanceTier::Primary,
            score: 1,
            reasons: Default::default(),
        })
        .collect()
}

#[test]
#[ignore = "requires a controlled GTK display"]
fn server_menu_updates_preserve_the_native_popup_and_route_current_entries() {
    gtk::init().expect("controlled GTK display");
    let chrome = WindowChrome::new();
    let window = gtk::Window::new();
    window.set_child(Some(chrome.widget()));
    let mut entries = servers();
    chrome.configure_servers(&entries, "regulate");
    gtk::prelude::WidgetExt::realize(&window);
    let popup = chrome.server_menu.popover().unwrap();
    gtk::prelude::WidgetExt::realize(&popup);
    let surface = popup.surface().expect("real native popup surface");
    let renderer = popup.renderer().expect("real native popup renderer");
    assert!(renderer.is_realized());

    let activated = std::rc::Rc::new(RefCell::new(Vec::new()));
    let observed = std::rc::Rc::clone(&activated);
    let action = gtk::gio::SimpleAction::new("open-server", Some(gtk::glib::VariantTy::STRING));
    action.connect_activate(move |_, parameter| {
        observed
            .borrow_mut()
            .push(parameter.unwrap().str().unwrap().to_owned());
    });
    let actions = gtk::gio::SimpleActionGroup::new();
    actions.add_action(&action);
    window.insert_action_group("workspace", Some(&actions));

    for iteration in 0..32 {
        let old_list = popup.child().unwrap().downgrade();
        let lane = if iteration % 2 == 0 {
            "bro"
        } else {
            "regulate"
        };
        for entry in &mut entries {
            entry.server.worklane_id = lane.to_owned();
        }
        let origin = format!("http://localhost:{}", 4000 + iteration);
        entries[0].server.origin.clone_from(&origin);
        entries[0].server.url = format!("{origin}/current");
        entries[0].server.display = format!("current server {iteration}");
        entries[1].tier = ServerRelevanceTier::Hidden;
        chrome.configure_servers(&entries, lane);

        assert_eq!(
            chrome.server_menu.popover().unwrap(),
            popup,
            "server churn and lane changes must not replace the native popup"
        );
        assert_eq!(popup.surface().unwrap(), surface);
        assert_eq!(popup.renderer().unwrap(), renderer);
        assert!(renderer.is_realized());
        assert!(
            old_list.upgrade().is_none(),
            "obsolete entries must finalize"
        );
        let list = popup.child().unwrap();
        assert_eq!(
            list.observe_children().n_items(),
            7,
            "heading plus six visible servers"
        );
        let first = list
            .first_child()
            .unwrap()
            .next_sibling()
            .unwrap()
            .downcast::<gtk::Button>()
            .unwrap();
        assert_eq!(
            first.label().as_deref(),
            Some(entries[0].server.display.as_str())
        );
        assert_eq!(
            first.tooltip_text().as_deref(),
            Some(entries[0].server.url.as_str())
        );
        first.emit_clicked();
        assert_eq!(activated.borrow().last(), Some(&origin));

        chrome.configure_servers(&[], "empty");
        assert!(!chrome.server_primary.is_visible());
        assert!(!chrome.server_menu.is_visible());
        chrome.configure_servers(&entries, lane);
        assert_eq!(popup.surface().unwrap(), surface);
        assert_eq!(popup.renderer().unwrap(), renderer);
    }
    window.destroy();
}

#[test]
#[ignore = "requires a controlled GTK display"]
fn blocked_server_menu_content_reports_before_gtk_returns_without_payloads() {
    gtk::init().expect("controlled GTK display");
    let (sender, receiver) = std::sync::mpsc::sync_channel(4);
    let _observer = crate::ui_diagnostics::Observer::for_test(move |message| {
        let _ = sender.try_send(message);
    });
    let chrome = WindowChrome::new();
    let reported = std::rc::Rc::new(std::cell::Cell::new(false));
    let observed = std::rc::Rc::clone(&reported);
    chrome.server_popover.connect_child_notify(move |_| {
        // GTK is inside set_child here. Evidence must arrive from the
        // observer before this deliberately blocked GUI callback can return.
        let message = receiver
            .recv_timeout(std::time::Duration::from_secs(3))
            .unwrap();
        assert!(message.contains("state=stalled"));
        assert!(message.contains("phase=ServerMenuContent"));
        assert!(!message.contains("PRIVATE_DIAGNOSTIC_MARKER"));
        eprintln!("controlled-ui-report {message}");
        observed.set(true);
    });
    let mut servers = servers();
    servers[0].server.display = "PRIVATE_DIAGNOSTIC_MARKER".to_owned();
    servers[0].server.url = "http://PRIVATE_DIAGNOSTIC_MARKER".to_owned();
    chrome.configure_servers(&servers, "regulate");
    assert!(reported.get());
}

#[test]
#[ignore = "requires a controlled GTK display"]
fn server_menu_reuses_unchanged_widgets_and_releases_replaced_widgets() {
    gtk::init().expect("controlled GTK display");
    let _observer = crate::ui_diagnostics::Observer::install().expect("local journal diagnostics");
    let chrome = WindowChrome::new();
    let mut servers = servers();
    chrome.configure_servers(&servers, "regulate");
    let original_popup = chrome.server_menu.popover().unwrap();
    let previous = original_popup.child().unwrap().downgrade();
    servers[0].server.display = "renamed server".into();
    chrome.configure_servers(&servers, "regulate");
    assert!(
        previous.upgrade().is_none(),
        "replaced entries must finalize, not retain their buttons in a cycle"
    );
    let current = chrome.server_menu.popover().unwrap();
    assert_eq!(current, original_popup, "changed entries reuse the popup");
    drop(original_popup);
    for _ in 0..25 {
        servers[0].server.updated_at_ms += 1;
        chrome.configure_servers(&servers, "regulate");
        assert_eq!(
            chrome.server_menu.popover().unwrap(),
            current,
            "unchanged visible entries must not rebuild on metadata refresh"
        );
    }
    chrome.configure_servers(&servers, "other");
    assert!(!chrome.server_menu.is_visible());
    chrome.configure_servers(&servers, "regulate");
    assert_eq!(
        chrome.server_menu.popover().unwrap(),
        current,
        "switching to a lane without servers and back must reuse the menu"
    );
    let list = current.child().unwrap();
    let first = list
        .first_child()
        .unwrap()
        .next_sibling()
        .unwrap()
        .downcast::<gtk::Button>()
        .unwrap();
    assert_eq!(first.label().as_deref(), Some("renamed server"));
    assert_eq!(
        first.action_target_value().unwrap().str(),
        Some("http://localhost:3000")
    );
    let weak_button = first.downgrade();
    drop(first);
    drop(list);
    let weak_menu = current.downgrade();
    drop(current);
    drop(chrome);
    assert!(weak_menu.upgrade().is_none());
    assert!(weak_button.upgrade().is_none());

    let arrange = arrange_panes_popover();
    let weak_arrange = arrange.downgrade();
    drop(arrange);
    assert!(
        weak_arrange.upgrade().is_none(),
        "arrange buttons must not retain their parent menu"
    );

    let chrome = WindowChrome::new();
    let target = zentty_core::OpenWithTarget {
        id: "editor".into(),
        name: "Editor".into(),
        kind: OpenWithTargetKind::Editor,
        launcher: zentty_core::OpenWithLauncher::Executable {
            path: "/bin/true".into(),
            prefix_args: Vec::new(),
        },
    };
    let catalog = OpenWithCatalog {
        enabled: vec![target.clone()],
        primary: Some(target),
        unavailable_ids: Vec::new(),
    };
    chrome.configure_open_with(&catalog);
    let old_open_with = chrome.open_with_menu.popover().unwrap().downgrade();
    chrome.configure_open_with(&OpenWithCatalog::default());
    assert!(
        old_open_with.upgrade().is_none(),
        "Open With buttons must not retain the replaced menu"
    );
}
