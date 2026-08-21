use zentty_core::{
    AgentIntegrationState, AppConfig, BackgroundOpacity, CommandFlattenAggressiveness,
    FocusFollowsMouseDelay, NewWorklanePlacement, PaneRightBehaviorMode, ThemeMode, UpdateChannel,
};

#[test]
fn appearance_defaults_and_source_compatible_values_are_explicit() {
    let defaults = AppConfig::parse_toml("").unwrap().appearance;
    assert_eq!(defaults.theme_mode, ThemeMode::Dark);
    assert_eq!(defaults.background_opacity, None);
    assert!(defaults.sync_opencode_theme_with_terminal);

    let appearance = AppConfig::parse_toml(
        r#"
        [appearance]
        theme_mode = "automatic"
        preferred_dark_theme_name = "Catppuccin Frappe"
        preferred_light_theme_name = "Catppuccin Latte"
        local_background_opacity = 0.876
        sync_opencode_theme_with_terminal = false
        "#,
    )
    .unwrap()
    .appearance;
    assert_eq!(appearance.theme_mode, ThemeMode::Automatic);
    assert_eq!(
        appearance.theme_spec().to_string(),
        "dark:Catppuccin Frappe,light:Catppuccin Latte"
    );
    assert_eq!(
        appearance.background_opacity,
        BackgroundOpacity::from_fraction(0.88)
    );
    assert!(!appearance.sync_opencode_theme_with_terminal);
}

#[test]
fn appearance_accepts_source_mode_tokens_but_rejects_unknown_or_nonfinite_values() {
    for (token, expected) in [
        ("followMacOS", ThemeMode::Automatic),
        ("alwaysDark", ThemeMode::Dark),
        ("alwaysLight", ThemeMode::Light),
    ] {
        let source = format!("[appearance]\ntheme_mode = \"{token}\"\n");
        assert_eq!(
            AppConfig::parse_toml(&source)
                .unwrap()
                .appearance
                .theme_mode,
            expected
        );
    }
    assert!(AppConfig::parse_toml("[appearance]\ntheme_mode = \"system\"\n").is_err());
    assert!(AppConfig::parse_toml("[appearance]\nlocal_background_opacity = nan\n").is_err());
}

#[test]
fn lifecycle_settings_use_source_defaults_and_exact_toml_keys() {
    let defaults = AppConfig::parse_toml("").unwrap();
    assert!(defaults.confirmations.confirm_before_closing_pane);
    assert!(defaults.confirmations.confirm_before_closing_window);
    assert!(defaults.confirmations.confirm_before_quitting);
    assert!(defaults.restore.restore_workspace_on_launch);

    let configured = AppConfig::parse_toml(
        r"
        [confirmations]
        confirm_before_closing_pane = false
        confirm_before_closing_window = false
        confirm_before_quitting = false
        future_confirmation = true

        [restore]
        restore_workspace_on_launch = false
        future_restore = true
        ",
    )
    .unwrap();
    assert!(!configured.confirmations.confirm_before_closing_pane);
    assert!(!configured.confirmations.confirm_before_closing_window);
    assert!(!configured.confirmations.confirm_before_quitting);
    assert!(!configured.restore.restore_workspace_on_launch);

    for source in [
        "[confirmations]\nconfirm_before_closing_pane = \"yes\"\n",
        "[confirmations]\nconfirm_before_closing_window = 1\n",
        "[confirmations]\nconfirm_before_quitting = []\n",
        "[restore]\nrestore_workspace_on_launch = \"no\"\n",
    ] {
        assert!(
            AppConfig::parse_toml(source).is_err(),
            "accepted {source:?}"
        );
    }
}

#[test]
fn notification_settings_use_source_defaults_and_exact_toml_keys() {
    let defaults = AppConfig::parse_toml("").unwrap().notifications;
    assert_eq!(defaults.sound_name, "");
    assert_eq!(defaults.custom_sound_display_name, None);

    let configured = AppConfig::parse_toml(
        r#"
        [notifications]
        sound_name = "message-new-instant"
        custom_sound_display_name = "My alert.ogg"
        future_notification_setting = true
        "#,
    )
    .unwrap()
    .notifications;
    assert_eq!(configured.sound_name, "message-new-instant");
    assert_eq!(
        configured.custom_sound_display_name.as_deref(),
        Some("My alert.ogg")
    );

    for source in [
        "[notifications]\nsound_name = true\n",
        "[notifications]\ncustom_sound_display_name = 3\n",
    ] {
        assert!(
            AppConfig::parse_toml(source).is_err(),
            "accepted {source:?}"
        );
    }
}

