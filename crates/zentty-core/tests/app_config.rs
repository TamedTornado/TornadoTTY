use zentty_core::{AppConfig, CommandFlattenAggressiveness};

#[test]
fn missing_clipboard_section_uses_source_defaults() {
    let config = AppConfig::parse_toml("[future]\nenabled = true\n").unwrap();
    let clipboard = config.clipboard;
    assert!(!clipboard.always_clean_copies);
    assert!(clipboard.clean_options.flatten_multi_line_commands);
    assert_eq!(
        clipboard.clean_options.command_flatten_aggressiveness,
        CommandFlattenAggressiveness::Normal
    );
    assert!(!clipboard.clean_options.preserve_blank_lines_when_flattening);
    assert!(clipboard.clean_options.remove_box_drawing);
    assert!(clipboard.clean_options.flatten_slash_command_selections);
    assert!(clipboard.clean_options.strip_url_tracking_parameters);
    assert!(clipboard.clean_options.quote_paths_with_spaces);
    assert!(clipboard.show_copy_markdown_command);
}

#[test]
fn source_clipboard_keys_map_to_clean_copy_policy() {
    let config = AppConfig::parse_toml(
        r#"
            [clipboard]
            always_clean_copies = true
            flatten_multi_line_commands = false
            command_flatten_aggressiveness = "high"
            preserve_blank_lines_when_flattening = true
            remove_box_drawing = false
            flatten_slash_command_selections = false
            strip_url_tracking_parameters = false
            quote_paths_with_spaces = false
            show_copy_markdown_command = false
        "#,
    )
    .unwrap();
    let clipboard = config.clipboard;
    assert!(clipboard.always_clean_copies);
    assert!(!clipboard.clean_options.flatten_multi_line_commands);
    assert_eq!(
        clipboard.clean_options.command_flatten_aggressiveness,
        CommandFlattenAggressiveness::High
    );
    assert!(clipboard.clean_options.preserve_blank_lines_when_flattening);
    assert!(!clipboard.clean_options.remove_box_drawing);
    assert!(!clipboard.clean_options.flatten_slash_command_selections);
    assert!(!clipboard.clean_options.strip_url_tracking_parameters);
    assert!(!clipboard.clean_options.quote_paths_with_spaces);
    assert!(!clipboard.show_copy_markdown_command);
}

#[test]
fn unknown_clipboard_keys_are_forward_compatible() {
    let config = AppConfig::parse_toml(
        "[clipboard]\nalways_clean_copies = true\nfuture_clipboard_flag = true\n",
    )
    .unwrap();
    assert!(config.clipboard.always_clean_copies);
}

#[test]
fn invalid_known_clipboard_values_reject_the_snapshot() {
    for source in [
        "[clipboard]\nalways_clean_copies = \"yes\"\n",
        "[clipboard]\ncommand_flatten_aggressiveness = \"extreme\"\n",
        "[clipboard]\ncommand_flatten_aggressiveness = high\n",
    ] {
        assert!(
            AppConfig::parse_toml(source).is_err(),
            "accepted {source:?}"
        );
    }
}

#[test]
fn parser_accepts_crlf_unicode_comments_and_is_deterministic() {
    let source = "# café\r\n[clipboard]\r\nalways_clean_copies = true # source option\r\n";
    assert_eq!(
        AppConfig::parse_toml(source).unwrap(),
        AppConfig::parse_toml(source).unwrap()
    );
}

#[test]
fn source_server_detection_defaults_and_owned_keys_are_preserved() {
    let defaults = AppConfig::parse_toml("").unwrap().server_detection;
    assert!(defaults.passive_detection_enabled);
    assert_eq!(defaults.preferred_browser_id, "system-default");
    assert!(defaults.enabled_browser_target_ids.is_empty());
    assert!(defaults.ignored_port_rules.is_empty());

    let configured = AppConfig::parse_toml(
        r#"
        [server_detection]
        passive_detection_enabled = false
        preferred_browser_id = "firefox"
        enabled_browser_target_ids = ["firefox", "custom:work"]
        ignored_port_rules = ["9229", "24678-24680"]
        future_server_option = true
        "#,
    )
    .unwrap()
    .server_detection;
    assert!(!configured.passive_detection_enabled);
    assert_eq!(configured.preferred_browser_id, "firefox");
    assert_eq!(
        configured.enabled_browser_target_ids,
        ["firefox", "custom:work"]
    );
    assert_eq!(configured.ignored_port_rules, ["9229", "24678-24680"]);
}

