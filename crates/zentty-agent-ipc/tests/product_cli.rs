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
    ] {
        assert!(parse_product_cli(&invalid).is_err(), "accepted {invalid:?}");
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
