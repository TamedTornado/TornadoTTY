use super::ApplicationShell;
use std::cell::RefCell;
use std::rc::Rc;
use zentty_agent_ipc::{ProductIpcReply, ProductIpcRequest};
use zentty_core::{AgentTarget, PaneRecipe, PaneResizeDirection, ThemeModeCommand, WorklaneColor};

pub(crate) struct DiscoveryRows {
    pub(crate) window: serde_json::Value,
    pub(crate) worklanes: Vec<serde_json::Value>,
    pub(crate) panes: Vec<serde_json::Value>,
}

impl ApplicationShell {
    pub(crate) fn product_grid_source_pane(&self, target: &AgentTarget) -> Option<PaneRecipe> {
        let pane = self.state.pane(&target.pane_id)?;
        (self.state.worklane_id_for_pane(&target.pane_id) == Some(target.worklane_id.as_str()))
            .then(|| PaneRecipe {
                id: pane.id.clone(),
                custom_title: pane.custom_title.clone(),
                title_seed: (pane.live_title != "shell").then(|| pane.live_title.clone()),
                working_directory: pane.working_directory.clone(),
                last_activity_title: None,
                last_run_command: pane.last_run_command.clone(),
            })
    }

    pub(crate) fn execute_product_command(
        shell: &Rc<RefCell<Self>>,
        target: &AgentTarget,
        request: &ProductIpcRequest,
    ) -> ProductIpcReply {
        let result = Self::execute_product_command_inner(shell, target, request);
        result.map_or_else(
            |(code, message)| {
                ProductIpcReply::failure(code, message).expect("bounded product diagnostic")
            },
            |stdout| ProductIpcReply::success(stdout).expect("bounded product output"),
        )
    }

    fn execute_product_command_inner(
        shell: &Rc<RefCell<Self>>,
        target: &AgentTarget,
        request: &ProductIpcRequest,
    ) -> Result<String, (&'static str, String)> {
        eprintln!(
            "zentty-linux: product-command subcommand={} target={} selector-pane={} selector-index={}",
            request.subcommand(),
            target.pane_id,
            option_value(request.arguments(), "--pane-id").unwrap_or("none"),
            option_value(request.arguments(), "--pane-index").unwrap_or("none"),
        );
        if shell.borrow().window_template.id != target.window_id {
            return Err(("stale_target", "target window is unavailable".to_owned()));
        }
        if !target_exists(&shell.borrow(), target) {
            return Err(("stale_target", "target pane is unavailable".to_owned()));
        }
        validate_authenticated_selectors(
            &shell.borrow(),
            target,
            request.arguments(),
            matches!(request.subcommand(), "focus" | "close"),
        )?;
        match request.subcommand() {
            "split" => apply_split_command(shell, target, request.arguments())?,
            "focus" => apply_focus_command(shell, target, request.arguments())?,
            "pane-rename" | "worklane-rename" | "worklane-color" => {
                apply_metadata_command(shell, target, request)?;
            }
            "close" => Self::close_pane(shell, &target.pane_id),
            "resize" => apply_resize_command(shell, target, request.arguments())?,
            "layout" => {
                select_authenticated_target(shell, target)?;
                apply_layout(shell, request.arguments())?;
            }
            "theme" => return apply_theme_command(shell, request.arguments()),
            "grid" => {
                select_authenticated_target(shell, target)?;
                if request
                    .arguments()
                    .iter()
                    .any(|argument| argument == "--new-window")
                {
                    return Err((
                        "coordinator_required",
                        "new-window grid must be handled by the application coordinator".to_owned(),
                    ));
                }
                apply_grid(shell, request.arguments())?;
            }
            "zoom" => Self::toggle_product_zoom(shell),
            value => {
                return Err((
                    "unsupported_command",
                    format!("unsupported product command {value:?}"),
                ));
            }
        }
        Ok(String::new())
    }

