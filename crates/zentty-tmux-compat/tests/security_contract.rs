use serde_json::{Map, Value, json};
use zentty_tmux_compat::{ParsedArguments, StoreError, TeamStore};

fn store_json() -> Value {
    json!({"version":1,"buffers":{},"anchors":{},"activePaneIDs":{}})
}

fn repeated_map(count: usize, value: impl Fn(usize) -> Value) -> Value {
    Value::Object(
        (0..count)
            .map(|index| (format!("id-{index}"), value(index)))
            .collect::<Map<_, _>>(),
    )
}

fn decode(value: &Value) -> Result<TeamStore, StoreError> {
    TeamStore::from_json(&serde_json::to_vec(value).unwrap())
}

#[test]
fn parser_accessors_and_non_clusters_are_observable() {
    let parsed = ParsedArguments::parse(
        &["-dPh".to_owned(), "-F".to_owned(), "format".to_owned()],
        &["-F".to_owned()],
        &["-d".to_owned(), "-P".to_owned(), "-h".to_owned()],
    );
    assert_eq!(parsed.value("-F"), Some("format"));
    assert_eq!(parsed.value("-t"), None);
    assert!(parsed.has_flag("-d"));
    assert!(!parsed.has_flag("-v"));

    let plain = ParsedArguments::parse(
        &["abc".to_owned()],
        &[],
        &["-b".to_owned(), "-c".to_owned()],
    );
    assert_eq!(plain.positionals(), ["abc"]);
    assert!(plain.flags().is_empty());

    let long = ParsedArguments::parse(&["--abc".to_owned()], &["--".to_owned()], &[]);
    assert_eq!(long.positionals(), ["--abc"]);
    assert!(long.values().is_empty());
}

#[test]
fn store_size_limits_accept_the_ceiling_and_reject_one_byte_more() {
    assert_eq!(TeamStore::MAX_BUFFER_BYTES, 262_144);
    assert_eq!(TeamStore::MAX_STORE_BYTES, 1_048_576);
    assert_eq!(TeamStore::MAX_IDENTIFIERS, 256);
    assert_eq!(TeamStore::MAX_IDENTIFIER_BYTES, 256);

    let mut exact_store = serde_json::to_vec(&store_json()).unwrap();
    exact_store.resize(TeamStore::MAX_STORE_BYTES, b' ');
    TeamStore::from_json(&exact_store).unwrap();
    exact_store.push(b' ');
    assert_eq!(
        TeamStore::from_json(&exact_store).unwrap_err(),
        StoreError::LimitExceeded
    );

    let mut exact_buffer = store_json();
    exact_buffer["buffers"] = json!({"default":"x".repeat(TeamStore::MAX_BUFFER_BYTES)});
    decode(&exact_buffer).unwrap();
    exact_buffer["buffers"] = json!({"default":"x".repeat(TeamStore::MAX_BUFFER_BYTES + 1)});
    assert_eq!(
        decode(&exact_buffer).unwrap_err(),
        StoreError::LimitExceeded
    );

    let many_valid_buffers = json!({
        "version": 1,
        "anchors": {},
        "activePaneIDs": {},
        "buffers": repeated_map(TeamStore::MAX_IDENTIFIERS, |_| json!("x".repeat(4_080)))
    });
    let compact = serde_json::to_vec(&many_valid_buffers).unwrap();
    assert!(compact.len() <= TeamStore::MAX_STORE_BYTES);
    let store = TeamStore::from_json(&compact).unwrap();
    assert!(matches!(store.to_json(), Err(StoreError::LimitExceeded)));

    let mut exact_encoded = json!({
        "version": 1,
        "anchors": {},
        "activePaneIDs": {},
        "buffers": {
            "a": "x".repeat(TeamStore::MAX_BUFFER_BYTES),
            "b": "x".repeat(TeamStore::MAX_BUFFER_BYTES),
            "c": "x".repeat(TeamStore::MAX_BUFFER_BYTES),
            "d": ""
        }
    });
    let fixed_bytes = serde_json::to_vec_pretty(&exact_encoded).unwrap().len();
    let final_buffer_bytes = TeamStore::MAX_STORE_BYTES - fixed_bytes;
    assert!(final_buffer_bytes <= TeamStore::MAX_BUFFER_BYTES);
    exact_encoded["buffers"]["d"] = json!("x".repeat(final_buffer_bytes));
    let compact = serde_json::to_vec(&exact_encoded).unwrap();
    assert!(compact.len() <= TeamStore::MAX_STORE_BYTES);
    let store = TeamStore::from_json(&compact).unwrap();
    assert_eq!(store.to_json().unwrap().len(), TeamStore::MAX_STORE_BYTES);
}

