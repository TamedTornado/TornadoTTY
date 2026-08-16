use super::ApplicationShell;
use super::agent_lifecycle_signal::{AgentLifecycleSignal, parse_agent_lifecycle_signal};
use super::shell_signal::{ShellSignal, parse_shell_signal};
use std::cell::RefCell;
use std::rc::Rc;
use zentty_api::{ApplicationReply as ProductIpcReply, ApplicationRequest as ProductIpcRequest};
use zentty_core::{
    AgentTarget, PaneLayoutPolicy, PaneRecipe, PaneResizeDirection, ThemeModeCommand, WorklaneColor,
};

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
            "split" => {
                let before = pane_ids(&shell.borrow());
                apply_split_command(shell, target, request.arguments())?;
                let created = created_pane_ids(&shell.borrow(), &before);
                return topology_response(
                    &shell.borrow(),
                    "split",
                    &target.pane_id,
                    &created,
                    request.arguments(),
                );
            }
            "focus" => apply_focus_command(shell, target, request.arguments())?,
            "pane-rename" | "worklane-rename" | "worklane-color" => {
                apply_metadata_command(shell, target, request)?;
            }
            "close" => Self::close_pane(shell, &target.pane_id),
            "resize" => {
                apply_resize_command(shell, target, request.arguments())?;
                return topology_response(
                    &shell.borrow(),
                    "resize",
                    &target.pane_id,
                    &[],
                    request.arguments(),
                );
            }
            "layout" => {
                select_authenticated_target(shell, target)?;
                apply_layout(shell, request.arguments())?;
                return topology_response(
                    &shell.borrow(),
                    "layout",
                    &target.pane_id,
                    &[],
                    request.arguments(),
                );
            }
            "theme" => return apply_theme_command(shell, request.arguments()),
            "shell-signal" => return apply_shell_signal(shell, target, request.arguments()),
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
                let grid = apply_grid(shell, request.arguments())?;
                return topology_response(
                    &shell.borrow(),
                    "grid",
                    &grid.source_pane_id,
                    &grid.created_pane_ids,
                    request.arguments(),
                );
            }
            "zoom" => {
                Self::toggle_product_zoom(shell);
                return topology_response(
                    &shell.borrow(),
                    "zoom",
                    &target.pane_id,
                    &[],
                    request.arguments(),
                );
            }
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

fn apply_shell_signal(
    shell: &Rc<RefCell<ApplicationShell>>,
    target: &AgentTarget,
    arguments: &[String],
) -> Result<String, (&'static str, String)> {
    if arguments
        .first()
        .is_some_and(|kind| matches!(kind.as_str(), "lifecycle" | "pid"))
    {
        return apply_agent_lifecycle_signal(shell, target, arguments);
    }
    let signal = parse_shell_signal(arguments)?;
    match &signal {
        ShellSignal::State { state, command } => {
            if state == "running"
                && let Some(command) = command
            {
                let working_directory = shell
                    .borrow()
                    .state
                    .pane(&target.pane_id)
                    .and_then(|pane| pane.working_directory.clone());
                shell.borrow_mut().state.configure_pane_launch(
                    &target.pane_id,
                    working_directory,
                    Some(command.clone()),
                );
            }
            eprintln!(
                "zentty-linux: shell-signal pane={} kind=shell-state state={} command-present={}",
                target.pane_id,
                state,
                command.is_some()
            );
        }
        ShellSignal::RootPid { event, pid } => {
            eprintln!(
                "zentty-linux: shell-signal pane={} kind=pane-root-pid event={} pid={}",
                target.pane_id,
                event,
                pid.map_or_else(|| "none".to_owned(), |pid| pid.to_string())
            );
        }
        ShellSignal::Context { scope, path } => {
            // A remote path is display context for an SSH process, not a
            // locally launchable directory. Preserve the pane's last local
            // launch directory until the richer remote-context model lands.
            if scope != "remote" {
                let last_run_command = shell
                    .borrow()
                    .state
                    .pane(&target.pane_id)
                    .and_then(|pane| pane.last_run_command.clone());
                shell.borrow_mut().state.configure_pane_launch(
                    &target.pane_id,
                    path.clone(),
                    last_run_command,
                );
            }
            eprintln!(
                "zentty-linux: shell-signal pane={} kind=pane-context scope={} path-present={}",
                target.pane_id,
                scope,
                path.is_some()
            );
        }
    }
    Ok(String::new())
}

