use zentty_core::{Workspace, WorkspaceError};

const VALID_V1: &[u8] =
    include_bytes!("../../../docs/architecture/fixtures/workspace-state/valid-v1.json");
const V0: &[u8] =
    include_bytes!("../../../docs/architecture/fixtures/workspace-state/old-version-v0.json");
const UNKNOWN_FIELD: &[u8] =
    include_bytes!("../../../docs/architecture/fixtures/workspace-state/unknown-field-v1.json");
const MALFORMED: &[u8] =
    include_bytes!("../../../docs/architecture/fixtures/workspace-state/malformed-v1.json");
type JsonMutation = (&'static str, fn(&mut serde_json::Value));

#[test]
fn v1_fixture_round_trips_semantically_and_preserves_durable_metadata() {
    let workspace = Workspace::from_json(VALID_V1).expect("v1 fixture must load");
    let lane = &workspace.windows()[0].worklanes()[0];
    let pane = &lane.panes()[0];

    assert_eq!(workspace.revision(), 7);
    assert_eq!(lane.title(), Some("API"));
    assert_eq!(pane.title(), Some("server"));
    assert_eq!(pane.cwd().to_str(), Some("/home/user/Development/zentty"));
    assert_eq!(pane.launch_profile_id(), "default-shell");
    assert_eq!(pane.agent().expect("agent metadata").adapter(), "codex");

    let encoded = workspace.to_json().expect("workspace must encode");
    assert_eq!(Workspace::from_json(&encoded), Ok(workspace));
    let text = String::from_utf8(encoded).unwrap();
    for forbidden in ["environment", "clipboard", "transcript", "credential"] {
        assert!(!text.contains(forbidden));
    }
}

#[test]
fn v0_fixture_migrates_to_canonical_v1_idempotently() {
    let migrated = Workspace::from_json(V0).expect("v0 fixture must migrate");
    assert_eq!(migrated.revision(), 0);
    let encoded = migrated.to_json().expect("migrated workspace must encode");
    assert_eq!(Workspace::from_json(&encoded), Ok(migrated));
    assert!(
        String::from_utf8(encoded)
            .unwrap()
            .contains("\"schema_version\": 1")
    );
}

#[test]
fn malformed_unknown_and_newer_documents_are_rejected() {
    assert!(matches!(
        Workspace::from_json(MALFORMED),
        Err(WorkspaceError::InvalidPersistedState(_))
    ));
    assert!(matches!(
        Workspace::from_json(UNKNOWN_FIELD),
        Err(WorkspaceError::InvalidPersistedState(_))
    ));
    assert_eq!(
        Workspace::from_json(br#"{"schema_version":2}"#),
        Err(WorkspaceError::UnsupportedSchemaVersion(2))
    );
}

#[test]
fn dangling_selection_and_noncontiguous_order_are_rejected() {
    let valid = String::from_utf8(VALID_V1.to_vec()).unwrap();
    let dangling = valid.replacen(
        "5b667f15-a90d-4624-bbce-22b82f58b63e",
        "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
        1,
    );
    assert!(matches!(
        Workspace::from_json(dangling.as_bytes()),
        Err(WorkspaceError::InvalidPersistedState(_))
    ));

    let bad_order = valid.replacen("\"order\": 0", "\"order\": 1", 1);
    assert!(matches!(
        Workspace::from_json(bad_order.as_bytes()),
        Err(WorkspaceError::InvalidPersistedState(_))
    ));
}

#[test]
fn semantic_layout_identity_and_confidentiality_violations_are_rejected() {
    let cases: &[JsonMutation] = &[
        ("duplicate ID", |root| {
            root["windows"][0]["worklanes"][0]["panes"][1]["id"] =
                root["windows"][0]["worklanes"][0]["panes"][0]["id"].clone();
        }),
        ("dangling active pane", |root| {
            root["windows"][0]["worklanes"][0]["active_pane_id"] =
                serde_json::Value::String("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".into());
        }),
        ("unknown pane column", |root| {
            root["windows"][0]["worklanes"][0]["panes"][0]["layout"]["column"] =
                serde_json::Value::from(2);
        }),
        ("noncontiguous pane row", |root| {
            root["windows"][0]["worklanes"][0]["panes"][1]["layout"]["row"] =
                serde_json::Value::from(3);
        }),
        ("zero column weight", |root| {
            root["windows"][0]["worklanes"][0]["layout"]["columns"][0]["weight"] =
                serde_json::Value::from(0);
        }),
        ("secret-bearing unknown field", |root| {
            root["windows"][0]["worklanes"][0]["panes"][0]["environment"] =
                serde_json::json!({"TOKEN": "must-not-load"});
        }),
    ];

    for (label, mutate) in cases {
        let mut root: serde_json::Value = serde_json::from_slice(VALID_V1).unwrap();
        mutate(&mut root);
        assert!(
            matches!(
                Workspace::from_json(&serde_json::to_vec(&root).unwrap()),
                Err(WorkspaceError::InvalidPersistedState(_) | WorkspaceError::DuplicateId(_))
            ),
            "case unexpectedly loaded: {label}"
        );
    }
}
