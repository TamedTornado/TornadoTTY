use crate::{ProductIpcError, ProductIpcKind, ProductIpcRequest};

const COLORS: &[&str] = &[
    "red", "orange", "amber", "yellow", "lime", "green", "teal", "cyan", "blue", "indigo",
    "purple", "pink",
];
const LAYOUTS: &[&str] = &[
    "full",
    "halves",
    "thirds",
    "quarters",
    "golden-wide",
    "golden-narrow",
    "golden-tall",
    "golden-short",
    "reset",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CliProductCommand {
    Version,
    ListColors,
    InstallIntegration(String),
    UninstallIntegration(String),
    Request(ProductIpcRequest),
}

/// Parses the public topology command families while leaving legacy helper
/// commands for the existing CLI dispatcher.
///
/// # Errors
///
/// Returns [`ProductIpcError`] when a recognized topology invocation is
/// ambiguous, malformed, or exceeds a protocol bound.
pub fn parse_product_cli(
    arguments: &[String],
) -> Result<Option<CliProductCommand>, ProductIpcError> {
    let Some(command) = arguments.first().map(String::as_str) else {
        return Ok(None);
    };
    let rest = &arguments[1..];
    match command {
        "version" => {
            require_empty(rest, "version")?;
            Ok(Some(CliProductCommand::Version))
        }
        "list" => parse_list(rest),
        "window" => parse_window(rest),
        "worklane" => parse_worklane(rest),
        "select" => parse_select(rest),
        "split" => parse_split(rest, None),
        "hsplit" => parse_split(rest, Some("right")),
        "vsplit" => parse_split(rest, Some("down")),
        "grid" => parse_grid(rest),
        "pane" => parse_pane(rest),
        "layout" => parse_layout(rest),
        "theme" => parse_theme(rest),
        "notify" => parse_notify(rest),
        "install" => parse_integration(rest, true),
        "uninstall" => parse_integration(rest, false),
        _ => Ok(None),
    }
}

fn parse_integration(
    arguments: &[String],
    install: bool,
) -> Result<Option<CliProductCommand>, ProductIpcError> {
    const TARGETS: &[&str] = &[
        "amp-hooks",
        "cursor-hooks",
        "droid-hooks",
        "kimi-hooks",
        "grok-hooks",
        "agy-hooks",
        "hermes-hooks",
        "vibe-hooks",
    ];
    let [target] = arguments else {
        return invalid(if install {
            "install requires exactly one integration target"
        } else {
            "uninstall requires exactly one integration target"
        });
    };
    if !TARGETS.contains(&target.as_str()) {
        return invalid(format!(
            "unknown integration target {target:?}; supported: {}",
            TARGETS.join(", ")
        ));
    }
    Ok(Some(if install {
        CliProductCommand::InstallIntegration(target.clone())
    } else {
        CliProductCommand::UninstallIntegration(target.clone())
    }))
}

fn parse_notify(arguments: &[String]) -> Result<Option<CliProductCommand>, ProductIpcError> {
    validate_options(
        arguments,
        &["--title", "--subtitle", "--body"],
        &["--no-inbox", "--silent"],
    )?;
    let title = option_value(arguments, "--title")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| command_error("notification title is required"))?;
    let mut canonical = vec!["--title".to_owned(), title.to_owned()];
    for option in ["--subtitle", "--body"] {
        if let Some(value) = option_value(arguments, option)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            canonical.extend([option.to_owned(), value.to_owned()]);
        }
    }
    for flag in ["--no-inbox", "--silent"] {
        if arguments.iter().any(|argument| argument == flag) {
            canonical.push(flag.to_owned());
        }
    }
    request(ProductIpcKind::Pane, "notify", canonical)
}

fn parse_list(arguments: &[String]) -> Result<Option<CliProductCommand>, ProductIpcError> {
    let (subcommand, rest) = match arguments.first().map(String::as_str) {
        Some("windows") => ("windows", &arguments[1..]),
        Some("worklanes") => ("worklanes", &arguments[1..]),
        Some("panes") => ("panes", &arguments[1..]),
        Some(value) if !value.starts_with('-') => {
            return invalid(format!("unknown list resource {value:?}"));
        }
        _ => ("overview", arguments),
    };
    validate_discovery_options(rest)?;
    request(ProductIpcKind::Discover, subcommand, rest.to_vec())
}