fn apply_agent_lifecycle_signal(
    shell: &Rc<RefCell<ApplicationShell>>,
    target: &AgentTarget,
    arguments: &[String],
) -> Result<String, (&'static str, String)> {
    let signal = parse_agent_lifecycle_signal(arguments)?;
    let now = super::unix_time_ms();
    match signal {
        AgentLifecycleSignal::Event {
            event,
            origin,
            confidence,
        } => {
            shell.borrow_mut().state.apply_agent_signal_event(
                target.clone(),
                event.as_ref(),
                origin,
                confidence,
                now,
            );
            eprintln!(
                "zentty-linux: agent-signal pane={} kind=lifecycle",
                target.pane_id
            );
        }
        AgentLifecycleSignal::AttachPid {
            pid,
            tool,
            session_id,
            parent_session_id,
        } => {
            shell.borrow_mut().state.apply_agent_pid_signal(
                &target.pane_id,
                session_id.as_deref(),
                parent_session_id.as_deref(),
                tool.as_deref(),
                Some(pid),
                now,
            );
            eprintln!(
                "zentty-linux: agent-signal pane={} kind=pid event=attach pid={pid}",
                target.pane_id
            );
        }
        AgentLifecycleSignal::ClearPid { session_id } => {
            shell.borrow_mut().state.apply_agent_pid_signal(
                &target.pane_id,
                session_id.as_deref(),
                None,
                None,
                None,
                now,
            );
            eprintln!(
                "zentty-linux: agent-signal pane={} kind=pid event=clear",
                target.pane_id
            );
        }
    }
    shell.borrow().render_sidebar();
    Ok(String::new())
}

fn remove_null_fields(value: &mut serde_json::Value) {
    if let Some(object) = value.as_object_mut() {
        object.retain(|_, value| !value.is_null());
    }
}

fn pane_ids(shell: &ApplicationShell) -> Vec<String> {
    shell
        .state
        .worklanes()
        .iter()
        .flat_map(|worklane| &worklane.columns)
        .flat_map(|column| &column.panes)
        .map(|pane| pane.id.clone())
        .collect()
}

fn created_pane_ids(shell: &ApplicationShell, before: &[String]) -> Vec<String> {
    pane_ids(shell)
        .into_iter()
        .filter(|pane_id| !before.contains(pane_id))
        .collect()
}