    pub(crate) fn product_discovery_rows(
        &self,
        order: usize,
        is_focused_window: bool,
        include_control_tokens: bool,
    ) -> DiscoveryRows {
        let pane_count = self
            .state
            .worklanes()
            .iter()
            .flat_map(|worklane| &worklane.columns)
            .map(|column| column.panes.len())
            .sum::<usize>();
        let window = serde_json::json!({
            "id": self.window_template.id,
            "order": order,
            "isFocused": is_focused_window,
            "worklaneCount": self.state.worklanes().len(),
            "paneCount": pane_count,
        });
        let mut worklanes = Vec::new();
        let mut panes = Vec::new();
        for (worklane_index, worklane) in self.state.worklanes().iter().enumerate() {
            let focused_pane_id = worklane
                .columns
                .iter()
                .find(|column| column.id == worklane.focused_column_id)
                .map(|column| column.focused_pane_id.clone());
            let worklane_pane_count = worklane
                .columns
                .iter()
                .map(|column| column.panes.len())
                .sum::<usize>();
            let mut worklane_row = serde_json::json!({
                "id": worklane.id,
                "windowID": self.window_template.id,
                "order": worklane_index + 1,
                "title": worklane.title,
                "isFocused": self.state.active_worklane_id() == worklane.id,
                "paneCount": worklane_pane_count,
                "columnCount": worklane.columns.len(),
                "focusedPaneID": focused_pane_id,
            });
            remove_null_fields(&mut worklane_row);
            worklanes.push(worklane_row);
            let mut pane_index = 0_usize;
            for (column_index, column) in worklane.columns.iter().enumerate() {
                for pane in &column.panes {
                    pane_index += 1;
                    let control_token = include_control_tokens
                        .then(|| self.agent_events.control_token_for_pane(&pane.id))
                        .flatten();
                    let mut pane_row = serde_json::json!({
                        "id": pane.id,
                        "windowID": self.window_template.id,
                        "worklaneID": worklane.id,
                        "index": pane_index,
                        "column": column_index + 1,
                        "title": pane.display_title(),
                        "workingDirectory": pane.working_directory,
                        "isFocused": self.state.active_worklane_id() == worklane.id
                            && focused_pane_id.as_deref() == Some(pane.id.as_str()),
                        "agentTool": serde_json::Value::Null,
                        "agentStatus": serde_json::Value::Null,
                        "controlToken": control_token,
                    });
                    remove_null_fields(&mut pane_row);
                    panes.push(pane_row);
                }
            }
        }
        DiscoveryRows {
            window,
            worklanes,
            panes,
        }
    }
}

fn remove_null_fields(value: &mut serde_json::Value) {
    if let Some(object) = value.as_object_mut() {
        object.retain(|_, value| !value.is_null());
    }
}

fn apply_split_command(
    shell: &Rc<RefCell<ApplicationShell>>,
    target: &AgentTarget,
    arguments: &[String],
) -> Result<(), (&'static str, String)> {
    select_authenticated_target(shell, target)?;
    let direction = arguments.first().map_or("right", String::as_str);
    match direction {
        "right" => ApplicationShell::split_focused_pane_right(shell),
        "left" => ApplicationShell::add_focused_pane_left(shell),
        "down" => ApplicationShell::split_focused_pane_below(shell),
        "up" => ApplicationShell::split_focused_pane(shell, "cli-split-up", |state, pane_id| {
            state.insert_focused_pane_above(pane_id)
        }),
        _ => Err(format!("invalid split direction {direction:?}")),
    }
    .map_err(|message| ("split_failed", message))?;
    apply_split_layout(shell, direction, arguments)
}

fn apply_focus_command(
    shell: &Rc<RefCell<ApplicationShell>>,
    target: &AgentTarget,
    arguments: &[String],
) -> Result<(), (&'static str, String)> {
    select_authenticated_target(shell, target)?;
    let positional = arguments
        .first()
        .filter(|value| !value.starts_with('-'))
        .map(String::as_str);
    let changed = match positional {
        Some("left") => shell.borrow_mut().state.focus_pane_left(),
        Some("right") => shell.borrow_mut().state.focus_pane_right(),
        Some("up") => shell.borrow_mut().state.focus_pane_up(),
        Some("down") => shell.borrow_mut().state.focus_pane_down(),
        Some(_) | None => true,
    };
    if !changed {
        return Err((
            "target_not_found",
            "no pane exists in that direction".to_owned(),
        ));
    }
    shell.borrow().render();
    shell.borrow().scroll_panes_to_focused();
    shell.borrow().focus_selected_surface();
    Ok(())
}

