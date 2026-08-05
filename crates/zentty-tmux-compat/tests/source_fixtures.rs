use serde_json::Value;
use zentty_tmux_compat::{
    Command, FormatRenderer, Invocation, PaneTarget, ParsedArguments, SendKeys, StoreError,
    TeamStore, TeamTransition,
};

fn fixtures() -> Vec<Value> {
    let document: Value = serde_json::from_str(include_str!(
        "../../../linux/tests/fixtures/tmux-compat-source-v1.json"
    ))
    .expect("source fixture JSON must parse");
    document["fixtures"]
        .as_array()
        .expect("fixtures must be an array")
        .clone()
}

#[test]
fn source_argument_parser_fixtures_cover_clusters_values_and_positionals() {
    for case in fixtures()
        .into_iter()
        .filter(|case| case["kind"] == "arguments")
    {
        let parsed = ParsedArguments::parse(
            &string_array(&case["arguments"]),
            &string_array(&case["value_options"]),
            &string_array(&case["boolean_options"]),
        );
        let expected = &case["expected"];
        let expected_values = expected["values"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(key, value)| (key.clone(), value.as_str().unwrap().to_owned()))
            .collect();
        assert_eq!(parsed.values(), &expected_values, "{}", case["id"]);
        assert_eq!(
            parsed.flags(),
            string_array(&expected["flags"]),
            "{}",
            case["id"]
        );
        assert_eq!(
            parsed.positionals(),
            string_array(&expected["positionals"]),
            "{}",
            case["id"]
        );
    }
}

fn fixture(id: &str) -> Value {
    fixtures()
        .into_iter()
        .find(|fixture| fixture["id"] == id)
        .unwrap_or_else(|| panic!("missing fixture: {id}"))
}

fn string_array(value: &Value) -> Vec<String> {
    value
        .as_array()
        .expect("fixture value must be an array")
        .iter()
        .map(|value| {
            value
                .as_str()
                .expect("array value must be a string")
                .to_owned()
        })
        .collect()
}

#[test]
fn source_invocation_global_options_are_stripped_without_losing_subcommand_options() {
    let case = fixture("parse.split-window-global-socket");
    let invocation = Invocation::parse(&string_array(&case["argv"])).expect("valid invocation");
    assert_eq!(invocation.command, Command::SplitWindow);
    assert_eq!(
        invocation.arguments,
        string_array(&case["expected"]["arguments"])
    );
}

#[test]
fn missing_global_option_value_and_unknown_command_fail_explicitly() {
    let missing = Invocation::parse(&["-S".to_owned()]).unwrap_err();
    assert_eq!(
        missing.to_string(),
        "tmux global option requires a value: -S"
    );
    let unknown = Invocation::parse(&["definitely-not-tmux".to_owned()]).unwrap_err();
    assert_eq!(
        unknown.to_string(),
        "unsupported tmux compatibility command: definitely-not-tmux"
    );
    assert_eq!(
        Invocation::parse(&[]).unwrap_err().to_string(),
        "tmux compatibility command is required"
    );
}

#[test]
fn every_source_global_boolean_preserves_the_following_command() {
    let contract: Value = serde_json::from_str(include_str!(
        "../../../docs/design/zentty-tmux-compat-source-contract-v1.json"
    ))
    .unwrap();
    for option in contract["cli"]["global_boolean_options"]
        .as_array()
        .unwrap()
    {
        let invocation = Invocation::parse(&[
            option.as_str().unwrap().to_owned(),
            "list-panes".to_owned(),
            "-F".to_owned(),
            "#{pane_id}".to_owned(),
        ])
        .unwrap();
        assert_eq!(invocation.command, Command::ListPanes, "{option}");
        assert_eq!(invocation.arguments, ["-F", "#{pane_id}"], "{option}");
    }
}

#[test]
fn source_send_key_fixtures_match_named_and_literal_translation() {
    for id in ["send.named-keys", "send.literal"] {
        let case = fixture(id);
        let actual = SendKeys::translate(
            &string_array(&case["arguments"]),
            case["standard_input"].as_str(),
        );
        assert_eq!(actual, case["expected_text"].as_str().unwrap(), "{id}");
    }
}

#[test]
fn every_source_named_key_translation_is_observable() {
    for (name, expected) in [
        ("Enter", "\r"),
        ("C-m", "\r"),
        ("KPEnter", "\r"),
        ("Tab", "\t"),
        ("C-i", "\t"),
        ("Space", " "),
        ("BSpace", "\u{7f}"),
        ("Backspace", "\u{7f}"),
        ("Escape", "\u{1b}"),
        ("Esc", "\u{1b}"),
        ("C-[", "\u{1b}"),
        ("C-c", "\u{03}"),
        ("C-d", "\u{04}"),
        ("C-z", "\u{1a}"),
        ("C-l", "\u{0c}"),
    ] {
        assert_eq!(
            SendKeys::translate(&[name.to_owned()], None),
            expected,
            "{name}"
        );
    }
    assert_eq!(SendKeys::translate(&[], Some("stdin")), "stdin");
}