fn parse_window(arguments: &[String]) -> Result<Option<CliProductCommand>, ProductIpcError> {
    require_group_subcommand(arguments, "window", "list")?;
    let rest = &arguments[1..];
    validate_discovery_options(rest)?;
    request(ProductIpcKind::Discover, "windows", rest.to_vec())
}

fn parse_worklane(arguments: &[String]) -> Result<Option<CliProductCommand>, ProductIpcError> {
    let Some(subcommand) = arguments.first().map(String::as_str) else {
        return invalid("worklane requires list, rename, or color");
    };
    let rest = &arguments[1..];
    match subcommand {
        "list" => {
            validate_discovery_options(rest)?;
            request(ProductIpcKind::Discover, "worklanes", rest.to_vec())
        }
        "rename" => parse_rename(rest, "worklane-rename", "--id", "--id"),
        "color" => parse_worklane_color(rest),
        _ => invalid(format!("unknown worklane command {subcommand:?}")),
    }
}

fn parse_select(arguments: &[String]) -> Result<Option<CliProductCommand>, ProductIpcError> {
    require_group_subcommand(arguments, "select", "pane")?;
    let rest = &arguments[1..];
    validate_options(
        rest,
        &[
            "--window-id",
            "--worklane-id",
            "--pane-id",
            "--pane-index",
            "--output-version",
        ],
        &["--shell", "--include-control-token"],
    )?;
    validate_output_version(rest)?;
    request(ProductIpcKind::Discover, "select-pane", rest.to_vec())
}

fn parse_split(
    arguments: &[String],
    fixed_direction: Option<&str>,
) -> Result<Option<CliProductCommand>, ProductIpcError> {
    let mut index = 0;
    let direction = if let Some(direction) = fixed_direction {
        direction.to_owned()
    } else if arguments
        .first()
        .is_some_and(|value| !value.starts_with('-'))
    {
        index = 1;
        arguments[0].clone()
    } else {
        "right".to_owned()
    };
    if !["right", "left", "up", "down"].contains(&direction.as_str()) {
        return invalid(format!("invalid split direction {direction:?}"));
    }
    let options = &arguments[index..];
    validate_options(
        options,
        &[
            "--ratio",
            "--window-id",
            "--worklane-id",
            "--pane-id",
            "--pane-index",
            "--pane-token",
        ],
        &["--equal", "--golden", "--json"],
    )?;
    let layout_count = usize::from(options.iter().any(|value| value == "--equal"))
        + usize::from(options.iter().any(|value| value == "--golden"))
        + usize::from(option_value(options, "--ratio").is_some());
    if layout_count > 1 {
        return invalid("split layout options are mutually exclusive");
    }
    if let Some(ratio) = option_value(options, "--ratio") {
        let ratio = ratio
            .parse::<u8>()
            .map_err(|_| command_error("split ratio must be an integer from 1 through 99"))?;
        if !(1..=99).contains(&ratio) {
            return invalid("split ratio must be from 1 through 99");
        }
    }
    let mut canonical = vec![direction];
    canonical.extend_from_slice(options);
    request(ProductIpcKind::Pane, "split", canonical)
}

