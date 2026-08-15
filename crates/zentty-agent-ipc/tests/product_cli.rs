use zentty_agent_ipc::{
    CliProductCommand, ProductIpcKind, ProductIpcReply, ProductIpcRequest, parse_product_cli,
};

fn values(arguments: &[&str]) -> Vec<String> {
    arguments.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn source_discovery_commands_and_aliases_map_to_one_route() {
    let cases = [
        (
            values(&["list", "windows", "--json"]),
            "windows",
            vec!["--json"],
        ),
        (
            values(&["window", "list", "--json"]),
            "windows",
            vec!["--json"],
        ),
        (
            values(&["list", "worklanes", "--window-id", "window-2"]),
            "worklanes",
            vec!["--window-id", "window-2"],
        ),
        (
            values(&["worklane", "list", "--window-id", "window-2"]),
            "worklanes",
            vec!["--window-id", "window-2"],
        ),
        (
            values(&["list", "panes", "--worklane-id", "lane-2"]),
            "panes",
            vec!["--worklane-id", "lane-2"],
        ),
        (
            values(&["pane", "list", "--worklane-id", "lane-2"]),
            "panes-current-worklane",
            vec!["--worklane-id", "lane-2"],
        ),
    ];

    for (arguments, expected_subcommand, expected_arguments) in cases {
        let parsed = parse_product_cli(&arguments).unwrap().unwrap();
        let CliProductCommand::Request(request) = parsed else {
            panic!("expected product request");
        };
        assert_eq!(request.kind(), ProductIpcKind::Discover);
        assert_eq!(request.subcommand(), expected_subcommand);
        assert_eq!(request.arguments(), expected_arguments);
    }
}

#[test]
fn integration_management_targets_are_local_validated_commands() {
    for target in [
        "amp-hooks",
        "cursor-hooks",
        "droid-hooks",
        "kimi-hooks",
        "grok-hooks",
        "agy-hooks",
        "hermes-hooks",
        "vibe-hooks",
    ] {
        assert_eq!(
            parse_product_cli(&values(&["install", target])).unwrap(),
            Some(CliProductCommand::InstallIntegration(target.to_owned()))
        );
        assert_eq!(
            parse_product_cli(&values(&["uninstall", target])).unwrap(),
            Some(CliProductCommand::UninstallIntegration(target.to_owned()))
        );
    }
    assert!(parse_product_cli(&values(&["install", "unknown-hooks"])).is_err());
    assert!(parse_product_cli(&values(&["uninstall"])).is_err());
    assert!(parse_product_cli(&values(&["install", "cursor-hooks", "extra"])).is_err());
}

#[test]
fn source_mutation_commands_preserve_canonical_vocabulary() {
    let cases = [
        (
            values(&["hsplit", "--equal"]),
            "split",
            vec!["right", "--equal"],
        ),
        (
            values(&["vsplit", "--golden"]),
            "split",
            vec!["down", "--golden"],
        ),
        (
            values(&["split", "left", "--ratio", "60"]),
            "split",
            vec!["left", "--ratio", "60"],
        ),
        (values(&["pane", "focus", "right"]), "focus", vec!["right"]),
        (
            values(&["pane", "rename", "Build logs", "--pane-id", "pane-2"]),
            "pane-rename",
            vec!["--title", "Build logs", "--rename-pane-id", "pane-2"],
        ),
        (
            values(&["worklane", "rename", "Backend", "--id", "lane-2"]),
            "worklane-rename",
            vec!["--title", "Backend", "--id", "lane-2"],
        ),
        (
            values(&["layout", "thirds", "--vertical"]),
            "layout",
            vec!["thirds", "--vertical"],
        ),
    ];

    for (arguments, expected_subcommand, expected_arguments) in cases {
        let parsed = parse_product_cli(&arguments).unwrap().unwrap();
        let CliProductCommand::Request(request) = parsed else {
            panic!("expected product request");
        };
        assert_eq!(request.kind(), ProductIpcKind::Pane);
        assert_eq!(request.subcommand(), expected_subcommand);
        assert_eq!(request.arguments(), expected_arguments);
    }
}

#[test]
fn pane_notification_preserves_source_text_and_delivery_flags() {
    let parsed = parse_product_cli(&values(&[
        "notify",
        "--title",
        " Agent ready ",
        "--subtitle",
        "Review it",
        "--body",
        "Line one\nline two",
        "--no-inbox",
        "--silent",
    ]))
    .unwrap()
    .unwrap();
    let CliProductCommand::Request(request) = parsed else {
        panic!("expected product request");
    };
    assert_eq!(request.kind(), ProductIpcKind::Pane);
    assert_eq!(request.subcommand(), "notify");
    assert_eq!(
        request.arguments(),
        &values(&[
            "--title",
            "Agent ready",
            "--subtitle",
            "Review it",
            "--body",
            "Line one\nline two",
            "--no-inbox",
            "--silent",
        ])
    );
    for invalid in [
        vec!["notify"],
        vec!["notify", "--title", "   "],
        vec!["notify", "--title", "ready", "--unknown"],
    ] {
        assert!(parse_product_cli(&values(&invalid)).is_err(), "{invalid:?}");
    }
}

#[test]
fn grid_parser_bounds_dimensions_and_preserves_command_argv() {
    let parsed = parse_product_cli(&values(&[
        "grid",
        "2x3",
        "--new-only",
        "--focus",
        "last",
        "--worklane-id",
        "new",
        "--",
        "codex",
        "--model",
        "gpt-5",
    ]))
    .unwrap()
    .unwrap();
    let CliProductCommand::Request(request) = parsed else {
        panic!("expected product request");
    };
    assert_eq!(request.kind(), ProductIpcKind::Pane);
    assert_eq!(request.subcommand(), "grid");
    assert_eq!(
        request.arguments(),
        &values(&[
            "--rows",
            "2",
            "--columns",
            "3",
            "--new-only",
            "--focus",
            "last",
            "--new-worklane",
            "--command-json",
            r#"["codex","--model","gpt-5"]"#,
        ])
    );

    for invalid in [
        values(&["grid", "0x2"]),
        values(&["grid", "7x6"]),
        values(&["grid", "2-by-2"]),
        values(&["grid", "2x2", "--focus", "somewhere"]),
        values(&[
            "grid",
            "2x2",
            "--window-id",
            "new",
            "--worklane-id",
            "lane-1",
        ]),
        values(&["grid", "2x2", "--new-only", "--include-source"]),
    ] {
        assert!(parse_product_cli(&invalid).is_err(), "accepted {invalid:?}");
    }
}

#[test]
fn grid_parser_preserves_valid_boundaries_and_destinations() {
    let boundary = parse_product_cli(&values(&["grid", "6x6", "--focus", "source"]))
        .unwrap()
        .unwrap();
    let CliProductCommand::Request(boundary) = boundary else {
        panic!("expected boundary grid request");
    };
    assert_eq!(
        boundary.arguments(),
        &values(&["--rows", "6", "--columns", "6", "--focus", "source"])
    );

    let new_window = parse_product_cli(&values(&["grid", "1x1", "--window-id", "new", "--json"]))
        .unwrap()
        .unwrap();
    let CliProductCommand::Request(new_window) = new_window else {
        panic!("expected new-window grid request");
    };
    assert_eq!(
        new_window.arguments(),
        &values(&[
            "--rows",
            "1",
            "--columns",
            "1",
            "--focus",
            "source",
            "--new-window",
            "--json",
        ])
    );

    let existing_destination = parse_product_cli(&values(&[
        "grid",
        "1x1",
        "--window-id",
        "window-2",
        "--worklane-id",
        "worklane-3",
    ]))
    .unwrap()
    .unwrap();
    let CliProductCommand::Request(existing_destination) = existing_destination else {
        panic!("expected existing-destination grid request");
    };
    assert!(
        !existing_destination
            .arguments()
            .iter()
            .any(|argument| argument == "--new-only")
    );
    assert!(
        parse_product_cli(&values(&["grid", "1x1", "--include-source"]))
            .unwrap()
            .is_some()
    );
}

#[test]
fn topology_mutations_preserve_explicit_machine_output_request() {
    let cases = [
        values(&["split", "left", "--ratio", "60", "--json"]),
        values(&["hsplit", "--equal", "--json"]),
        values(&["vsplit", "--golden", "--json"]),
        values(&["grid", "2x3", "--focus", "last", "--json"]),
        values(&["layout", "thirds", "--vertical", "--json"]),
        values(&["pane", "resize", "60%", "--json"]),
        values(&["pane", "zoom", "--json"]),
    ];

    for arguments in cases {
        let parsed = parse_product_cli(&arguments).unwrap().unwrap();
        let CliProductCommand::Request(request) = parsed else {
            panic!("expected product request for {arguments:?}");
        };
        assert!(
            request
                .arguments()
                .iter()
                .any(|argument| argument == "--json"),
            "machine-output flag disappeared from {arguments:?}"
        );
    }
}

#[test]
fn grid_rejects_commands_with_line_breaks_before_contacting_the_product() {
    for line_break in ["line one\nline two", "line one\rline two"] {
        assert!(
            parse_product_cli(&values(&["grid", "1x2", "--", "printf", line_break])).is_err(),
            "accepted a command token containing a line break"
        );
    }
}

#[test]
fn parser_rejects_ambiguous_and_invalid_source_invocations() {
    for invalid in [
        values(&["split", "diagonal"]),
        values(&["split", "right", "--equal", "--golden"]),
        values(&["split", "right", "--ratio", "0"]),
        values(&["pane", "focus", "2", "--pane-id", "pane-2"]),
        values(&["pane", "rename", "Title", "--clear"]),
        values(&["worklane", "color", "ultraviolet"]),
        values(&["layout", "masonry"]),
        values(&["list", "panes", "--json", "--json"]),
        values(&[
            "list",
            "panes",
            "--worklane-id",
            "lane-1",
            "--worklane-id",
            "lane-2",
        ]),
        values(&["select", "pane", "--pane-index", "1", "--pane-index", "2"]),
        values(&["list", "panes", "--output-version", "2"]),
    ] {
        assert!(parse_product_cli(&invalid).is_err(), "accepted {invalid:?}");
    }
    assert!(
        parse_product_cli(&values(&["server", "list"]))
            .unwrap()
            .is_none()
    );
}

#[test]
fn current_output_version_is_explicit_and_preserved_on_the_wire() {
    for arguments in [
        values(&["list", "--json", "--output-version", "1"]),
        values(&[
            "select",
            "pane",
            "--pane-index",
            "1",
            "--output-version",
            "1",
        ]),
    ] {
        let Some(CliProductCommand::Request(request)) = parse_product_cli(&arguments).unwrap()
        else {
            panic!("expected product request for {arguments:?}");
        };
        assert!(
            request
                .arguments()
                .windows(2)
                .any(|pair| pair == ["--output-version", "1"])
        );
    }
}

#[test]
fn bounded_product_protocol_rejects_unknown_routes_and_large_payloads() {
    assert!(ProductIpcRequest::new(ProductIpcKind::Pane, "unknown", vec![]).is_err());
    assert!(
        ProductIpcRequest::new(
            ProductIpcKind::Discover,
            "panes",
            vec!["x".repeat(ProductIpcRequest::MAX_ARGUMENT_BYTES + 1)],
        )
        .is_err()
    );
    assert!(ProductIpcReply::success("x".repeat(ProductIpcReply::MAX_STDOUT_BYTES + 1)).is_err());
}
