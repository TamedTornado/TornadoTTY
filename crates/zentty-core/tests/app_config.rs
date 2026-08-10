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
