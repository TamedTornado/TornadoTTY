use zentty_core::{
    CleanCopyOptions, CommandFlattenAggressiveness, clean_copy, clean_copy_with_columns,
    is_likely_markdown, reformat_markdown,
};

fn clean(input: &str) -> String {
    clean_copy(input, CleanCopyOptions::default()).text
}

fn clean_at(input: &str, columns: usize) -> String {
    clean_copy_with_columns(input, CleanCopyOptions::default(), Some(columns)).text
}

#[test]
fn clean_copy_preserves_structured_record_blocks() {
    let systemd = concat!(
        "# /etc/systemd/system/tailscale-local-lan-route.service\n",
        "[Unit]\n",
        "Description=Prefer local HQ LAN over overlapping Tailscale route\n",
        "After=network-online.target tailscaled.service\n",
        "Wants=network-online.target\n\n",
        "[Service]\n",
        "Type=oneshot\n",
        "ExecStart=-/usr/sbin/ip rule add to 10.0.0.0/24 priority 2500 lookup main\n",
        "ExecStop=-/usr/sbin/ip rule del to 10.0.0.0/24 priority 2500 lookup main\n",
        "RemainAfterExit=yes\n\n",
        "[Install]\n",
        "WantedBy=multi-user.target",
    );
    assert_eq!(clean(systemd), systemd);

    let colon_records = concat!(
        "name: tailscale-local-lan-route\n",
        "description: Prefer the local HQ LAN over the overlapping Tailscale route\n",
        "command: /usr/sbin/ip rule add to 10.0.0.0/24 priority 2500 lookup main\n",
        "enabled: true",
    );
    assert_eq!(clean(colon_records), colon_records);

    let mixed = concat!(
        "Summary:\n",
        "The route helper adds a policy rule so the local HQ LAN wins over the wider\n",
        "Tailscale route, and removes it again when the unit is stopped or the host\n",
        "reboots.\n\n",
        "[Service]\n",
        "Type=oneshot\n",
        "ExecStart=-/usr/sbin/ip rule add to 10.0.0.0/24 priority 2500 lookup main\n",
        "RemainAfterExit=yes",
    );
    assert_eq!(
        clean_at(mixed, 71),
        concat!(
            "Summary:\n",
            "The route helper adds a policy rule so the local HQ LAN wins over the wider ",
            "Tailscale route, and removes it again when the unit is stopped or the host reboots.\n\n",
            "[Service]\n",
            "Type=oneshot\n",
            "ExecStart=-/usr/sbin/ip rule add to 10.0.0.0/24 priority 2500 lookup main\n",
            "RemainAfterExit=yes",
        )
    );
}

#[test]
fn clean_copy_still_reflows_prose_whose_first_line_contains_equals() {
    let input = concat!(
        "Set DEBUG=1 in the environment before starting the service so the route helper\n",
        "prints every rule it adds and removes to the journal.",
    );
    assert_eq!(
        clean(input),
        "Set DEBUG=1 in the environment before starting the service so the route helper prints every rule it adds and removes to the journal."
    );
}

#[test]
fn clean_copy_uses_terminal_width_to_preserve_real_newlines() {
    let input = concat!(
        "Summary:\n",
        "The route helper adds a policy rule so the local HQ LAN wins over the wider\n",
        "Tailscale route, and removes it again when the unit is stopped or the host\n",
        "reboots.",
    );
    assert_eq!(
        clean_at(input, 80),
        concat!(
            "Summary:\n",
            "The route helper adds a policy rule so the local HQ LAN wins over the wider ",
            "Tailscale route, and removes it again when the unit is stopped or the host reboots.",
        )
    );
}

#[test]
fn clean_copy_falls_back_to_longest_line_when_width_is_unknown() {
    let input = concat!(
        "Ran the migration on staging\n",
        "Done in 4.2s\n",
        "Another command output line that is fairly long for a terminal",
    );
    assert_eq!(clean(input), input);
}

#[test]
fn clean_copy_folds_lines_that_reach_or_exceed_terminal_width() {
    let input = concat!(
        "This line was soft-wrapped by the terminal and copied back as one long line over\n",
        "the pane width.",
    );
    assert_eq!(
        clean_at(input, 40),
        "This line was soft-wrapped by the terminal and copied back as one long line over the pane width."
    );
}

