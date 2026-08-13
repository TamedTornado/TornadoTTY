use std::collections::BTreeSet;

use zentty_core::{
    DetectedServer, DetectedServerConfidence, DetectedServerSource, ServerPortRule, ServerRegistry,
    ServerRelevanceContext, ServerRelevanceReason, ServerRelevanceTier,
    ServerTerminationObservation, ServerUrlError, authorize_server_termination, detect_server_urls,
    normalize_server_url, rank_servers,
};

#[test]
fn source_url_normalization_preserves_local_paths_and_rejects_public_or_hostile_targets() {
    let bare = normalize_server_url("3000").unwrap();
    assert_eq!(bare.url, "http://localhost:3000/");
    assert_eq!(bare.origin, "http://localhost:3000");
    assert_eq!(bare.display, "localhost:3000");
    assert_eq!(bare.port, 3000);

    for raw in [
        "localhost:5173",
        "http://0.0.0.0:5173/",
        "http://[::]:5173/",
        "http://[::1]:5173/",
        "http://127.0.0.1:5173/",
    ] {
        assert_eq!(
            normalize_server_url(raw).unwrap().origin,
            "http://localhost:5173"
        );
    }

    let path = normalize_server_url("https://192.168.1.20:4173/docs?q=1#top").unwrap();
    assert_eq!(path.url, "https://192.168.1.20:4173/docs?q=1#top");
    assert_eq!(path.origin, "https://192.168.1.20:4173");

    assert!(normalize_server_url("https://example.com:443").is_err());
    assert!(normalize_server_url("http://localhost").is_err());
    assert!(normalize_server_url("file://localhost:3000/tmp").is_err());
    assert!(normalize_server_url("0").is_err());
    assert!(normalize_server_url("65536").is_err());
    assert_eq!(
        normalize_server_url(":3000"),
        Err(ServerUrlError::MissingPort)
    );
    assert_eq!(
        normalize_server_url("localhost:"),
        Err(ServerUrlError::MissingPort)
    );

    for accepted in [
        "[fc00::1]:3000",
        "[fdff::1]:3000",
        "[fe80::1]:3000",
        "[febf::1]:3000",
    ] {
        assert!(normalize_server_url(accepted).is_ok(), "{accepted}");
    }
    for rejected in ["[f800::1]:3000", "[fec0::1]:3000", "[ffff::1]:3000"] {
        assert!(normalize_server_url(rejected).is_err(), "{rejected}");
    }
}

#[test]
fn output_detection_deduplicates_origins_and_prefers_loopback() {
    let candidates = detect_server_urls(
        "Docs https://example.com:443; Network http://192.168.1.20:5173/, \
         Local: http://localhost:5173/). API http://127.0.0.1:8080/docs?q=1#top",
    );
    assert_eq!(
        candidates
            .iter()
            .map(|candidate| candidate.origin.as_str())
            .collect::<Vec<_>>(),
        [
            "http://localhost:5173",
            "http://192.168.1.20:5173",
            "http://localhost:8080"
        ]
    );
    assert_eq!(
        candidates.last().unwrap().url,
        "http://localhost:8080/docs?q=1#top"
    );
}

#[test]
fn port_rules_normalize_merge_add_remove_and_split() {
    assert_eq!(ServerPortRule::parse("9229").unwrap().canonical(), "9229");
    assert_eq!(
        ServerPortRule::parse(" 24678 - 24680 ")
            .unwrap()
            .canonical(),
        "24678-24680"
    );
    for invalid in ["", "abc", "0", "70000", "3000-", "5000-4000"] {
        assert!(ServerPortRule::parse(invalid).is_none(), "{invalid}");
    }

    let rules = ServerPortRule::normalize(["3000", "3001", "3002-3005", "bad", "8080"]);
    assert_eq!(
        rules
            .iter()
            .map(ServerPortRule::canonical)
            .collect::<Vec<_>>(),
        ["3000-3005", "8080"]
    );
    let rules = ServerPortRule::adding_port(3001, &["3000", "3002"]);
    assert_eq!(rules, ["3000-3002"]);
    let rules = ServerPortRule::removing_port(3001, &["3000-3002"]);
    assert_eq!(rules, ["3000", "3002"]);
    assert_eq!(
        ServerPortRule::removing_port(3000, &["3000-3002"]),
        ["3001-3002"]
    );
    assert_eq!(
        ServerPortRule::removing_port(3002, &["3000-3002"]),
        ["3000-3001"]
    );
}

