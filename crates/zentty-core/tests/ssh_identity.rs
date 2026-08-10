use zentty_core::{SshConnectionOption, SshDestination, parse_ssh_destination};

#[test]
fn parses_source_supported_ssh_destination_forms() {
    for (argv, expected) in [
        (
            vec!["ssh", "host"],
            SshDestination::new("host", None, "host", None),
        ),
        (
            vec!["ssh", "user@host"],
            SshDestination::new("user@host", Some("user"), "host", None),
        ),
        (
            vec!["ssh", "-p", "2222", "host"],
            SshDestination::new("host", None, "host", Some(2222)),
        ),
        (
            vec!["ssh", "-l", "user", "host"],
            SshDestination::new("user@host", Some("user"), "host", None),
        ),
        (
            vec!["ssh", "-v", "host"],
            SshDestination::new("host", None, "host", None),
        ),
        (
            vec!["ssh", "user@2001:db8::1"],
            SshDestination::new("user@2001:db8::1", Some("user"), "2001:db8::1", None),
        ),
        (
            vec!["ssh", "-p2222", "-ldeploy", "--", "[2001:db8::1]"],
            SshDestination::new(
                "deploy@[2001:db8::1]",
                Some("deploy"),
                "[2001:db8::1]",
                Some(2222),
            ),
        ),
    ] {
        assert_eq!(parse_ssh_destination(&argv), Some(expected), "{argv:?}");
    }
}

#[test]
fn retains_only_reusable_connection_options_as_opaque_argv() {
    let destination = parse_ssh_destination(&[
        "ssh",
        "-4",
        "-i",
        "/key with spaces",
        "-o",
        "ProxyJump=bastion",
        "-o",
        "RemoteCommand=touch /tmp/wrong",
        "-L",
        "8080:localhost:80",
        "host",
    ])
    .unwrap();
    assert_eq!(
        destination.connection_options(),
        &[
            SshConnectionOption::Flag("-4".to_owned()),
            SshConnectionOption::Value {
                flag: "-i".to_owned(),
                value: "/key with spaces".to_owned(),
            },
            SshConnectionOption::Value {
                flag: "-o".to_owned(),
                value: "ProxyJump=bastion".to_owned(),
            },
        ]
    );
}

#[test]
fn rejects_missing_malformed_or_ambiguous_destinations() {
    for argv in [
        vec![],
        vec!["ssh"],
        vec!["ssh", "--"],
        vec!["ssh", "-p", "invalid", "host"],
        vec!["ssh", "-p", "70000", "host"],
        vec!["ssh", "-l"],
        vec!["ssh", "-v"],
        vec!["ssh", "user@"],
        vec!["ssh", "@host"],
        vec!["ssh", "   "],
    ] {
        assert_eq!(parse_ssh_destination(&argv), None, "{argv:?}");
    }
}

#[test]
fn explicit_user_overrides_only_a_target_without_a_user() {
    assert_eq!(
        parse_ssh_destination(&["ssh", "-l", "flag-user", "target-user@host"]),
        Some(SshDestination::new(
            "target-user@host",
            Some("target-user"),
            "host",
            None,
        ))
    );
}