fn padded(line: &str, width: usize) -> String {
    format!(
        "{line}{}",
        " ".repeat(width.saturating_sub(line.chars().count()))
    )
}

#[test]
fn removes_terminal_control_sequences_without_losing_text() {
    assert_eq!(clean("\x1b[31mred\x1b[0m plain"), "red plain");
    assert_eq!(
        clean("before\x1b]8;;https://example.com\x07link\x1b]8;;\x1b\\ after"),
        "beforelink after"
    );
    assert_eq!(clean("\x1b(BASCII"), "ASCII");
    assert_eq!(clean("before\x1b=keypad mode\nafter"), "before\nafter");
    assert_eq!(clean("unknown\x1bXsequence"), "unknown\x1bXsequence");
    assert_eq!(clean("trailing escape\x1b"), "trailing escape\x1b");
    assert_eq!(clean("\x1b[?25lhidden\x1b[?25h"), "hidden");
    assert_eq!(clean("before\x1b]0;title\x1b\\after"), "beforeafter");
    assert_eq!(
        clean("before\x1b]0;partial\x1bXstill-osc\x07after"),
        "beforeafter"
    );
}

#[test]
fn trims_terminal_padding_but_preserves_meaningful_final_newline() {
    assert_eq!(clean("one   \ntwo\t  \n"), "one\ntwo\n");
    assert_eq!(clean("one\n\n\n"), "one\n");
}

#[test]
fn strips_monotonic_line_number_gutters_but_not_times_or_ipv6() {
    assert_eq!(
        clean("10 │ alpha\n11 │ beta\n12 │ gamma"),
        "alpha\nbeta\ngamma"
    );
    assert_eq!(
        clean("10:30 meeting\n11:30 review"),
        "10:30 meeting\n11:30 review"
    );
    assert_eq!(
        clean("2001:db8::1\n2001:db8::2"),
        "2001:db8::1\n2001:db8::2"
    );
    assert_eq!(clean("     1\thello\n     2\tworld"), "hello\nworld");
    assert_eq!(clean("1:first\n5:second\n12:third"), "first\nsecond\nthird");
    assert_eq!(
        clean("1:00:00 timeout\n2:00:01 retry"),
        "00:00 timeout\n00:01 retry"
    );
    assert_eq!(clean("1| hello\n2| world"), "hello\nworld");
    assert_eq!(
        clean("47 │ Host *\n48 │ SetEnv TERM=xterm-256color"),
        "Host *\nSetEnv TERM=xterm-256color"
    );
    assert_eq!(clean("1:hello\nno number here"), "1:hello\nno number here");
    assert_eq!(clean("1:0\n2:1"), "0\n1");
    assert_eq!(clean("1:a0 value\n2:b1 value"), "a0 value\nb1 value");
    assert_eq!(
        clean("5:fifth\n3:third\n1:first\n2:second\n4:fourth"),
        "5:fifth\n3:third\n1:first\n2:second\n4:fourth"
    );
    assert_eq!(
        clean("1:one\n2:two\n3:three\n4:four\nplain"),
        "1:one\n2:two\n3:three\n4:four\nplain"
    );
    assert_eq!(
        clean("1:one\n2:two\n3:three\n4:four\n5:five\nplain"),
        "one\ntwo\nthree\nfour\nfive\nplain"
    );
    assert_eq!(
        clean("1:one\n1:again\n2:two\n3:three"),
        "one\nagain\ntwo\nthree"
    );
}

#[test]
fn strips_box_chrome_only_with_structural_evidence() {
    assert_eq!(
        clean("┌────────┐\n│ alpha  │\n│ beta   │\n└────────┘"),
        "alpha\nbeta"
    );
    assert_eq!(clean("one ─ separator\ntwo"), "one ─ separator\ntwo");
    assert_eq!(
        clean("curl -I https://example.com | │ head -n 5"),
        "curl -I https://example.com | head -n 5"
    );
    assert_eq!(
        clean("curl -I https://github.com/releases/ │ download/app.zip | head"),
        "curl -I https://github.com/releases/download/app.zip | head"
    );
    assert_eq!(clean("│ hello\n"), "hello\n");
    assert_eq!(
        clean("keep\n│ legitimate diagram line\ndone"),
        "keep\n│ legitimate diagram line\ndone"
    );
    assert_eq!(clean("──────"), "──────");
    assert_eq!(clean("│\n│"), "│\n│");
    assert_eq!(
        clean("──────\nkeep │\nplain one\nplain two"),
        "──────\nkeep │\nplain one\nplain two"
    );
    assert_eq!(clean("curl │ head\nkeep │"), "curl head\nkeep │");
    assert_eq!(
        clean("│ first\n│ second\nplain third\nplain fourth"),
        "│ first\n│ second\nplain third\nplain fourth"
    );
}