#[test]
fn relevance_scores_every_source_confidence_focus_running_and_freshness_boundary() {
    let mut manual = server("3000", DetectedServerSource::Manual, Some("pane-1"), 40_000);
    manual.confidence = DetectedServerConfidence::Explicit;
    let mut watch = server("3001", DetectedServerSource::Watch, Some("pane-2"), 39_999);
    watch.confidence = DetectedServerConfidence::Pid;
    let mut docker = server("3002", DetectedServerSource::Docker, None, 0);
    docker.confidence = DetectedServerConfidence::Cwd;
    let mut scanner = server("3003", DetectedServerSource::Scanner, None, 0);
    scanner.confidence = DetectedServerConfidence::Worklane;
    let ranked = rank_servers(
        vec![scanner, docker, watch, manual],
        &ServerRelevanceContext {
            focused_pane_id: Some("pane-1".into()),
            running_pane_ids: BTreeSet::from(["pane-1".into()]),
            now_ms: 100_000,
            ..ServerRelevanceContext::default()
        },
    );
    let by_origin = ranked
        .iter()
        .map(|entry| (entry.server.origin.as_str(), entry))
        .collect::<std::collections::BTreeMap<_, _>>();
    let manual = by_origin["http://localhost:3000"];
    assert_eq!(manual.score, 465);
    assert!(manual.reasons.contains(&ServerRelevanceReason::FocusedPane));
    assert!(manual.reasons.contains(&ServerRelevanceReason::RunningPane));
    assert!(manual.reasons.contains(&ServerRelevanceReason::Manual));
    assert!(manual.reasons.contains(&ServerRelevanceReason::Fresh));
    assert_eq!(by_origin["http://localhost:3001"].score, 80);
    assert_eq!(by_origin["http://localhost:3002"].score, 50);
    assert_eq!(by_origin["http://localhost:3003"].score, 0);
    assert!(
        !by_origin["http://localhost:3003"]
            .reasons
            .contains(&ServerRelevanceReason::Manual)
    );

    let no_pane = rank_servers(
        vec![server("4000", DetectedServerSource::Scanner, None, 0)],
        &ServerRelevanceContext {
            focused_pane_id: None,
            now_ms: 100_000,
            ..ServerRelevanceContext::default()
        },
    );
    assert!(
        !no_pane[0]
            .reasons
            .contains(&ServerRelevanceReason::FocusedPane)
    );
}

#[test]
fn relevance_has_one_primary_and_never_promotes_ignored_scanner_servers() {
    let servers = vec![
        server("3000", DetectedServerSource::Watch, Some("pane-2"), 9_900),
        server("5173", DetectedServerSource::Scanner, Some("pane-1"), 9_990),
        server("9229", DetectedServerSource::Scanner, Some("pane-1"), 9_999),
    ];
    let ranked = rank_servers(
        servers,
        &ServerRelevanceContext {
            focused_pane_id: Some("pane-1".into()),
            running_pane_ids: BTreeSet::from(["pane-1".into()]),
            ignored_port_rules: ServerPortRule::normalize(["9229"]),
            selected_origin: None,
            now_ms: 10_000,
        },
    );
    assert_eq!(
        ranked
            .iter()
            .find(|entry| entry.tier == ServerRelevanceTier::Primary)
            .unwrap()
            .server
            .origin,
        "http://localhost:5173"
    );
    assert_eq!(
        ranked
            .iter()
            .filter(|entry| entry.tier == ServerRelevanceTier::Primary)
            .count(),
        1
    );
    let hidden = ranked
        .iter()
        .find(|entry| entry.server.origin == "http://localhost:9229")
        .unwrap();
    assert_eq!(hidden.tier, ServerRelevanceTier::Hidden);
    assert!(
        hidden
            .reasons
            .contains(&ServerRelevanceReason::IgnoredPort(9229))
    );
}

#[test]
fn selected_origin_and_deterministic_ties_follow_source_weights() {
    let selected = server("5173", DetectedServerSource::Scanner, None, 0);
    let manual = server("3000", DetectedServerSource::Manual, None, 0);
    let context = ServerRelevanceContext {
        selected_origin: Some(selected.origin.clone()),
        now_ms: 100_000,
        ..ServerRelevanceContext::default()
    };
    assert_eq!(
        rank_servers(vec![manual, selected], &context)[0]
            .server
            .origin,
        "http://localhost:5173"
    );

    let tied = rank_servers(
        vec![
            server("5173", DetectedServerSource::Scanner, None, 0),
            server("3000", DetectedServerSource::Scanner, None, 0),
        ],
        &ServerRelevanceContext {
            now_ms: 100_000,
            ..ServerRelevanceContext::default()
        },
    );
    assert_eq!(tied[0].server.origin, "http://localhost:3000");
}

#[test]
fn registry_merges_sources_by_precedence_and_preserves_first_seen_across_refresh() {
    let mut registry = ServerRegistry::default();
    let mut scanner = server("5173", DetectedServerSource::Scanner, Some("pane-1"), 100);
    scanner.updated_at_ms = 500;
    registry.replace_source(
        DetectedServerSource::Scanner,
        "worklane-1",
        vec![scanner.clone()],
    );

    let mut watch = server("5173", DetectedServerSource::Watch, Some("pane-2"), 200);
    watch.url = "http://localhost:5173/dashboard".into();
    watch.updated_at_ms = 400;
    registry.upsert(watch);

    let merged = registry.servers_in("worklane-1");
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].source, DetectedServerSource::Watch);
    assert_eq!(merged[0].pane_id.as_deref(), Some("pane-2"));
    assert_eq!(merged[0].url, "http://localhost:5173/dashboard");
    assert_eq!(merged[0].first_seen_at_ms, 100);

    scanner.updated_at_ms = 900;
    scanner.first_seen_at_ms = 900;
    registry.replace_source(DetectedServerSource::Scanner, "worklane-1", vec![scanner]);
    assert_eq!(registry.servers_in("worklane-1")[0].first_seen_at_ms, 100);

    registry.replace_source(DetectedServerSource::Watch, "worklane-1", vec![]);
    assert_eq!(
        registry.servers_in("worklane-1")[0].source,
        DetectedServerSource::Scanner
    );
    registry.replace_source(DetectedServerSource::Scanner, "worklane-1", vec![]);
    assert!(registry.servers_in("worklane-1").is_empty());
}