#[test]
fn source_open_with_defaults_preserve_linux_target_order() {
    let open_with = AppConfig::parse_toml("").unwrap().open_with;
    assert_eq!(open_with.primary_target_id, "system-file-manager");
    assert_eq!(
        open_with.enabled_target_ids,
        ["system-file-manager", "vscode", "cursor", "system-terminal"]
    );
    assert!(open_with.custom_apps.is_empty());
}

#[test]
fn source_open_with_keys_and_custom_apps_are_preserved() {
    let open_with = AppConfig::parse_toml(
        r#"
        [open_with]
        primary_target_id = "custom:zed"
        enabled_target_ids = ["custom:zed", "system-file-manager"]

        [[open_with.custom_apps]]
        id = "custom:zed"
        name = "Zed Preview"
        path = "/opt/zed preview/bin/zed"
        "#,
    )
    .unwrap()
    .open_with;

    assert_eq!(open_with.primary_target_id, "custom:zed");
    assert_eq!(
        open_with.enabled_target_ids,
        ["custom:zed", "system-file-manager"]
    );
    assert_eq!(open_with.custom_apps.len(), 1);
    assert_eq!(open_with.custom_apps[0].id, "custom:zed");
    assert_eq!(open_with.custom_apps[0].name, "Zed Preview");
    assert_eq!(open_with.custom_apps[0].path, "/opt/zed preview/bin/zed");
}

#[test]
fn malformed_known_open_with_values_reject_the_snapshot() {
    for source in [
        "[open_with]\nenabled_target_ids = \"vscode\"\n",
        "[open_with]\nprimary_target_id = 42\n",
        "[[open_with.custom_apps]]\nid = \"custom:x\"\nname = \"X\"\n",
    ] {
        assert!(
            AppConfig::parse_toml(source).is_err(),
            "accepted {source:?}"
        );
    }
}

#[test]
fn open_with_normalization_matches_source_duplicate_and_fallback_policy() {
    let open_with = AppConfig::parse_toml(
        r#"
        [open_with]
        primary_target_id = "custom:duplicate"
        enabled_target_ids = ["missing", "custom:duplicate", "vscode", "vscode"]

        [[open_with.custom_apps]]
        id = "custom:first"
        name = "First"
        path = "/opt/shared"

        [[open_with.custom_apps]]
        id = "custom:duplicate"
        name = "Duplicate Path"
        path = "/opt/shared"

        [[open_with.custom_apps]]
        id = "vscode"
        name = "Reserved ID"
        path = "/opt/reserved"

        [[open_with.custom_apps]]
        id = ""
        name = "Malformed"
        path = "/opt/malformed"
        "#,
    )
    .unwrap()
    .open_with;

    assert_eq!(open_with.custom_apps.len(), 1);
    assert_eq!(open_with.custom_apps[0].id, "custom:first");
    assert_eq!(open_with.enabled_target_ids, ["custom:first", "vscode"]);
    assert_eq!(open_with.primary_target_id, "custom:first");
}

#[test]
fn open_with_normalization_rejects_each_independently_empty_custom_field() {
    let open_with = AppConfig::parse_toml(
        r#"
        [open_with]
        primary_target_id = "vscode"
        enabled_target_ids = ["vscode"]

        [[open_with.custom_apps]]
        id = ""
        name = "Name"
        path = "/opt/id-empty"

        [[open_with.custom_apps]]
        id = "custom:name-empty"
        name = ""
        path = "/opt/name-empty"

        [[open_with.custom_apps]]
        id = "custom:path-empty"
        name = "Path Empty"
        path = ""
        "#,
    )
    .unwrap()
    .open_with;
    assert!(open_with.custom_apps.is_empty());
}

#[test]
fn valid_custom_primary_does_not_fall_back_to_first_enabled_builtin() {
    let open_with = AppConfig::parse_toml(
        r#"
        [open_with]
        primary_target_id = "custom:primary"
        enabled_target_ids = ["vscode", "custom:primary"]

        [[open_with.custom_apps]]
        id = "custom:primary"
        name = "Primary"
        path = "/opt/primary"
        "#,
    )
    .unwrap()
    .open_with;
    assert_eq!(open_with.primary_target_id, "custom:primary");
}