#[test]
fn strips_prompts_only_when_the_selection_is_predominantly_prompted() {
    assert_eq!(
        clean("$ cargo test\n$ git status\noutput"),
        "cargo test\ngit status\noutput"
    );
    assert_eq!(clean("# Heading\nparagraph"), "# Heading\nparagraph");
    assert_eq!(clean("$ command\noutput"), "$ command\noutput");
    assert_eq!(
        clean("$ one\n$ two\n$ three\nout one\nout two\nout three"),
        "$ one\n$ two\n$ three\nout one\nout two\nout three"
    );
    assert_eq!(
        clean("$ one\n$ two\n$ three\n$ four\nout one\nout two"),
        "one\ntwo\nthree\nfour\nout one\nout two"
    );
}

#[test]
fn flattens_explicit_command_continuations_but_preserves_prose_and_status() {
    assert_eq!(
        clean("xcodebuild test \\\n  -scheme Zentty"),
        "xcodebuild test -scheme Zentty"
    );
    assert_eq!(
        clean("git log --oneline |\n  head -n 5"),
        "git log --oneline | head -n 5"
    );
    let prose = "I pushed the fix to feature/login-flow.\nCan you review it today?";
    assert_eq!(clean(prose), prose);
    let status = "$ g\n M .github/workflows/ci.yml\n?? .github/workflows/promote.yml";
    assert_eq!(clean(status), status);
}

#[test]
fn command_flattening_can_be_disabled() {
    let options = CleanCopyOptions {
        flatten_multi_line_commands: false,
        ..Default::default()
    };
    assert_eq!(
        clean_copy("git status \\\n+  --short", options).text,
        "git status \\\n+  --short"
    );
}

#[test]
fn cleans_source_agent_prompt_shapes_without_flattening_structures() {
    assert_eq!(
        clean(
            "› first paragraph wraps\n  onto a second line\n\n  second paragraph wraps\n  onto another line"
        ),
        "first paragraph wraps onto a second line\n\nsecond paragraph wraps onto another line"
    );
    assert_eq!(
        clean("› first paragraph\n\n\n  second paragraph"),
        "first paragraph\n\n\nsecond paragraph"
    );
    assert_eq!(clean("❯ /commit"), "/commit");
    assert_eq!(
        clean(
            "❯ /my-skill:run-task \"Analyze the dataset\n  for patterns and report findings\"\n────────────────────────────────────────\n/my-skill:run-task \"Analyze the dataset\n  for patterns and report findings\""
        ),
        "/my-skill:run-task \"Analyze the dataset for patterns and report findings\""
    );
    assert_eq!(
        clean("⏺ Ran the analysis and found two\n  issues worth fixing."),
        "Ran the analysis and found two issues worth fixing."
    );
    assert_eq!(
        clean("● Models available and ready\n  to take the next task."),
        "Models available and ready to take the next task."
    );
    for structured in [
        "› func hello() {\n    print(\"world\")\n}",
        "› let value = makeValue()\n  await render(value)",
        "› - first item\n  - second item\n  - third item",
        "› {\n  \"name\": \"Zentty\",\n  \"enabled\": true\n}",
        "› $ git status\n  On branch main\n  $ git diff",
        "• first item\n• second item\n• third item",
        "⏺ Read(CleanCopyPipeline.swift)\n⏺ Edit(CleanCopyPipeline.swift)",
    ] {
        assert_eq!(clean(structured), structured);
    }

    let sixty = std::iter::once("› first")
        .chain(std::iter::repeat_n("continuation", 59))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(
        clean(&sixty),
        format!("first {}", "continuation ".repeat(59).trim_end())
    );
    let sixty_one = format!("{sixty}\ncontinuation");
    assert_eq!(clean(&sixty_one), sixty_one);
}