fn parse_grid(arguments: &[String]) -> Result<Option<CliProductCommand>, ProductIpcError> {
    let Some(dimensions) = arguments.first() else {
        return invalid("grid requires ROWSxCOLUMNS");
    };
    let Some((rows, columns)) = dimensions.split_once(['x', 'X']) else {
        return invalid("grid size must be ROWSxCOLUMNS");
    };
    let rows = rows
        .parse::<u8>()
        .map_err(|_| command_error("grid rows must be a positive integer"))?;
    let columns = columns
        .parse::<u8>()
        .map_err(|_| command_error("grid columns must be a positive integer"))?;
    if rows == 0 || columns == 0 || usize::from(rows) * usize::from(columns) > 36 {
        return invalid("grid must contain between 1 and 36 panes");
    }
    let (options, command) = arguments[1..]
        .iter()
        .position(|value| value == "--")
        .map_or((&arguments[1..], &[][..]), |index| {
            (&arguments[1..=index], &arguments[2 + index..])
        });
    validate_options(
        options,
        &[
            "--focus",
            "--window-id",
            "--worklane-id",
            "--pane-id",
            "--pane-index",
            "--pane-token",
        ],
        &["--new-only", "--include-source", "--json"],
    )?;
    if options.iter().any(|value| value == "--new-only")
        && options.iter().any(|value| value == "--include-source")
    {
        return invalid("--new-only and --include-source are mutually exclusive");
    }
    let focus = option_value(options, "--focus").unwrap_or("source");
    if !["source", "first", "last"].contains(&focus) {
        return invalid(format!("invalid grid focus {focus:?}"));
    }
    let window = option_value(options, "--window-id");
    let worklane = option_value(options, "--worklane-id");
    if window == Some("new") && worklane.is_some_and(|value| value != "new") {
        return invalid("--window-id new cannot target an existing worklane");
    }
    let mut canonical = vec![
        "--rows".to_owned(),
        rows.to_string(),
        "--columns".to_owned(),
        columns.to_string(),
    ];
    if options.iter().any(|value| value == "--new-only") {
        canonical.push("--new-only".to_owned());
    }
    canonical.extend(["--focus".to_owned(), focus.to_owned()]);
    let mut options = options.iter();
    while let Some(option) = options.next() {
        let option = option.as_str();
        if ["--new-only", "--include-source", "--focus", "--json"].contains(&option) {
            if option == "--json" {
                canonical.push(option.to_owned());
            }
            if option == "--focus" {
                let _ = options.next();
            }
            continue;
        }
        let value = options
            .next()
            .expect("validated grid value options contain a value");
        match (option, value.as_str()) {
            ("--window-id", "new") => canonical.push("--new-window".to_owned()),
            ("--worklane-id", "new") => canonical.push("--new-worklane".to_owned()),
            _ => canonical.extend([option.to_owned(), value.clone()]),
        }
    }
    if !command.is_empty() {
        if command
            .iter()
            .any(|argument| argument.contains(['\n', '\r']))
        {
            return invalid("grid command tokens may not contain line breaks");
        }
        let json = serde_json::to_string(command)
            .map_err(|error| command_error(format!("could not encode grid command: {error}")))?;
        canonical.extend(["--command-json".to_owned(), json]);
    }
    request(ProductIpcKind::Pane, "grid", canonical)
}

fn parse_pane(arguments: &[String]) -> Result<Option<CliProductCommand>, ProductIpcError> {
    let Some(subcommand) = arguments.first().map(String::as_str) else {
        return invalid("pane requires a subcommand");
    };
    let rest = &arguments[1..];
    match subcommand {
        "list" => {
            validate_discovery_options(rest)?;
            request(
                ProductIpcKind::Discover,
                "panes-current-worklane",
                rest.to_vec(),
            )
        }
        "focus" => parse_focus(rest),
        "rename" => parse_rename(rest, "pane-rename", "--pane-id", "--rename-pane-id"),
        "close" => parse_targeted("close", rest, true),
        "zoom" => parse_targeted("zoom", rest, false),
        "resize" => parse_resize(rest),
        _ => invalid(format!("unknown pane command {subcommand:?}")),
    }
}

fn parse_focus(arguments: &[String]) -> Result<Option<CliProductCommand>, ProductIpcError> {
    validate_targeted_options(arguments)?;
    let positional = arguments.first().filter(|value| !value.starts_with('-'));
    if positional.is_some()
        && (option_value(arguments, "--pane-id").is_some()
            || option_value(arguments, "--pane-index").is_some())
        && !["left", "right", "up", "down"].contains(&positional.unwrap().as_str())
    {
        return invalid("positional pane target conflicts with explicit pane selector");
    }
    request(ProductIpcKind::Pane, "focus", arguments.to_vec())
}