#[test]
fn registry_clears_only_the_authenticated_pane_and_optional_source() {
    let mut registry = ServerRegistry::default();
    registry.upsert(server(
        "3000",
        DetectedServerSource::Manual,
        Some("pane-1"),
        1,
    ));
    registry.upsert(server(
        "4000",
        DetectedServerSource::Watch,
        Some("pane-1"),
        1,
    ));
    registry.upsert(server(
        "5000",
        DetectedServerSource::Manual,
        Some("pane-2"),
        1,
    ));

    registry.clear_pane("worklane-1", "pane-1", Some(DetectedServerSource::Watch));
    let origins = registry
        .servers_in("worklane-1")
        .into_iter()
        .map(|server| server.origin)
        .collect::<Vec<_>>();
    assert_eq!(origins, ["http://localhost:3000", "http://localhost:5000"]);

    registry.clear_pane("worklane-1", "pane-1", None);
    assert_eq!(registry.servers_in("worklane-1").len(), 1);
    assert_eq!(
        registry.servers_in("worklane-1")[0].pane_id.as_deref(),
        Some("pane-2")
    );
}

#[test]
fn disabling_passive_detection_removes_only_scanner_and_docker_sources() {
    let mut registry = ServerRegistry::default();
    for (port, source) in [
        ("3000", DetectedServerSource::Manual),
        ("4000", DetectedServerSource::Watch),
        ("5000", DetectedServerSource::Docker),
        ("6000", DetectedServerSource::Scanner),
    ] {
        registry.upsert(server(port, source, Some("pane-1"), 1));
    }

    let removed =
        registry.remove_sources(&[DetectedServerSource::Docker, DetectedServerSource::Scanner]);
    assert_eq!(removed, 2);
    let remaining = registry
        .servers_in("worklane-1")
        .into_iter()
        .map(|server| server.source)
        .collect::<Vec<_>>();
    assert_eq!(
        remaining,
        [DetectedServerSource::Manual, DetectedServerSource::Watch]
    );
}

#[test]
fn termination_requires_a_current_pid_owned_scanner_listener_below_the_pane_shell() {
    let scanner = server("5173", DetectedServerSource::Scanner, Some("pane-1"), 100);
    let observation = ServerTerminationObservation {
        pane_id: Some("pane-1".into()),
        listener_pid: 220,
        listener_start_time: 900,
        port: 5173,
        owned_by_pane: true,
    };
    let target = authorize_server_termination(&scanner, "pane-1", 100, &observation).unwrap();
    assert_eq!(target.pid, 220);
    assert_eq!(target.start_time, 900);

    let mut rejected = observation.clone();
    rejected.owned_by_pane = false;
    assert!(authorize_server_termination(&scanner, "pane-1", 100, &rejected).is_none());
    rejected = observation.clone();
    rejected.listener_pid = 1;
    assert!(authorize_server_termination(&scanner, "pane-1", 100, &rejected).is_none());
    assert!(authorize_server_termination(&scanner, "pane-1", 1, &observation).is_none());
    rejected = observation.clone();
    rejected.port = 3000;
    assert!(authorize_server_termination(&scanner, "pane-1", 100, &rejected).is_none());

    let mut cwd = scanner.clone();
    cwd.confidence = DetectedServerConfidence::Cwd;
    assert!(authorize_server_termination(&cwd, "pane-1", 100, &observation).is_none());
    let mut manual = scanner;
    manual.source = DetectedServerSource::Manual;
    assert!(authorize_server_termination(&manual, "pane-1", 100, &observation).is_none());
}

fn server(
    port: &str,
    source: DetectedServerSource,
    pane_id: Option<&str>,
    first_seen_ms: u64,
) -> DetectedServer {
    let candidate = normalize_server_url(port).unwrap();
    DetectedServer {
        id: format!("wl|{}", candidate.origin),
        origin: candidate.origin,
        url: candidate.url,
        display: candidate.display,
        worklane_id: "worklane-1".into(),
        pane_id: pane_id.map(str::to_owned),
        source,
        ports: vec![candidate.port],
        confidence: DetectedServerConfidence::Pid,
        updated_at_ms: first_seen_ms,
        first_seen_at_ms: first_seen_ms,
    }
}