#[test]
fn padded_short_row_evidence_uses_source_boundaries() {
    let git_status = [
        padded("❯ g", 100),
        padded(" M .github/workflows/ci.yml", 100),
        padded("?? .github/workflows/promote.yml", 100),
    ]
    .join("\n");
    assert_eq!(
        clean(&git_status),
        "g\n M .github/workflows/ci.yml\n?? .github/workflows/promote.yml"
    );

    let three_spaces = "› first   \n  second   ";
    assert_eq!(clean(three_spaces), "first second");
    let four_spaces = "› first    \n  second    ";
    assert_eq!(clean(four_spaces), "first\n  second");
    assert_eq!(clean("› first\n    \n    \n  second"), "first\n\n\nsecond");

    // The prompt marker and following space count toward the source's <60 row limit.
    let fifty_nine = "x".repeat(57);
    assert_eq!(
        clean(&format!("› {fifty_nine}    \n  second    ")),
        format!("{fifty_nine}\n  second")
    );
    let sixty = "x".repeat(58);
    assert_eq!(
        clean(&format!("› {sixty}    \n  second    ")),
        format!("{sixty} second")
    );
}

#[test]
fn reflows_source_agent_output_and_blockquotes() {
    assert_eq!(
        clean(
            "• Hi Peter — I'll keep this review\n  tight and focus only on issues.\n\n  Using the code-review skill."
        ),
        "Hi Peter — I'll keep this review tight and focus only on issues.\n\nUsing the code-review skill."
    );
    assert_eq!(
        clean("• First status wraps\n  onto another line.\n\n• Second status wraps\n  too."),
        "• First status wraps onto another line.\n\n• Second status wraps too."
    );
    assert_eq!(
        clean(
            ">> The deeper nested quote begins here and runs long enough to read as prose wrapping,\n>> and it continues on this second line before it ends."
        ),
        "The deeper nested quote begins here and runs long enough to read as prose wrapping, and it continues on this second line before it ends."
    );
    assert_eq!(
        clean(
            "> It's a good first step, and publishing the source is useful.\n>\n> I did look through it. The specific repository-snapshot path we saw\n> evidence of before is disabled or unavailable in this public build."
        ),
        "It's a good first step, and publishing the source is useful.\n\nI did look through it. The specific repository-snapshot path we saw evidence of before is disabled or unavailable in this public build."
    );
}

#[test]
fn plain_prose_reflow_obeys_every_source_bailout_and_quote_threshold() {
    let long = "This is a deliberately long prose line that exceeds sixty characters for wrapping.";
    assert_eq!(clean(long), long);
    for structured in [
        "fn main() { this line is deliberately long enough to look like wrapped prose\n    return; }",
        "{\n\"description\": \"a deliberately long structured value that exceeds sixty characters\"\n}",
        "$ printf 'a deliberately long shell command line that exceeds sixty characters'\ncommand output",
    ] {
        assert_eq!(clean(structured), structured);
    }

    let padded_rows = format!("{}    \n{}    ", "a".repeat(59), "b".repeat(59));
    assert_eq!(
        clean(&padded_rows),
        format!("{}\n{}", "a".repeat(59), "b".repeat(59))
    );

    let sixty = std::iter::repeat_n(long, 60).collect::<Vec<_>>().join("\n");
    assert_eq!(
        clean(&sixty),
        std::iter::repeat_n(long, 60).collect::<Vec<_>>().join(" ")
    );
    let sixty_one = format!("{sixty}\n{long}");
    assert_eq!(clean(&sixty_one), sixty_one);

    let minority = format!("> {long}\nplain continuation\nplain ending");
    assert_eq!(
        clean_at(&minority, 20),
        format!("> {long} plain continuation plain ending")
    );
    let majority = format!("> {long}\n> quoted continuation\nplain ending");
    assert_eq!(
        clean_at(&majority, 20),
        format!("{long} quoted continuation plain ending")
    );
}

#[test]
fn prose_reflow_preserves_repeated_gaps_and_markdown_item_boundaries() {
    let input = "• first paragraph wraps across\n  another line\n\n  second paragraph wraps across\n  another line\n\n\n  third paragraph";
    assert_eq!(
        clean(input),
        "first paragraph wraps across another line\n\nsecond paragraph wraps across another line\n\n\nthird paragraph"
    );
    let mixed = "A deliberately long opening sentence that exceeds sixty characters for reflow.\nIt has a continuation.\n\n- first item wraps\n  onto another line\n- second item";
    assert_eq!(
        clean_at(mixed, 20),
        "A deliberately long opening sentence that exceeds sixty characters for reflow. It has a continuation.\n\n- first item wraps onto another line\n- second item"
    );
}

