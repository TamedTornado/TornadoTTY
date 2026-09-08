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
fn server_menu_reuses_unchanged_widgets_and_releases_replaced_widgets() {
    gtk::init().expect("controlled GTK display");
    let chrome = WindowChrome::new();
    let mut servers = servers();
    chrome.configure_servers(&servers, "regulate");
    let previous = chrome.server_menu.popover().unwrap().downgrade();
    servers[0].server.display = "renamed server".into();
    chrome.configure_servers(&servers, "regulate");
    assert!(
        previous.upgrade().is_none(),
        "replaced menu must finalize, not retain its buttons in a cycle"
    );
    let current = chrome.server_menu.popover().unwrap();
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