#[test]
fn updates_and_error_reporting_use_source_defaults_and_exact_toml_keys() {
    let defaults = AppConfig::parse_toml("").unwrap();
    assert_eq!(defaults.updates.channel, UpdateChannel::Stable);
    assert!(!defaults.error_reporting.enabled);

    let configured = AppConfig::parse_toml(
        r#"
        [updates]
        channel = "beta"
        future_update_setting = true

        [error_reporting]
        enabled = false
        future_privacy_setting = "preserved"
        "#,
    )
    .unwrap();
    assert_eq!(configured.updates.channel, UpdateChannel::Beta);
    assert!(!configured.error_reporting.enabled);

    for source in [
        "[updates]\nchannel = \"nightly\"\n",
        "[updates]\nchannel = true\n",
        "[error_reporting]\nenabled = \"yes\"\n",
    ] {
        assert!(
            AppConfig::parse_toml(source).is_err(),
            "accepted {source:?}"
        );
    }
}

#[test]
fn partial_reload_applies_independent_sections_and_retains_only_invalid_ones() {
    let last_good = AppConfig::parse_toml(
        r#"
        [clipboard]
        always_clean_copies = false

        [updates]
        channel = "beta"

        [panes]
        show_labels = false
        "#,
    )
    .unwrap();
    let partial = AppConfig::parse_toml_partial(
        r#"
        [clipboard]
        always_clean_copies = true

        [updates]
        channel = "not-a-channel"
        "#,
        &last_good,
    )
    .unwrap();

    assert!(partial.config.clipboard.always_clean_copies);
    assert_eq!(partial.config.updates, last_good.updates);
    assert_eq!(partial.config.panes, AppConfig::default().panes);
    assert_eq!(partial.retained_sections, ["updates"]);
}

#[test]
fn partial_reload_rejects_syntax_failure_without_exposing_input() {
    let secret = "operator-secret-must-not-appear";
    let error = AppConfig::parse_toml_partial(
        &format!("[clipboard\nsecret = {secret:?}\n"),
        &AppConfig::default(),
    )
    .unwrap_err();
    assert!(error.contains("invalid Zentty configuration"));
    assert!(!error.contains(secret));
}

#[test]
fn partial_reload_reports_invalid_sections_in_schema_order() {
    let partial = AppConfig::parse_toml_partial(
        r#"
        [appearance]
        theme_mode = "invalid"

        [updates]
        channel = "invalid"

        [pane_layout]
        right_split_behavior = "invalid"

        [shortcuts]
        [[shortcuts.bindings]]
        command_id = "pane.close"
        shortcut = "not a shortcut"
        "#,
        &AppConfig::default(),
    )
    .unwrap();
    assert_eq!(
        partial.retained_sections,
        ["appearance", "updates", "pane_layout", "shortcuts"]
    );
}