#[test]
fn numbered_items_require_nonempty_ascii_digits_and_a_space() {
    let long = "A deliberately long opening sentence that exceeds sixty characters for reflow.";
    let input = format!("{long}\ncontinuation\n\n1. first\n2) second");
    assert_eq!(
        clean(&input),
        format!("{long} continuation\n\n1. first\n2) second")
    );
    for not_a_list in [". item", "x. item", "1.item"] {
        let input = format!("{long}\ncontinuation\n\n{not_a_list}\nwrapped");
        assert_eq!(
            clean_at(&input, 20),
            format!("{long} continuation\n\n{not_a_list} wrapped")
        );
    }
}

#[test]
fn source_detection_handles_begin_without_braces() {
    let input = "def perform_work\nbegin\nthis deliberately long source line exceeds sixty characters but has no punctuation\nend";
    assert_eq!(clean(input), input);
}

#[test]
fn compact_shell_blocks_survive_when_command_flattening_is_disabled() {
    let options = CleanCopyOptions {
        flatten_multi_line_commands: false,
        ..Default::default()
    };
    let block = "RAILS_ENV=test cargo test --all-targets --all-features with-an-intentionally-long-argument\nsudo git status --short --branch with-an-intentionally-long-argument";
    assert_eq!(clean_copy(block, options).text, block);
    let path_block = "./scripts/test-on-display with-an-intentionally-long-argument-that-wraps\n/usr/bin/printf another-intentionally-long-argument-that-wraps";
    assert_eq!(clean_copy(path_block, options).text, path_block);

    for invalid_environment in ["=bad", "BAD-NAME=value"] {
        let input = format!(
            "{invalid_environment} cargo test with-an-intentionally-long-argument-that-wraps\ncontinuation text"
        );
        assert_eq!(
            clean_copy(&input, options).text,
            format!(
                "{invalid_environment} cargo test with-an-intentionally-long-argument-that-wraps continuation text"
            )
        );
    }
}

#[test]
fn agent_reflow_preserves_wrapped_tokens_without_fusing_prose() {
    assert_eq!(
        clean("› open /tmp/scan-qr-f1cc4328-eb1d-4a3c-9bd2-\n  f1a4ccda5f6a.png"),
        "open /tmp/scan-qr-f1cc4328-eb1d-4a3c-9bd2-f1a4ccda5f6a.png"
    );
    assert_eq!(
        clean("› export N\n  ODE_PATH=/usr/bin"),
        "export NODE_PATH=/usr/bin"
    );
    assert_eq!(
        clean("› export NODE_\n  PATH=/usr/bin"),
        "export NODE_PATH=/usr/bin"
    );
    assert_eq!(
        clean("› open ~/Library/\n  Application Support/Zentty"),
        "open ~/Library/Application Support/Zentty"
    );
    for (input, expected) in [
        (
            "⏺ Set THE\n  API_KEY before running.",
            "Set THE API_KEY before running.",
        ),
        (
            "⏺ The var is FOO_BAR\n  BAZ is a separate token.",
            "The var is FOO_BAR BAZ is a separate token.",
        ),
        (
            "› Grade A\n  B students passed",
            "Grade A B students passed",
        ),
        (
            "⏺ We shipped to the EU\n  US customers are next.",
            "We shipped to the EU US customers are next.",
        ),
        (
            "› Here is the answer.\n  Here is more context.",
            "Here is the answer. Here is more context.",
        ),
    ] {
        assert_eq!(clean(input), expected);
    }
}

#[test]
fn slash_command_decoration_is_source_compatible_and_optional() {
    assert_eq!(
        clean("/my-skill:run-task \"Analyze the dataset\n  for patterns and report findings\""),
        "/my-skill:run-task \"Analyze the dataset for patterns and report findings\""
    );
    assert_eq!(
        clean("\"/commit \\\"with details\\\"\""),
        "/commit \"with details\""
    );
    let options = CleanCopyOptions {
        flatten_slash_command_selections: false,
        ..Default::default()
    };
    let input = "/commit first line\n  second line";
    assert_eq!(clean_copy(input, options).text, input);
    for invalid in [
        "/:task first\nsecond",
        "/task: first\nsecond",
        "/task.name first\nsecond",
        "/one:two:three first\nsecond",
        "\"/commit without escaped quote\"",
        "\"/commit \\\"unterminated",
        "/commit \\\"escaped but unquoted\\\"",
    ] {
        assert_eq!(clean(invalid), invalid);
    }
}