fn apply_metadata_command(
    shell: &Rc<RefCell<ApplicationShell>>,
    target: &AgentTarget,
    request: &ProductIpcRequest,
) -> Result<(), (&'static str, String)> {
    let arguments = request.arguments();
    let changed = match request.subcommand() {
        "pane-rename" => {
            if option_value(arguments, "--rename-pane-id")
                .is_some_and(|pane_id| pane_id != target.pane_id)
            {
                return Err((
                    "unauthorized_target",
                    "pane selector does not match the authenticated token".to_owned(),
                ));
            }
            shell
                .borrow_mut()
                .state
                .set_pane_custom_title(&target.pane_id, option_value(arguments, "--title"))
        }
        "worklane-rename" | "worklane-color" => {
            if option_value(arguments, "--id")
                .is_some_and(|worklane_id| worklane_id != target.worklane_id)
            {
                return Err((
                    "unauthorized_target",
                    "worklane selector does not match the authenticated token".to_owned(),
                ));
            }
            if request.subcommand() == "worklane-rename" {
                shell
                    .borrow_mut()
                    .state
                    .set_worklane_title(&target.worklane_id, option_value(arguments, "--title"))
            } else {
                let color = option_value(arguments, "--color")
                    .and_then(|value| (value != "reset").then(|| WorklaneColor::named(value)))
                    .flatten();
                shell
                    .borrow_mut()
                    .state
                    .set_worklane_color(&target.worklane_id, color)
            }
        }
        _ => unreachable!("metadata dispatcher receives only metadata commands"),
    };
    if !changed {
        return Err(("unchanged", "metadata did not change".to_owned()));
    }
    shell.borrow().render();
    Ok(())
}

fn apply_resize_command(
    shell: &Rc<RefCell<ApplicationShell>>,
    target: &AgentTarget,
    arguments: &[String],
) -> Result<(), (&'static str, String)> {
    select_authenticated_target(shell, target)?;
    let value = arguments
        .first()
        .map(String::as_str)
        .ok_or_else(|| ("invalid_request", "resize direction is missing".to_owned()))?;
    if let Some(percentage) = value.strip_suffix('%') {
        return apply_percentage_resize(shell, percentage);
    }
    let direction = match value {
        "left" => PaneResizeDirection::Left,
        "right" => PaneResizeDirection::Right,
        "up" => PaneResizeDirection::Up,
        "down" => PaneResizeDirection::Down,
        _ => return Err(("invalid_request", "invalid resize direction".to_owned())),
    };
    if !shell.borrow_mut().resize_focused_pane_by_cell(direction) {
        return Err(("resize_failed", "pane could not be resized".to_owned()));
    }
    shell.borrow().render();
    shell.borrow().focus_selected_surface();
    Ok(())
}

fn apply_percentage_resize(
    shell: &Rc<RefCell<ApplicationShell>>,
    percentage: &str,
) -> Result<(), (&'static str, String)> {
    let fraction = percentage
        .parse::<f64>()
        .map(|value| value / 100.0)
        .map_err(|_| ("invalid_request", "invalid resize percentage".to_owned()))?;
    let mut shell = shell.borrow_mut();
    shell.materialize_active_column_widths();
    let viewport = f64::from(shell.pane_viewport_width());
    let focused_pane = shell
        .state
        .focused_pane_id()
        .map(str::to_owned)
        .ok_or_else(|| ("stale_target", "focused pane is unavailable".to_owned()))?;
    let focused_column = shell.state.active_worklane().focused_column_id.clone();
    let minimum = shell.minimum_column_width(&focused_column);
    let width = (viewport * fraction.clamp(0.05, 0.95)).clamp(minimum, viewport.max(minimum));
    if !shell.state.restore_column_width(&focused_pane, width) {
        return Err(("resize_failed", "pane width did not change".to_owned()));
    }
    eprintln!(
        "zentty-linux: cli-pane-resize pane={focused_pane} percentage={} width={width:.3}",
        fraction * 100.0
    );
    shell.render();
    shell.focus_selected_surface();
    Ok(())
}