fn parse_targeted(
    subcommand: &str,
    arguments: &[String],
    allows_positional: bool,
) -> Result<Option<CliProductCommand>, ProductIpcError> {
    validate_targeted_options(arguments)?;
    if !allows_positional
        && arguments
            .first()
            .is_some_and(|value| !value.starts_with('-'))
    {
        return invalid(format!("{subcommand} does not accept a positional target"));
    }
    request(ProductIpcKind::Pane, subcommand, arguments.to_vec())
}

fn parse_resize(arguments: &[String]) -> Result<Option<CliProductCommand>, ProductIpcError> {
    let Some(target) = arguments.first().filter(|value| !value.starts_with('-')) else {
        return invalid("pane resize requires a direction or percentage");
    };
    let valid = ["left", "right", "up", "down"].contains(&target.as_str())
        || target
            .strip_suffix('%')
            .and_then(|value| value.parse::<u8>().ok())
            .is_some_and(|value| (1..=99).contains(&value));
    if !valid {
        return invalid(format!("invalid pane resize target {target:?}"));
    }
    validate_targeted_options(&arguments[1..])?;
    request(ProductIpcKind::Pane, "resize", arguments.to_vec())
}

fn parse_rename(
    arguments: &[String],
    subcommand: &str,
    source_id_option: &str,
    wire_id_option: &str,
) -> Result<Option<CliProductCommand>, ProductIpcError> {
    let clear = arguments.iter().any(|value| value == "--clear");
    let title = arguments.first().filter(|value| !value.starts_with('-'));
    if clear == title.is_some() {
        return invalid("provide exactly one title or --clear");
    }
    let option_start = usize::from(title.is_some());
    validate_options(
        &arguments[option_start..],
        &[source_id_option, "--worklane-id"],
        &["--clear"],
    )?;
    let mut canonical = if clear {
        vec!["--clear".to_owned()]
    } else {
        vec!["--title".to_owned(), title.unwrap().clone()]
    };
    if let Some(id) = option_value(arguments, source_id_option) {
        canonical.extend([wire_id_option.to_owned(), id.to_owned()]);
    }
    if let Some(id) = option_value(arguments, "--worklane-id") {
        canonical.extend(["--id".to_owned(), id.to_owned()]);
    }
    request(ProductIpcKind::Pane, subcommand, canonical)
}

fn parse_worklane_color(
    arguments: &[String],
) -> Result<Option<CliProductCommand>, ProductIpcError> {
    if arguments == ["--list"] {
        return Ok(Some(CliProductCommand::ListColors));
    }
    let Some(color) = arguments.first().filter(|value| !value.starts_with('-')) else {
        return invalid("worklane color requires a color or --list");
    };
    if !COLORS.contains(&color.as_str()) && !["reset", "default"].contains(&color.as_str()) {
        return invalid(format!("unknown worklane color {color:?}"));
    }
    validate_options(&arguments[1..], &["--id"], &[])?;
    let mut canonical = vec![
        "--color".to_owned(),
        if color == "default" {
            "reset".to_owned()
        } else {
            color.clone()
        },
    ];
    if let Some(id) = option_value(&arguments[1..], "--id") {
        canonical.extend(["--id".to_owned(), id.to_owned()]);
    }
    request(ProductIpcKind::Pane, "worklane-color", canonical)
}

fn parse_layout(arguments: &[String]) -> Result<Option<CliProductCommand>, ProductIpcError> {
    let Some(preset) = arguments.first().filter(|value| !value.starts_with('-')) else {
        return invalid("layout requires a preset");
    };
    if !LAYOUTS.contains(&preset.as_str()) {
        return invalid(format!("unknown layout preset {preset:?}"));
    }
    validate_options(
        &arguments[1..],
        &[
            "--window-id",
            "--worklane-id",
            "--pane-id",
            "--pane-index",
            "--pane-token",
        ],
        &["--vertical", "-v", "--json"],
    )?;
    request(ProductIpcKind::Pane, "layout", arguments.to_vec())
}