#[test]
fn separated_agent_bullets_require_one_leading_marker_per_block() {
    for invalid in [
        "• first\n\ncontinuation\n• second",
        "• one\n• extra\n\n• second",
    ] {
        assert_eq!(clean(invalid), invalid);
    }
}

#[test]
fn command_aggressiveness_controls_heuristic_flattening() {
    let input = "git push origin feature/login-flow\n  --force";
    assert_eq!(clean(input), input);
    let low = CleanCopyOptions {
        command_flatten_aggressiveness: CommandFlattenAggressiveness::Low,
        ..Default::default()
    };
    assert_eq!(clean_copy(input, low).text, input);
    let high = CleanCopyOptions {
        command_flatten_aggressiveness: CommandFlattenAggressiveness::High,
        ..Default::default()
    };
    assert_eq!(
        clean_copy(input, high).text,
        "git push origin feature/login-flow --force"
    );
    for block in [
        "RAILS_ENV=test bundle exec rspec spec/models/example.rb:311\nscripts/test-on-virtual-display --suite CleanCopy\ncargo test --all-targets",
        "git status --short --branch\npnpm install --frozen-lockfile\nnode dist/cli.js --help",
        "FIRST=value\nSECOND=value",
        "feature/login-flow\nrelease/candidate",
    ] {
        assert_eq!(clean(block), block);
    }
}

#[test]
fn command_flattening_ports_source_limits_vetoes_and_signal_classes() {
    let explicit_four = ["custom one \\", "  two \\", "  three \\", "  four"].join("\n");
    assert_eq!(clean(&explicit_four), "custom one two three four");
    let mut explicit_five = explicit_four.clone();
    explicit_five.push_str(" \\");
    explicit_five.push('\n');
    explicit_five.push_str("  five");
    assert_eq!(clean(&explicit_five), explicit_five);
    let eleven = std::iter::repeat_n("custom \\", 11)
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(clean(&eleven), eleven);

    let env = "FIRST=value\nSECOND=value";
    assert_eq!(clean(env), env);
    let high = CleanCopyOptions {
        command_flatten_aggressiveness: CommandFlattenAggressiveness::High,
        ..Default::default()
    };
    assert_eq!(clean_copy(env, high).text, "FIRST=value SECOND=value");

    let padded_unknown = format!(
        "{}    \n{}    ",
        "custom run".repeat(5),
        "continuation".repeat(4)
    );
    assert_eq!(
        clean_copy(&padded_unknown, high).text,
        padded_unknown
            .lines()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("\n")
    );

    for (input, expected) in [
        (
            "custom run && value\n  continuation text.",
            "custom run && value continuation text.",
        ),
        (
            "custom run || value\n  continuation text.",
            "custom run || value continuation text.",
        ),
        (
            "custom src/value\n  continuation text.",
            "custom src/value continuation text.",
        ),
        (
            "git something\n  continuation text.",
            "git something continuation text.",
        ),
        (
            "custom --flag\n  continuation text.",
            "custom --flag continuation text.",
        ),
    ] {
        assert_eq!(clean(input), expected);
    }
    let source =
        "fn something(\n  continuation text that is deliberately long enough to be prose-shaped";
    assert_eq!(clean(source), source);
}

#[test]
fn repairs_wrapped_urls_without_swallowing_trailing_prose() {
    assert_eq!(
        clean("https://example.com/very/long/path/segment/that\n/continues/here?query=value"),
        "https://example.com/very/long/path/segment/that/continues/here?query=value"
    );
    assert_eq!(
        clean("https://example.com/a\n/b\n/c"),
        "https://example.com/a/b/c"
    );
    let prose = "https://example.com/docs/setup\nOpen this in your browser, then continue.";
    assert_eq!(clean(prose), prose);
    let prefixed = "prefixhttp://example.com/a\n/b";
    assert_eq!(clean(prefixed), prefixed);
}