#[test]
fn fully_valid_partial_reload_matches_the_strict_parser() {
    let source = r#"
        [appearance]
        theme_mode = "light"

        [clipboard]
        always_clean_copies = true

        [updates]
        channel = "beta"

        [pane_layout]
        right_split_behavior = "alwaysSplit"
        visible_split_window_width = 1440
    "#;
    let strict = AppConfig::parse_toml(source).unwrap();
    let partial = AppConfig::parse_toml_partial(source, &AppConfig::default()).unwrap();
    assert_eq!(partial.config, strict);
    assert!(partial.retained_sections.is_empty());
}

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
fn worklane_and_pane_layout_settings_use_source_defaults_and_exact_keys() {
    let defaults = AppConfig::parse_toml("").unwrap();
    assert_eq!(
        defaults.worklanes.new_worklane_placement,
        NewWorklanePlacement::AfterCurrent
    );
    assert_eq!(
        defaults.pane_layout.right_split_behavior,
        PaneRightBehaviorMode::Adaptive
    );
    assert_eq!(defaults.pane_layout.visible_split_window_width, 1920);
    assert!(defaults.panes.show_labels);
    assert!(defaults.panes.show_borders);
    assert_eq!(defaults.panes.inactive_opacity_percent, 70);
    assert!(defaults.panes.show_project_icons);
    assert!(!defaults.panes.smooth_scroll_enabled);
    assert!(!defaults.panes.focus_follows_mouse);
    assert_eq!(
        defaults.panes.focus_follows_mouse_delay,
        FocusFollowsMouseDelay::Short
    );

    let configured = AppConfig::parse_toml(
        r#"
        [worklanes]
        new_worklane_placement = "top"

        [pane_layout]
        right_split_behavior = "alwaysSplit"
        visible_split_window_width = 1440

        [panes]
        show_labels = false
        show_borders = false
        inactive_opacity = 0.856
        show_project_icons = false
        smooth_scroll_enabled = true
        focus_follows_mouse = true
        focus_follows_mouse_delay = "immediate"
        "#,
    )
    .unwrap();
    assert_eq!(
        configured.worklanes.new_worklane_placement,
        NewWorklanePlacement::Top
    );
    assert_eq!(
        configured.pane_layout.right_split_behavior,
        PaneRightBehaviorMode::AlwaysSplit
    );
    assert_eq!(configured.pane_layout.visible_split_window_width, 1440);
    assert_eq!(
        configured.pane_layout.right_insertion_behavior(800),
        zentty_core::PaneRightInsertionBehavior::VisibleSplit
    );
    assert!(!configured.panes.show_labels);
    assert!(!configured.panes.show_borders);
    assert_eq!(configured.panes.inactive_opacity_percent, 86);
    assert!(!configured.panes.show_project_icons);
    assert!(configured.panes.smooth_scroll_enabled);
    assert!(configured.panes.focus_follows_mouse);
    assert_eq!(
        configured.panes.focus_follows_mouse_delay,
        FocusFollowsMouseDelay::Immediate
    );

    for source in [
        "[worklanes]\nnew_worklane_placement = \"middle\"\n",
        "[pane_layout]\nright_split_behavior = \"sometimes\"\n",
        "[pane_layout]\nvisible_split_window_width = 1600\n",
        "[panes]\ninactive_opacity = 0.59\n",
        "[panes]\ninactive_opacity = 1.01\n",
        "[panes]\nfocus_follows_mouse_delay = \"long\"\n",
        "[panes]\nshow_project_icons = \"yes\"\n",
    ] {
        assert!(
            AppConfig::parse_toml(source).is_err(),
            "accepted {source:?}"
        );
    }
}