fn parse_theme(arguments: &[String]) -> Result<Option<CliProductCommand>, ProductIpcError> {
    let Some(command) = arguments.first().map(String::as_str) else {
        return invalid("theme requires toggle, dark, light, or auto");
    };
    if !["toggle", "dark", "light", "auto"].contains(&command) {
        return invalid(format!("unknown theme command {command:?}"));
    }
    require_empty(&arguments[1..], "theme")?;
    request(ProductIpcKind::Pane, "theme", vec![command.to_owned()])
}

fn validate_discovery_options(arguments: &[String]) -> Result<(), ProductIpcError> {
    validate_options(
        arguments,
        &["--window-id", "--worklane-id", "--output-version"],
        &["--include-control-token", "--json"],
    )?;
    validate_output_version(arguments)
}

fn validate_output_version(arguments: &[String]) -> Result<(), ProductIpcError> {
    if let Some(version) = option_value(arguments, "--output-version")
        && version != "1"
    {
        return invalid(format!(
            "unsupported output version {version:?}; supported: 1"
        ));
    }
    Ok(())
}

fn validate_targeted_options(arguments: &[String]) -> Result<(), ProductIpcError> {
    let start = usize::from(
        arguments
            .first()
            .is_some_and(|value| !value.starts_with('-')),
    );
    validate_options(
        &arguments[start..],
        &[
            "--window-id",
            "--worklane-id",
            "--pane-id",
            "--pane-index",
            "--pane-token",
        ],
        &["--json"],
    )
}

fn validate_options(
    arguments: &[String],
    value_options: &[&str],
    flags: &[&str],
) -> Result<(), ProductIpcError> {
    let mut index = 0;
    let mut seen = std::collections::BTreeSet::new();
    while index < arguments.len() {
        let argument = arguments[index].as_str();
        if flags.contains(&argument) {
            if !seen.insert(argument) {
                return invalid(format!("duplicate option {argument}"));
            }
            index += 1;
            continue;
        }
        if value_options.contains(&argument) {
            if !seen.insert(argument) {
                return invalid(format!("duplicate option {argument}"));
            }
            if arguments
                .get(index + 1)
                .is_none_or(|value| value.starts_with('-'))
            {
                return invalid(format!("missing value for {argument}"));
            }
            if argument == "--pane-index"
                && arguments[index + 1]
                    .parse::<usize>()
                    .ok()
                    .is_none_or(|value| value == 0)
            {
                return invalid("--pane-index must be a positive integer");
            }
            index += 2;
            continue;
        }
        return invalid(format!("unexpected argument {argument:?}"));
    }
    Ok(())
}

fn option_value<'a>(arguments: &'a [String], option: &str) -> Option<&'a str> {
    arguments
        .windows(2)
        .find_map(|pair| (pair[0] == option).then_some(pair[1].as_str()))
}

fn require_group_subcommand(
    arguments: &[String],
    group: &str,
    expected: &str,
) -> Result<(), ProductIpcError> {
    if arguments.first().map(String::as_str) != Some(expected) {
        return invalid(format!("{group} requires {expected}"));
    }
    Ok(())
}

fn require_empty(arguments: &[String], command: &str) -> Result<(), ProductIpcError> {
    if arguments.is_empty() {
        Ok(())
    } else {
        invalid(format!("{command} does not accept arguments"))
    }
}

fn request(
    kind: ProductIpcKind,
    subcommand: &str,
    arguments: Vec<String>,
) -> Result<Option<CliProductCommand>, ProductIpcError> {
    Ok(Some(CliProductCommand::Request(ProductIpcRequest::new(
        kind, subcommand, arguments,
    )?)))
}

fn command_error(message: impl Into<String>) -> ProductIpcError {
    ProductIpcError::InvalidCommand(message.into())
}

fn invalid<T>(message: impl Into<String>) -> Result<T, ProductIpcError> {
    Err(command_error(message))
}