#[test]
fn removes_only_known_tracking_parameters() {
    assert_eq!(
        clean("https://example.com/reset?token=abc123&utm_source=mail"),
        "https://example.com/reset?token=abc123"
    );
    assert_eq!(
        clean("https://example.com/search?q=zentty&fbclid=abc"),
        "https://example.com/search?q=zentty"
    );
    assert_eq!(
        clean("https://example.com/reset?token=abc123&UTM_Source=mail"),
        "https://example.com/reset?token=abc123"
    );
    assert_eq!(
        clean("https://example.com/search?q=zentty"),
        "https://example.com/search?q=zentty"
    );
    assert_eq!(
        clean("https://github.com/search?q=zentty&type=repositories"),
        "https://github.com/search?q=zentty&type=repositories"
    );
    let multiline = "https://example.com/a?utm_source=mail\nprose with spaces";
    assert_eq!(clean(multiline), multiline);
}

#[test]
fn removes_youtube_share_parameters_but_keeps_playback_parameters() {
    assert_eq!(
        clean("https://youtube.com/watch?v=abc&t=12s&feature=share&si=xyz&utm_source=mail"),
        "https://youtube.com/watch?v=abc&t=12s"
    );
    assert_eq!(
        clean("https://www.youtube.com/watch?v=abc&t=12s&feature=share"),
        "https://www.youtube.com/watch?v=abc&t=12s"
    );
    assert_eq!(
        clean("https://youtu.be/abc?t=12s&si=xyz&feature=share"),
        "https://youtu.be/abc?t=12s"
    );
}

#[test]
fn quotes_unambiguous_paths_with_spaces_and_bails_out_on_prose() {
    assert_eq!(
        clean("/Users/peter/My Documents/file.txt"),
        "\"/Users/peter/My Documents/file.txt\""
    );
    assert_eq!(clean("./My Folder/file"), "\"./My Folder/file\"");
    assert_eq!(clean("/My File"), "\"/My File\"");
    assert_eq!(clean("~/My File"), "\"~/My File\"");
    assert_eq!(clean("../My File"), "\"../My File\"");
    assert_eq!(clean("/tmp/My File Name"), "\"/tmp/My File Name\"");
    assert_eq!(
        clean("~/Library/Application Support/Zentty"),
        "\"~/Library/Application Support/Zentty\""
    );
    for prose in [
        "TCP/IP is a protocol",
        "I/O throughput is fine",
        "scripts/agent bench/run",
        "/etc/hosts is the file you want",
        "./configure then run make again",
    ] {
        assert_eq!(clean(prose), prose);
    }
}

#[test]
fn markdown_reflow_joins_prose_and_preserves_structures() {
    assert!(is_likely_markdown("## Title\n\nBody"));
    assert!(is_likely_markdown("- one\n- two"));
    assert!(is_likely_markdown("```rust\nlet x = 1;\n```"));
    assert!(!is_likely_markdown("ordinary\nwrapped prose"));
    assert_eq!(
        reformat_markdown("One long\nparagraph split\nacross lines"),
        "One long paragraph split across lines"
    );
    let fenced = "Intro\n```rust\nlet x = 1;\nwrapped\nline\n```";
    assert_eq!(
        reformat_markdown(fenced),
        "Intro\n```rust\nlet x = 1;\nwrapped\nline\n```"
    );
    let table = "# Results\n| name | value |\n| --- | --- |\n| a | 1 |";
    assert_eq!(reformat_markdown(table), table);
    assert_eq!(
        reformat_markdown(
            "A wrapped\nparagraph before\na table.\n| name | value |\n| --- | --- |\n| a | 1 |"
        ),
        "A wrapped paragraph before a table.\n| name | value |\n| --- | --- |\n| a | 1 |"
    );
}

#[test]
fn cleaning_is_idempotent_and_reports_modification_truthfully() {
    let first = clean_copy(
        "  $ cargo test   \n  $ git status   ",
        CleanCopyOptions::default(),
    );
    let second = clean_copy(&first.text, CleanCopyOptions::default());
    assert!(first.was_modified);
    assert!(!second.was_modified);
    assert_eq!(second.text, first.text);
}

#[test]
fn dedent_preserves_relative_indentation_blank_lines_tabs_and_unicode() {
    assert_eq!(
        clean("    hello\n        nested\n    back"),
        "hello\n    nested\nback"
    );
    assert_eq!(clean("    hello\n  \n    world"), "hello\n\nworld");
    assert_eq!(clean("\t\thello\n\t\tworld"), "hello\nworld");
    assert_eq!(clean("hello\n  world"), "hello\n  world");
    assert_eq!(clean("    indented"), "indented");
    assert_eq!(clean("    こんにちは\n    世界"), "こんにちは\n世界");
}