#[test]
fn each_store_collection_and_team_column_has_an_independent_count_limit() {
    for field in ["anchors", "activePaneIDs", "buffers"] {
        let value = match field {
            "anchors" => repeated_map(
                TeamStore::MAX_IDENTIFIERS,
                |_| json!({"leaderPaneID":"leader","columnPaneIDs":[],"preTeamLeaderWidth":null}),
            ),
            "activePaneIDs" => repeated_map(TeamStore::MAX_IDENTIFIERS, |_| json!("pane")),
            "buffers" => repeated_map(TeamStore::MAX_IDENTIFIERS, |_| json!("value")),
            _ => unreachable!(),
        };
        let mut exact = store_json();
        exact[field] = value;
        decode(&exact).unwrap_or_else(|error| panic!("{field} exact ceiling failed: {error}"));

        let overflow = match field {
            "anchors" => repeated_map(
                TeamStore::MAX_IDENTIFIERS + 1,
                |_| json!({"leaderPaneID":"leader","columnPaneIDs":[],"preTeamLeaderWidth":null}),
            ),
            "activePaneIDs" => repeated_map(TeamStore::MAX_IDENTIFIERS + 1, |_| json!("pane")),
            "buffers" => repeated_map(TeamStore::MAX_IDENTIFIERS + 1, |_| json!("value")),
            _ => unreachable!(),
        };
        let mut over = store_json();
        over[field] = overflow;
        assert_eq!(
            decode(&over).unwrap_err(),
            StoreError::LimitExceeded,
            "{field}"
        );
    }

    let mut exact_column = store_json();
    exact_column["anchors"] = json!({
        "lane": {
            "leaderPaneID":"leader",
            "columnPaneIDs":(0..TeamStore::MAX_IDENTIFIERS).map(|index| format!("pane-{index}")).collect::<Vec<_>>(),
            "preTeamLeaderWidth":800
        }
    });
    decode(&exact_column).unwrap();
    exact_column["anchors"]["lane"]["columnPaneIDs"]
        .as_array_mut()
        .unwrap()
        .push(json!("overflow"));
    assert_eq!(
        decode(&exact_column).unwrap_err(),
        StoreError::LimitExceeded
    );
}

#[test]
fn identifiers_reject_empty_and_overlong_values_at_exact_boundaries() {
    let exact = "x".repeat(TeamStore::MAX_IDENTIFIER_BYTES);
    let over = "x".repeat(TeamStore::MAX_IDENTIFIER_BYTES + 1);
    for name in [exact.as_str(), ""] {
        let mut candidate = store_json();
        candidate["buffers"] =
            Value::Object([(name.to_owned(), json!("value"))].into_iter().collect());
        if name.is_empty() {
            assert_eq!(decode(&candidate).unwrap_err(), StoreError::LimitExceeded);
        } else {
            decode(&candidate).unwrap();
        }
    }
    let mut candidate = store_json();
    candidate["buffers"] = Value::Object([(over, json!("value"))].into_iter().collect());
    assert_eq!(decode(&candidate).unwrap_err(), StoreError::LimitExceeded);
}

#[test]
fn store_errors_are_diagnostic() {
    assert_eq!(
        StoreError::UnsupportedVersion(2).to_string(),
        "unsupported tmux compatibility store version: 2"
    );
    assert_eq!(
        StoreError::LimitExceeded.to_string(),
        "tmux compatibility store limit exceeded"
    );
    assert!(
        TeamStore::from_json(b"not-json")
            .unwrap_err()
            .to_string()
            .starts_with("invalid tmux compatibility store:")
    );
}