fn topology_response(
    shell: &ApplicationShell,
    action: &str,
    source_pane_id: &str,
    created_pane_ids: &[String],
    arguments: &[String],
) -> Result<String, (&'static str, String)> {
    let worklane = shell.state.active_worklane();
    let focused_pane_id = shell.state.focused_pane_id().unwrap_or(source_pane_id);
    let all_pane_ids = worklane
        .columns
        .iter()
        .flat_map(|column| &column.panes)
        .map(|pane| pane.id.clone())
        .collect::<Vec<_>>();
    let affected_pane_ids = if action == "layout" {
        all_pane_ids.clone()
    } else {
        std::iter::once(source_pane_id.to_owned())
            .chain(created_pane_ids.iter().cloned())
            .collect::<Vec<_>>()
    };
    let columns = worklane
        .columns
        .iter()
        .map(|column| {
            let total_height = column.pane_heights.iter().sum::<f64>();
            serde_json::json!({
                "id": column.id,
                "width": column.width,
                "panes": column.panes.iter().zip(&column.pane_heights).map(|(pane, height)| {
                    serde_json::json!({"id": pane.id, "heightFraction": height / total_height})
                }).collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    let receipt = serde_json::json!({
        "version": 1,
        "action": action,
        "windowID": shell.window_template.id,
        "worklaneID": worklane.id,
        "sourcePaneID": source_pane_id,
        "focusedPaneID": focused_pane_id,
        "createdPaneIDs": created_pane_ids,
        "affectedPaneIDs": affected_pane_ids,
        "topology": {
            "viewportWidth": shell.pane_viewport_width(),
            "viewportHeight": shell.pane_viewport_height(),
            "columns": columns,
        },
    });
    if arguments.iter().any(|argument| argument == "--json") {
        serde_json::to_string(&receipt)
            .map(|value| value + "\n")
            .map_err(|error| {
                (
                    "result_failed",
                    format!("could not encode topology result: {error}"),
                )
            })
    } else {
        Ok(format!(
            "{action}: window={} worklane={} source={} focused={} created={} affected={}\n",
            shell.window_template.id,
            worklane.id,
            source_pane_id,
            focused_pane_id,
            created_pane_ids.len(),
            affected_pane_ids.len(),
        ))
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
        "left" => {
            let width = f64::from(PaneLayoutPolicy::visible_split_width(
                shell.borrow().pane_viewport_width(),
            ));
            ApplicationShell::split_focused_pane(shell, "cli-split-left", move |state, pane_id| {
                state.insert_focused_pane_left(pane_id, width)
            })
        }
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

struct GridMutation {
    source_pane_id: String,
    created_pane_ids: Vec<String>,
}

fn apply_grid(
    shell: &Rc<RefCell<ApplicationShell>>,
    arguments: &[String],
) -> Result<GridMutation, (&'static str, String)> {
    let before_state = shell.borrow().state.clone();
    let before_pane_ids = pane_ids(&shell.borrow());
    match apply_grid_unchecked(shell, arguments) {
        Ok(mut result) => {
            result.created_pane_ids = created_pane_ids(&shell.borrow(), &before_pane_ids);
            if arguments
                .iter()
                .any(|argument| argument == "--destination-source-created")
            {
                result
                    .created_pane_ids
                    .insert(0, result.source_pane_id.clone());
            }
            Ok(result)
        }
        Err((code, message)) => {
            let rollback = rollback_grid(shell, before_state, &before_pane_ids);
            Err((
                code,
                rollback.map_or(message.clone(), |detail| {
                    format!("{message}; rollback warning: {detail}")
                }),
            ))
        }
    }
}

fn apply_grid_unchecked(
    shell: &Rc<RefCell<ApplicationShell>>,
    arguments: &[String],
) -> Result<GridMutation, (&'static str, String)> {
    let (rows, columns) = parse_grid_dimensions(arguments)?;
    let command_text = parse_grid_command(arguments)?;
    if rows == 0 || columns == 0 || rows.saturating_mul(columns) > 36 {
        return Err((
            "invalid_request",
            "grid must contain between 1 and 36 panes".to_owned(),
        ));
    }
    prepare_grid_destination(shell, arguments)?;
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
        inject_grid_failure(created.len())?;
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
            inject_grid_failure(created.len())?;
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
    Ok(GridMutation {
        source_pane_id: source,
        created_pane_ids: created,
    })
}

fn prepare_grid_destination(
    shell: &Rc<RefCell<ApplicationShell>>,
    arguments: &[String],
) -> Result<(), (&'static str, String)> {
    if arguments
        .iter()
        .any(|argument| argument == "--new-worklane")
    {
        return ApplicationShell::create_worklane(shell)
            .map_err(|message| ("grid_failed", message));
    }
    let should_isolate = shell
        .borrow()
        .state
        .active_worklane()
        .columns
        .iter()
        .map(|column| column.panes.len())
        .sum::<usize>()
        > 1;
    if !should_isolate {
        return Ok(());
    }
    let mut shell = shell.borrow_mut();
    let worklane_id = shell.take_worklane_id();
    let placement = shell.config.worklanes.new_worklane_placement;
    let width = f64::from(shell.pane_viewport_width());
    if !shell
        .state
        .isolate_focused_pane_in_new_worklane(worklane_id.clone(), placement, width)
    {
        return Err((
            "grid_failed",
            "could not isolate the selected grid source".to_owned(),
        ));
    }
    eprintln!(
        "zentty-linux: cli-grid-isolated worklane={worklane_id} source={} ",
        shell.state.focused_pane_id().unwrap_or("unavailable")
    );
    Ok(())
}

fn inject_grid_failure(created_count: usize) -> Result<(), (&'static str, String)> {
    let Some(path) = std::env::var_os("ZENTTY_TEST_GRID_FAILURE_AFTER_FILE") else {
        return Ok(());
    };
    let requested = std::fs::read_to_string(path)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok());
    if requested == Some(created_count) {
        return Err((
            "grid_failed",
            format!("injected grid construction failure after {created_count} panes"),
        ));
    }
    Ok(())
}

fn rollback_grid(
    shell: &Rc<RefCell<ApplicationShell>>,
    before_state: zentty_core::WorkspaceState,
    before_pane_ids: &[String],
) -> Option<String> {
    let current_pane_ids = pane_ids(&shell.borrow());
    let created = rollback_surface_order(&current_pane_ids, before_pane_ids);
    let mut failures = Vec::new();
    {
        let mut shell = shell.borrow_mut();
        for pane_id in &created {
            if let Err(error) = shell.remove_live_surface(pane_id) {
                failures.push(format!("{pane_id}: {error}"));
            }
        }
        shell.state = before_state;
        shell.sync_agent_targets();
        shell.render();
        shell.focus_selected_surface();
    }
    eprintln!(
        "zentty-linux: cli-grid-rollback removed={} failures={}",
        created.len(),
        failures.len()
    );
    (!failures.is_empty()).then(|| failures.join(", "))
}

fn rollback_surface_order(current_pane_ids: &[String], before_pane_ids: &[String]) -> Vec<String> {
    current_pane_ids
        .iter()
        .filter(|pane_id| !before_pane_ids.contains(pane_id))
        .rev()
        .cloned()
        .collect()
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
    if command
        .iter()
        .any(|argument| argument.contains(['\n', '\r']))
    {
        return Err((
            "invalid_request",
            "grid command tokens may not contain line breaks".to_owned(),
        ));
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
    {
        let mut shell = shell.borrow_mut();
        for pane_id in created {
            if shell.pane_runtime.surface(pane_id).is_none() {
                return Err((
                    "grid_failed",
                    format!("grid pane {pane_id:?} has no live terminal"),
                ));
            }
            // A native surface is registered before Ghostty reports terminal
            // initialization. Queue launch text on that existing lifecycle
            // boundary instead of racing send_text against PTY readiness.
            shell.pane_runtime.queue_prefill(pane_id, text.to_owned());
        }
    }
    if !arguments.iter().any(|argument| argument == "--new-only") {
        let shell_ref = shell.borrow();
        let surface = shell_ref.pane_runtime.surface(source).ok_or_else(|| {
            (
                "grid_failed",
                format!("grid pane {source:?} has no live terminal"),
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

#[cfg(test)]
mod tests {
    use super::rollback_surface_order;

    #[test]
    fn grid_rollback_destroys_only_new_surfaces_in_reverse_creation_order() {
        let current = ["source", "created-1", "neighbor", "created-2"].map(str::to_owned);
        let before = ["source", "neighbor"].map(str::to_owned);

        assert_eq!(
            rollback_surface_order(&current, &before),
            ["created-2", "created-1"]
        );
        assert!(rollback_surface_order(&before, &before).is_empty());
    }
}