fn apply_theme_command(
    shell: &Rc<RefCell<ApplicationShell>>,
    arguments: &[String],
) -> Result<String, (&'static str, String)> {
    let command = match arguments.first().map(String::as_str) {
        Some("toggle") => ThemeModeCommand::Toggle,
        Some("dark") => ThemeModeCommand::Dark,
        Some("light") => ThemeModeCommand::Light,
        Some("auto") => ThemeModeCommand::Automatic,
        _ => return Err(("invalid_request", "invalid theme command".to_owned())),
    };
    let mode = shell
        .borrow_mut()
        .apply_theme_mode_command(command)
        .map_err(|message| ("theme_failed", message))?;
    Ok(format!("{}\n", mode.cli_token()))
}

fn target_exists(shell: &ApplicationShell, target: &AgentTarget) -> bool {
    shell.state.worklanes().iter().any(|worklane| {
        worklane.id == target.worklane_id
            && worklane
                .columns
                .iter()
                .flat_map(|column| &column.panes)
                .any(|pane| pane.id == target.pane_id)
    })
}

fn select_authenticated_target(
    shell: &Rc<RefCell<ApplicationShell>>,
    target: &AgentTarget,
) -> Result<(), (&'static str, String)> {
    if shell
        .borrow_mut()
        .state
        .select_worklane_and_pane(&target.worklane_id, &target.pane_id)
    {
        Ok(())
    } else {
        Err(("stale_target", "target pane is unavailable".to_owned()))
    }
}

fn apply_split_layout(
    shell: &Rc<RefCell<ApplicationShell>>,
    direction: &str,
    arguments: &[String],
) -> Result<(), (&'static str, String)> {
    let equal = arguments.iter().any(|argument| argument == "--equal");
    let golden = arguments.iter().any(|argument| argument == "--golden");
    let ratio = option_value(arguments, "--ratio")
        .map(|value| {
            value
                .parse::<f64>()
                .map(|value| value / 100.0)
                .map_err(|_| ("invalid_request", "invalid split ratio".to_owned()))
        })
        .transpose()?;
    if !equal && !golden && ratio.is_none() {
        return Ok(());
    }
    let horizontal = matches!(direction, "left" | "right");
    let mut shell = shell.borrow_mut();
    let viewport = f64::from(shell.pane_viewport_width());
    let changed = match (horizontal, equal, golden, ratio) {
        (true, true, _, _) => shell.state.arrange_columns(2, viewport),
        (true, _, true, _) => shell.state.arrange_golden_width(true, viewport),
        (true, _, _, Some(fraction)) => {
            let pane_id = shell
                .state
                .focused_pane_id()
                .map(str::to_owned)
                .ok_or_else(|| ("stale_target", "new split pane is unavailable".to_owned()))?;
            shell
                .state
                .restore_column_width(&pane_id, viewport * fraction.clamp(0.05, 0.95))
        }
        (false, true, _, _) => {
            let pane_id = shell
                .state
                .focused_pane_id()
                .map(str::to_owned)
                .ok_or_else(|| ("stale_target", "new split pane is unavailable".to_owned()))?;
            shell.state.equalize_pane_heights_in_column(&pane_id)
        }
        (false, _, true, _) => shell.state.arrange_golden_height(true),
        (false, _, _, Some(fraction)) => {
            shell.state.resize_focused_pane_height_to_fraction(fraction)
        }
        _ => false,
    };
    if changed {
        shell.render();
        shell.focus_selected_surface();
    }
    let mode = if equal {
        "equal".to_owned()
    } else if golden {
        "golden".to_owned()
    } else {
        format!("ratio-{:.0}", ratio.unwrap_or_default() * 100.0)
    };
    eprintln!("zentty-linux: cli-split-layout direction={direction} mode={mode} changed={changed}");
    Ok(())
}