#[test]
fn agent_settings_use_source_defaults_sections_and_forward_compatible_states() {
    let defaults = AppConfig::parse_toml("").unwrap();
    assert!(!defaults.agent_teams.enabled);
    assert!(defaults.agent_caffeination.enabled);
    assert!(defaults.menu_bar.show_status_item);
    assert!(!defaults.agent_integrations.grandfathered_v1);
    assert!(defaults.agent_integrations.states.is_empty());

    let configured = AppConfig::parse_toml(
        r#"
        [agent_teams]
        enabled = true
        [agent_caffeination]
        enabled = false
        [menu_bar]
        show_status_item = false
        [agent_integrations]
        grandfathered_v1 = true
        [agent_integrations.states]
        claude = "off"
        codex = "on"
        future_agent = "future-state"
        "#,
    )
    .unwrap();
    assert!(configured.agent_teams.enabled);
    assert!(!configured.agent_caffeination.enabled);
    assert!(!configured.menu_bar.show_status_item);
    assert!(configured.agent_integrations.grandfathered_v1);
    assert_eq!(
        configured.agent_integrations.states["claude"],
        AgentIntegrationState::Off
    );
    assert_eq!(
        configured.agent_integrations.states["codex"],
        AgentIntegrationState::On
    );
    assert!(
        !configured
            .agent_integrations
            .states
            .contains_key("future_agent")
    );

    for source in [
        "[agent_teams]\nenabled = \"yes\"\n",
        "[agent_caffeination]\nenabled = 1\n",
        "[menu_bar]\nshow_status_item = []\n",
        "[agent_integrations]\ngrandfathered_v1 = \"no\"\n",
    ] {
        assert!(
            AppConfig::parse_toml(source).is_err(),
            "accepted {source:?}"
        );
    }
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
    assert!(defaults.custom_browsers.is_empty());
    assert!(defaults.ignored_port_rules.is_empty());

    let configured = AppConfig::parse_toml(
        r#"
        [server_detection]
        passive_detection_enabled = false
        preferred_browser_id = "firefox"
        enabled_browser_target_ids = ["firefox", "custom:work"]
        ignored_port_rules = ["9229", "24678-24680"]
        future_server_option = true

        [[server_detection.custom_browsers]]
        id = "custom:work"
        name = "Work Browser"
        path = "/opt/work-browser"
        bundle_identifier = "org.example.WorkBrowser"
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
    assert_eq!(configured.custom_browsers.len(), 1);
    assert_eq!(configured.custom_browsers[0].id, "custom:work");
    assert_eq!(configured.custom_browsers[0].name, "Work Browser");
    assert_eq!(configured.custom_browsers[0].path, "/opt/work-browser");
    assert_eq!(
        configured.custom_browsers[0].bundle_identifier.as_deref(),
        Some("org.example.WorkBrowser")
    );
}

#[test]
fn server_browser_config_deduplicates_custom_paths_and_rejects_reserved_rows() {
    let configured = AppConfig::parse_toml(
        r#"
        [server_detection]
        preferred_browser_id = "custom:duplicate"
        enabled_browser_target_ids = ["custom:duplicate", "firefox", "firefox", ""]

        [[server_detection.custom_browsers]]
        id = "custom:primary"
        name = "Primary"
        path = "/opt/browser"

        [[server_detection.custom_browsers]]
        id = "custom:duplicate"
        name = "Duplicate"
        path = "/opt/browser"

        [[server_detection.custom_browsers]]
        id = "system-default"
        name = "Reserved"
        path = "/opt/reserved"
        "#,
    )
    .unwrap()
    .server_detection;

    assert_eq!(configured.custom_browsers.len(), 1);
    assert_eq!(configured.custom_browsers[0].id, "custom:primary");
    assert_eq!(configured.preferred_browser_id, "custom:primary");
    assert_eq!(
        configured.enabled_browser_target_ids,
        ["custom:primary", "firefox"]
    );
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
fn open_with_presentation_reconciles_stale_targets_and_primary_selection() {
    let configured = AppConfig::parse_toml(
        r#"
        [open_with]
        primary_target_id = "custom:gone"
        enabled_target_ids = ["custom:gone", "cursor", "system-terminal"]

        [[open_with.custom_apps]]
        id = "custom:gone"
        name = "Gone"
        path = "/opt/gone"

        [[open_with.custom_apps]]
        id = "custom:kept"
        name = "Kept"
        path = "/opt/kept"
        "#,
    )
    .unwrap()
    .open_with;

    let reconciled =
        configured.reconciled_available(&["system-terminal".into(), "custom:kept".into()]);
    assert_eq!(reconciled.primary_target_id, "system-terminal");
    assert_eq!(reconciled.enabled_target_ids, ["system-terminal"]);
    assert_eq!(reconciled.custom_apps.len(), 1);
    assert_eq!(reconciled.custom_apps[0].id, "custom:kept");

    let no_targets = reconciled.reconciled_available(&[]);
    assert!(no_targets.enabled_target_ids.is_empty());
    assert!(no_targets.custom_apps.is_empty());
    assert_eq!(no_targets.primary_target_id, "system-file-manager");
}

#[test]
fn dev_server_presentation_reconciles_stale_browser_targets() {
    let configured = AppConfig::parse_toml(
        r#"
        [server_detection]
        passive_detection_enabled = false
        preferred_browser_id = "custom:gone"
        enabled_browser_target_ids = ["custom:gone", "custom:kept"]
        ignored_port_rules = ["3000-3002"]

        [[server_detection.custom_browsers]]
        id = "custom:gone"
        name = "Gone"
        path = "/opt/gone"

        [[server_detection.custom_browsers]]
        id = "custom:kept"
        name = "Kept"
        path = "/opt/kept"
        "#,
    )
    .unwrap()
    .server_detection;

    let reconciled =
        configured.reconciled_available(&["system-default".into(), "custom:kept".into()]);
    assert!(!reconciled.passive_detection_enabled);
    assert_eq!(reconciled.preferred_browser_id, "system-default");
    assert_eq!(reconciled.enabled_browser_target_ids, ["custom:kept"]);
    assert_eq!(reconciled.custom_browsers.len(), 1);
    assert_eq!(reconciled.custom_browsers[0].id, "custom:kept");
    assert_eq!(reconciled.ignored_port_rules, ["3000-3002"]);
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