#[test]
fn source_format_fixtures_match_renderer_contract() {
    for case in fixtures()
        .into_iter()
        .filter(|case| case["kind"] == "format")
    {
        let context = case["context"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(key, value)| (key.clone(), value.as_str().unwrap().to_owned()))
            .collect();
        assert_eq!(
            FormatRenderer::render(case["template"].as_str().unwrap(), &context),
            case["expected"].as_str().unwrap(),
            "{}",
            case["id"]
        );
    }
}

#[test]
fn explicit_pane_target_is_scoped_to_available_panes() {
    let case = fixture("target.explicit-pane");
    let available = string_array(&case["available"]);
    assert_eq!(
        PaneTarget::resolve(
            case["selector"].as_str(),
            &available,
            case["fallback"].as_str().unwrap()
        ),
        case["expected"].as_str().unwrap()
    );
    assert_eq!(
        PaneTarget::resolve(Some("%foreign"), &available, "pane-1"),
        "pane-1"
    );
}

#[test]
fn source_team_store_transitions_preserve_anchor_and_restore_width() {
    let mut store = TeamStore::default();
    let first = store.record_split("lane-1", "pane-1", "pane-2", false, Some(800));
    assert_eq!(first, TeamTransition::FirstSplit);
    let second = store.record_split("lane-1", "pane-1", "pane-3", true, None);
    assert_eq!(second, TeamTransition::StackedSplit);
    let anchor = store.anchor("lane-1").unwrap();
    assert_eq!(anchor.leader_pane_id, "pane-1");
    assert_eq!(anchor.column_pane_ids, ["pane-2", "pane-3"]);
    assert_eq!(anchor.pre_team_leader_width, Some(800));
    assert_eq!(store.active_pane("lane-1"), Some("pane-2"));

    assert_eq!(store.remove_pane("lane-1", "pane-2"), None);
    assert_eq!(store.active_pane("lane-1"), None);
    assert_eq!(store.anchor("lane-1").unwrap().column_pane_ids, ["pane-3"]);
    assert_eq!(store.remove_pane("lane-1", "pane-3"), Some(800));
    assert!(store.anchor("lane-1").is_none());
    assert_eq!(store.active_pane("lane-1"), None);
}

#[test]
fn source_team_store_records_compatibility_selection_without_a_split() {
    let mut store = TeamStore::default();
    store.record_active_pane("lane-1", "pane-2");
    assert_eq!(store.active_pane("lane-1"), Some("pane-2"));
}

#[test]
fn team_store_schema_is_versioned_bounded_and_source_named() {
    let store = TeamStore::default();
    let encoded = store.to_json().unwrap();
    let value: Value = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(value["version"], 1);
    assert!(value.get("activePaneIDs").is_some());
    assert!(value.get("active_pane_ids").is_none());
    assert_eq!(TeamStore::from_json(&encoded).unwrap(), store);

    let future = br#"{"version":2,"buffers":{},"anchors":{},"activePaneIDs":{}}"#;
    assert_eq!(
        TeamStore::from_json(future).unwrap_err(),
        StoreError::UnsupportedVersion(2)
    );

    let oversized = format!(
        "{{\"version\":1,\"buffers\":{{\"default\":\"{}\"}},\"anchors\":{{}},\"activePaneIDs\":{{}}}}",
        "x".repeat(TeamStore::MAX_BUFFER_BYTES + 1)
    );
    assert_eq!(
        TeamStore::from_json(oversized.as_bytes()).unwrap_err(),
        StoreError::LimitExceeded
    );
}

#[test]
fn every_source_command_and_alias_canonicalizes() {
    let contract: Value = serde_json::from_str(include_str!(
        "../../../docs/design/zentty-tmux-compat-source-contract-v1.json"
    ))
    .unwrap();
    for command in contract["commands"].as_array().unwrap() {
        let expected = Command::parse(command["name"].as_str().unwrap()).unwrap();
        assert_eq!(expected.as_str(), command["name"].as_str().unwrap());
        for word in std::iter::once(&command["name"]).chain(command["aliases"].as_array().unwrap())
        {
            assert_eq!(Command::parse(word.as_str().unwrap()).unwrap(), expected);
        }
    }
}