fn validate_authenticated_selectors(
    shell: &ApplicationShell,
    target: &AgentTarget,
    arguments: &[String],
    positional_pane_selector: bool,
) -> Result<(), (&'static str, String)> {
    for (option, expected) in [
        ("--window-id", target.window_id.as_str()),
        ("--worklane-id", target.worklane_id.as_str()),
        ("--pane-id", target.pane_id.as_str()),
    ] {
        if option_value(arguments, option).is_some_and(|actual| actual != expected) {
            return Err((
                "unauthorized_target",
                format!("{option} does not match the authenticated pane capability"),
            ));
        }
    }
    let pane_index = shell
        .state
        .worklanes()
        .iter()
        .find(|worklane| worklane.id == target.worklane_id)
        .into_iter()
        .flat_map(|worklane| &worklane.columns)
        .flat_map(|column| &column.panes)
        .position(|pane| pane.id == target.pane_id)
        .map(|index| index + 1)
        .ok_or_else(|| ("stale_target", "target pane is unavailable".to_owned()))?;
    if let Some(raw_index) = option_value(arguments, "--pane-index") {
        let selected_index = raw_index.parse::<usize>().map_err(|_| {
            (
                "invalid_request",
                format!("invalid pane index {raw_index:?}"),
            )
        })?;
        if selected_index != pane_index {
            return Err((
                "unauthorized_target",
                "--pane-index does not match the authenticated pane capability".to_owned(),
            ));
        }
    }
    if positional_pane_selector
        && let Some(selector) = arguments.first().filter(|value| !value.starts_with('-'))
        && !["left", "right", "up", "down"].contains(&selector.as_str())
    {
        let matches = selector == &target.pane_id
            || selector
                .parse::<usize>()
                .is_ok_and(|selected_index| selected_index == pane_index);
        if !matches {
            return Err((
                "unauthorized_target",
                "positional pane selector does not match the authenticated pane capability"
                    .to_owned(),
            ));
        }
    }
    Ok(())
}

fn apply_layout(
    shell: &Rc<RefCell<ApplicationShell>>,
    arguments: &[String],
) -> Result<(), (&'static str, String)> {
    let preset = arguments.first().map_or("reset", String::as_str);
    let vertical = arguments
        .iter()
        .any(|argument| argument == "--vertical" || argument == "-v");
    let mut shell = shell.borrow_mut();
    let viewport = f64::from(shell.pane_viewport_width());
    let changed = match (preset, vertical) {
        ("full", _) => shell.state.arrange_columns(1, viewport),
        ("halves", false) => shell.state.arrange_columns(2, viewport),
        ("thirds", false) => shell.state.arrange_columns(3, viewport),
        ("quarters", false) => shell.state.arrange_columns(4, viewport),
        ("halves", true) => shell.state.arrange_panes_per_column(2),
        ("thirds", true) => shell.state.arrange_panes_per_column(3),
        ("quarters", true) => shell.state.arrange_panes_per_column(4),
        ("golden-wide", _) => shell.state.arrange_golden_width(true, viewport),
        ("golden-narrow", _) => shell.state.arrange_golden_width(false, viewport),
        ("golden-tall", _) => shell.state.arrange_golden_height(true),
        ("golden-short", _) => shell.state.arrange_golden_height(false),
        ("reset", _) => shell.state.reset_active_layout(viewport),
        _ => {
            return Err((
                "invalid_request",
                format!("invalid layout preset {preset:?}"),
            ));
        }
    };
    if changed {
        shell.render();
        shell.focus_selected_surface();
    }
    Ok(())
}

fn apply_grid(
    shell: &Rc<RefCell<ApplicationShell>>,
    arguments: &[String],
) -> Result<(), (&'static str, String)> {
    let (rows, columns) = parse_grid_dimensions(arguments)?;
    let command_text = parse_grid_command(arguments)?;
    if rows == 0 || columns == 0 || rows.saturating_mul(columns) > 36 {
        return Err((
            "invalid_request",
            "grid must contain between 1 and 36 panes".to_owned(),
        ));
    }
    if arguments
        .iter()
        .any(|argument| argument == "--new-worklane")
    {
        ApplicationShell::create_worklane(shell).map_err(|message| ("grid_failed", message))?;
    }
    let source = shell
        .borrow()
        .state
        .focused_pane_id()
        .map(str::to_owned)
        .ok_or_else(|| ("stale_target", "grid source pane is unavailable".to_owned()))?;
    let worklane_id = shell.borrow().state.active_worklane_id().to_owned();
    let mut created = Vec::new();
    let mut column_leaders = vec![source.clone()];
    for _ in 1..columns {
        let leader = column_leaders.last().expect("source leader exists").clone();
        let _ = shell
            .borrow_mut()
            .state
            .select_worklane_and_pane(&worklane_id, &leader);
        ApplicationShell::split_focused_pane_right(shell)
            .map_err(|message| ("grid_failed", message))?;
        let pane_id = shell
            .borrow()
            .state
            .focused_pane_id()
            .expect("split focuses its new pane")
            .to_owned();
        created.push(pane_id.clone());
        column_leaders.push(pane_id);
    }
    for leader in &column_leaders {
        let _ = shell
            .borrow_mut()
            .state
            .select_worklane_and_pane(&worklane_id, leader);
        for _ in 1..rows {
            ApplicationShell::split_focused_pane_below(shell)
                .map_err(|message| ("grid_failed", message))?;
            created.push(
                shell
                    .borrow()
                    .state
                    .focused_pane_id()
                    .expect("split focuses its new pane")
                    .to_owned(),
            );
        }
    }
    if let Some(text) = command_text {
        send_grid_command(shell, arguments, &source, &created, &text)?;
    }
    let focus = match option_value(arguments, "--focus").unwrap_or("source") {
        "first" => column_leaders.first().unwrap_or(&source),
        "last" => created.last().unwrap_or(&source),
        _ => &source,
    };
    let _ = shell
        .borrow_mut()
        .state
        .select_worklane_and_pane(&worklane_id, focus);
    shell.borrow().render();
    shell.borrow().scroll_panes_to_focused();
    shell.borrow().focus_selected_surface();
    eprintln!(
        "zentty-linux: cli-grid worklane={worklane_id} rows={rows} columns={columns} source={source} created={}",
        created.len()
    );
    Ok(())
}

fn parse_grid_dimensions(arguments: &[String]) -> Result<(usize, usize), (&'static str, String)> {
    let rows = option_value(arguments, "--rows")
        .and_then(|value| value.parse::<usize>().ok())
        .ok_or_else(|| ("invalid_request", "grid rows are missing".to_owned()))?;
    let columns = option_value(arguments, "--columns")
        .and_then(|value| value.parse::<usize>().ok())
        .ok_or_else(|| ("invalid_request", "grid columns are missing".to_owned()))?;
    Ok((rows, columns))
}

fn parse_grid_command(arguments: &[String]) -> Result<Option<String>, (&'static str, String)> {
    let Some(command_json) = option_value(arguments, "--command-json") else {
        return Ok(None);
    };
    let command = serde_json::from_str::<Vec<String>>(command_json)
        .map_err(|error| ("invalid_request", format!("invalid grid command: {error}")))?;
    if command.is_empty() {
        return Err(("invalid_request", "grid command is empty".to_owned()));
    }
    Ok(Some(
        command
            .iter()
            .map(|argument| shell_quote(argument))
            .collect::<Vec<_>>()
            .join(" ")
            + "\n",
    ))
}

fn send_grid_command(
    shell: &Rc<RefCell<ApplicationShell>>,
    arguments: &[String],
    source: &str,
    created: &[String],
    text: &str,
) -> Result<(), (&'static str, String)> {
    let mut destinations = created.to_vec();
    if !arguments.iter().any(|argument| argument == "--new-only") {
        destinations.push(source.to_owned());
    }
    for pane_id in destinations {
        let shell_ref = shell.borrow();
        let surface = shell_ref.pane_runtime.surface(&pane_id).ok_or_else(|| {
            (
                "grid_failed",
                format!("grid pane {pane_id:?} has no live terminal"),
            )
        })?;
        surface
            .send_text(text)
            .map_err(|error| ("grid_failed", error.to_string()))?;
    }
    Ok(())
}

fn shell_quote(value: &str) -> String {
    if value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || b"_+-./:@%=".contains(&byte))
    {
        value.to_owned()
    } else {
        format!("'{}'", value.replace('\'', "'\"'\"'"))
    }
}

fn option_value<'a>(arguments: &'a [String], option: &str) -> Option<&'a str> {
    arguments
        .windows(2)
        .find_map(|pair| (pair[0] == option).then_some(pair[1].as_str()))
}
