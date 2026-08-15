use super::ApplicationCoordinator;
use crate::application_shell::ApplicationShell;
use crate::persistence_coordinator::WindowSnapshot;
use gtk::glib;
use std::fmt::Write;
use zentty_agent_ipc::{
    AuthenticatedProductRequest, ProductIpcKind, ProductIpcReply, ProductIpcRequest,
};
use zentty_core::{AgentTarget, ColumnRecipe, PaneRecipe, WindowRecipe, WorklaneRecipe};

impl ApplicationCoordinator {
    pub(super) fn handle_product_commands(&mut self, commands: Vec<AuthenticatedProductRequest>) {
        for command in commands {
            if command.request.subcommand() == "grid"
                && command
                    .request
                    .arguments()
                    .iter()
                    .any(|argument| argument == "--new-window")
            {
                self.schedule_new_window_grid(command);
                continue;
            }
            let reply = match command.request.kind() {
                ProductIpcKind::Discover => self.handle_discovery_command(&command),
                ProductIpcKind::Pane if command.request.subcommand() == "notify" => {
                    self.handle_pane_notification(&command)
                }
                ProductIpcKind::Pane => self.shells.get(&command.target.window_id).map_or_else(
                    || {
                        ProductIpcReply::failure("stale_target", "target window is unavailable")
                            .expect("bounded static product failure")
                    },
                    |shell| {
                        crate::application_shell::ApplicationShell::execute_product_command(
                            shell,
                            &command.target,
                            &command.request,
                        )
                    },
                ),
            };
            if let Err(error) = command.respond(reply) {
                eprintln!("zentty-linux: product-command-response failed={error}");
            }
        }
    }

    fn schedule_new_window_grid(&self, command: AuthenticatedProductRequest) {
        let coordinator = self.self_handle.clone();
        glib::idle_add_local_once(move || {
            let reply = coordinator.upgrade().map_or_else(
                || {
                    ProductIpcReply::failure("application_unavailable", "application stopped")
                        .expect("bounded new-window diagnostic")
                },
                |coordinator| Self::create_new_window_grid(&coordinator, &command),
            );
            if let Err(error) = command.respond(reply) {
                eprintln!("zentty-linux: product-command-response failed={error}");
            }
        });
    }

    fn create_new_window_grid(
        coordinator: &std::rc::Rc<std::cell::RefCell<Self>>,
        command: &AuthenticatedProductRequest,
    ) -> ProductIpcReply {
        let (destination_window_id, source_pane) = {
            let mut coordinator = coordinator.borrow_mut();
            let Some(source) = coordinator.shells.get(&command.target.window_id) else {
                return product_failure("stale_target", "source window is unavailable");
            };
            let Some(source_pane) = source.borrow().product_grid_source_pane(&command.target)
            else {
                return product_failure("stale_target", "source pane is unavailable");
            };
            let destination_window_id = coordinator.window_set.generate_id();
            if let Err(error) = coordinator.window_set.insert(destination_window_id.clone()) {
                return product_failure(
                    "grid_failed",
                    format!("could not reserve destination window: {error:?}"),
                );
            }
            (destination_window_id, source_pane)
        };
        let worklane_id = format!("worklane-{destination_window_id}");
        let pane_id = format!("pane-{destination_window_id}");
        let snapshot =
            new_grid_window_snapshot(&destination_window_id, &worklane_id, &pane_id, source_pane);
        if let Err(error) = Self::build_shell(coordinator, snapshot) {
            coordinator
                .borrow_mut()
                .window_set
                .close(&destination_window_id);
            return product_failure("grid_failed", error);
        }
        let destination = coordinator
            .borrow()
            .shells
            .get(&destination_window_id)
            .cloned()
            .expect("newly built grid shell is registered");
        let arguments = command
            .request
            .arguments()
            .iter()
            .filter(|argument| argument.as_str() != "--new-window")
            .cloned()
            .collect::<Vec<_>>();
        let request = ProductIpcRequest::new(ProductIpcKind::Pane, "grid", arguments)
            .expect("validated grid request remains valid without destination flag");
        let target = AgentTarget::new(&destination_window_id, &worklane_id, &pane_id);
        let reply = ApplicationShell::execute_product_command(&destination, &target, &request);
        if reply.error().is_some() {
            let mut coordinator_ref = coordinator.borrow_mut();
            coordinator_ref.shells.remove(&destination_window_id);
            coordinator_ref.window_set.close(&destination_window_id);
            if let Err(error) = coordinator_ref.teardown_shell(&destination_window_id, &destination)
            {
                eprintln!(
                    "zentty-linux: cli-grid rollback-window={destination_window_id} error={error}"
                );
            }
            return reply;
        }
        if let Err(error) = Self::present_shell(coordinator, &destination_window_id, true) {
            if let Err(rollback_error) = coordinator
                .borrow_mut()
                .close_window(&destination_window_id)
            {
                eprintln!(
                    "zentty-linux: cli-grid rollback-window={destination_window_id} error={rollback_error}"
                );
            }
            return product_failure("grid_failed", error);
        }
        eprintln!(
            "zentty-linux: cli-grid-window source={} destination={} pane={}",
            command.target.pane_id, destination_window_id, pane_id
        );
        reply
    }

    fn handle_pane_notification(
        &mut self,
        command: &AuthenticatedProductRequest,
    ) -> ProductIpcReply {
        let result = parse_pane_notification(command.request.arguments()).map(|notification| {
            let desktop_body = match (
                notification.subtitle.as_deref(),
                notification.body.as_deref(),
            ) {
                (Some(subtitle), Some(body)) => format!("{subtitle}\n{body}"),
                (Some(subtitle), None) => subtitle.to_owned(),
                (None, Some(body)) => body.to_owned(),
                (None, None) => String::new(),
            };
            match crate::notification_service::NotificationService::send_pane(
                &notification.title,
                &desktop_body,
                &self.config.notifications,
                notification.silent,
            ) {
                Ok(id) => eprintln!(
                    "zentty-linux: cli-notification desktop=sent service-id={id} pane={} silent={}",
                    command.target.pane_id, notification.silent
                ),
                Err(error) => eprintln!(
                    "zentty-linux: cli-notification desktop=unavailable pane={} silent={} detail={error}",
                    command.target.pane_id, notification.silent
                ),
            }
            if notification.include_inbox {
                let primary = notification
                    .body
                    .as_deref()
                    .or(notification.subtitle.as_deref())
                    .unwrap_or("Notification from pane.");
                self.attention_inbox
                    .borrow_mut()
                    .record_pane_notification(
                        zentty_core::AttentionTarget::new(
                            &command.target.window_id,
                            &command.target.worklane_id,
                            &command.target.pane_id,
                        ),
                        &notification.title,
                        primary,
                        super::current_time_ms(),
                    );
                for shell in self.shells.values() {
                    shell.borrow().refresh_attention_inbox();
                }
            }
            eprintln!(
                "zentty-linux: cli-notification inbox={} pane={} title-bytes={} body-bytes={}",
                notification.include_inbox,
                command.target.pane_id,
                notification.title.len(),
                desktop_body.len()
            );
            String::new()
        });
        result.map_or_else(
            |message| {
                ProductIpcReply::failure("invalid_request", message)
                    .expect("bounded notification diagnostic")
            },
            |stdout| ProductIpcReply::success(stdout).expect("bounded notification output"),
        )
    }

    fn handle_discovery_command(&self, command: &AuthenticatedProductRequest) -> ProductIpcReply {
        let arguments = command.request.arguments();
        let window_filter = option_value(arguments, "--window-id");
        let worklane_filter = discovery_worklane_filter(
            command.request.subcommand(),
            arguments,
            &command.target.worklane_id,
        );
        let include_tokens = arguments
            .iter()
            .any(|argument| argument == "--include-control-token");
        let json = arguments.iter().any(|argument| argument == "--json");
        let mut windows = Vec::new();
        let mut worklanes = Vec::new();
        let mut panes = Vec::new();
        for (index, window_id) in self.window_set.ordered_ids().iter().enumerate() {
            if window_filter.is_some_and(|filter| filter != window_id) {
                continue;
            }
            let Some(shell) = self.shells.get(window_id) else {
                continue;
            };
            let rows = shell.borrow().product_discovery_rows(
                index + 1,
                self.window_set.active_id() == Some(window_id.as_str()),
                include_tokens,
            );
            let filtered_worklanes = rows
                .worklanes
                .into_iter()
                .filter(|row| {
                    worklane_filter.is_none_or(|filter| row["id"].as_str() == Some(filter))
                })
                .collect::<Vec<_>>();
            if worklane_filter.is_none() || !filtered_worklanes.is_empty() {
                windows.push(rows.window);
            }
            worklanes.extend(filtered_worklanes);
            panes.extend(rows.panes.into_iter().filter(|row| {
                worklane_filter.is_none_or(|filter| row["worklaneID"].as_str() == Some(filter))
            }));
        }
        let output = render_discovery(
            command.request.subcommand(),
            windows,
            worklanes,
            panes,
            arguments,
            &command.target.pane_id,
            &self.agent_runtime.borrow().socket_path_for_cli(),
            json,
        );
        output.map_or_else(
            |message| {
                ProductIpcReply::failure("invalid_request", message)
                    .expect("bounded discovery diagnostic")
            },
            |stdout| ProductIpcReply::success(stdout).expect("bounded discovery output"),
        )
    }
}

fn discovery_worklane_filter<'a>(
    subcommand: &str,
    arguments: &'a [String],
    caller_worklane_id: &'a str,
) -> Option<&'a str> {
    option_value(arguments, "--worklane-id").or_else(|| {
        (subcommand == "panes-current-worklane"
            || (subcommand == "select-pane" && option_value(arguments, "--pane-index").is_some()))
        .then_some(caller_worklane_id)
    })
}

#[allow(clippy::too_many_arguments)]
fn render_discovery(
    subcommand: &str,
    windows: Vec<serde_json::Value>,
    worklanes: Vec<serde_json::Value>,
    panes: Vec<serde_json::Value>,
    arguments: &[String],
    caller_pane_id: &str,
    socket_path: &str,
    json: bool,
) -> Result<String, String> {
    match subcommand {
        "windows" => Ok(render_rows("windows", windows, json)),
        "worklanes" => Ok(render_rows("worklanes", worklanes, json)),
        "panes" | "panes-current-worklane" => Ok(render_rows("panes", panes, json)),
        "overview" => Ok(render_overview(&windows, &worklanes, &panes, json)),
        "select-pane" => render_selected_pane(&panes, arguments, caller_pane_id, socket_path),
        _ => Err(format!("unsupported discovery command {subcommand:?}")),
    }
}

fn new_grid_window_snapshot(
    window_id: &str,
    worklane_id: &str,
    pane_id: &str,
    source_pane: PaneRecipe,
) -> WindowSnapshot {
    WindowSnapshot {
        window: WindowRecipe {
            id: window_id.to_owned(),
            frame: None,
            active_worklane_id: Some(worklane_id.to_owned()),
            worklanes: vec![WorklaneRecipe {
                id: worklane_id.to_owned(),
                title: None,
                next_pane_number: 2,
                focused_column_id: Some(format!("column-{pane_id}")),
                columns: vec![ColumnRecipe {
                    id: format!("column-{pane_id}"),
                    width: 1.0,
                    focused_pane_id: Some(pane_id.to_owned()),
                    last_focused_pane_id: Some(pane_id.to_owned()),
                    pane_heights: vec![1.0],
                    panes: vec![PaneRecipe {
                        id: pane_id.to_owned(),
                        custom_title: None,
                        title_seed: source_pane.title_seed,
                        working_directory: source_pane.working_directory,
                        last_activity_title: None,
                        last_run_command: None,
                    }],
                }],
                color: None,
                bookmark_origin_id: None,
            }],
        },
        restored_drafts: Vec::new(),
    }
}

fn product_failure(code: &str, message: impl Into<String>) -> ProductIpcReply {
    ProductIpcReply::failure(code, message).expect("bounded product diagnostic")
}

struct PaneNotification {
    title: String,
    subtitle: Option<String>,
    body: Option<String>,
    include_inbox: bool,
    silent: bool,
}

fn parse_pane_notification(arguments: &[String]) -> Result<PaneNotification, String> {
    let title = option_value(arguments, "--title")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "notification title is required".to_owned())?
        .to_owned();
    let subtitle = option_value(arguments, "--subtitle")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let body = option_value(arguments, "--body")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let mut index = 0;
    while index < arguments.len() {
        index += match arguments[index].as_str() {
            "--title" | "--subtitle" | "--body" if index + 1 < arguments.len() => 2,
            "--no-inbox" | "--silent" => 1,
            value => return Err(format!("unexpected notification argument {value:?}")),
        };
    }
    Ok(PaneNotification {
        title,
        subtitle,
        body,
        include_inbox: !arguments.iter().any(|argument| argument == "--no-inbox"),
        silent: arguments.iter().any(|argument| argument == "--silent"),
    })
}

fn render_rows(kind: &str, rows: Vec<serde_json::Value>, json: bool) -> String {
    if json {
        return pretty_json(&serde_json::Value::Array(rows));
    }
    if rows.is_empty() {
        return format!("No {kind}.\n");
    }
    let header = match kind {
        "windows" => format!(
            "{}  F  {}  {}  PANES",
            pad("ORDER", 5),
            pad("WINDOW", 36),
            pad("WORKLANES", 9)
        ),
        "worklanes" => format!(
            "{}  {}  F  {}  {}  {}  PANES",
            pad("WINDOW", 36),
            pad("ORDER", 5),
            pad("WORKLANE", 20),
            pad("TITLE", 16),
            pad("COLS", 4)
        ),
        _ => format!(
            "{}  {}  {}  {}  F  {}  {}  {}  STATUS",
            pad("WINDOW", 12),
            pad("WORKLANE", 20),
            pad("IDX", 3),
            pad("COL", 3),
            pad("TITLE", 16),
            pad("CWD", 30),
            pad("AGENT", 12)
        ),
    };
    let body = rows
        .iter()
        .map(|row| match kind {
            "windows" => format!(
                "{}  {}  {}  {}  {}",
                pad(&integer(row, "order"), 5),
                focus_marker(row),
                pad(string(row, "id"), 36),
                pad(&integer(row, "worklaneCount"), 9),
                integer(row, "paneCount"),
            ),
            "worklanes" => format!(
                "{}  {}  {}  {}  {}  {}  {}",
                pad(string(row, "windowID"), 36),
                pad(&integer(row, "order"), 5),
                focus_marker(row),
                pad(string(row, "id"), 20),
                pad(
                    &truncate_tail(optional_string(row, "title").unwrap_or("-"), 16),
                    16
                ),
                pad(&integer(row, "columnCount"), 4),
                integer(row, "paneCount"),
            ),
            _ => {
                let cwd = optional_string(row, "workingDirectory")
                    .map_or_else(|| "-".to_owned(), abbreviate_home);
                let agent = optional_string(row, "agentTool").unwrap_or("-");
                let status = optional_string(row, "agentStatus").unwrap_or("-");
                format!(
                    "{}  {}  {}  {}  {}  {}  {}  {}  {}",
                    pad(&truncate_tail(string(row, "windowID"), 12), 12),
                    pad(&truncate_tail(string(row, "worklaneID"), 20), 20),
                    pad(&integer(row, "index"), 3),
                    pad(&integer(row, "column"), 3),
                    focus_marker(row),
                    pad(&truncate_tail(string(row, "title"), 16), 16),
                    pad(&truncate_tail(&cwd, 30), 30),
                    pad(&truncate_tail(agent, 12), 12),
                    status,
                )
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("{header}\n{body}\n")
}

fn render_overview(
    windows: &[serde_json::Value],
    worklanes: &[serde_json::Value],
    panes: &[serde_json::Value],
    json: bool,
) -> String {
    let nested_windows = nested_overview(windows, worklanes, panes);
    if json {
        return pretty_json(&serde_json::json!({"windows": nested_windows}));
    }
    if nested_windows.is_empty() {
        return "No windows.\n".to_owned();
    }
    let mut output = format!(
        "WINDOWS {}  WORKLANES {}  PANES {}\n\n",
        nested_windows.len(),
        worklanes.len(),
        panes.len()
    );
    for (window_index, window) in nested_windows.iter().enumerate() {
        let nested_worklanes = window["worklanes"].as_array().expect("array built above");
        let pane_count = nested_worklanes
            .iter()
            .map(|worklane| worklane["panes"].as_array().map_or(0, Vec::len))
            .sum::<usize>();
        writeln!(
            output,
            "window {}  {}  {}  worklanes:{}  panes:{}",
            focus_marker(window),
            integer(window, "order"),
            string(window, "id"),
            nested_worklanes.len(),
            pane_count
        )
        .expect("writing to a string cannot fail");
        for worklane in nested_worklanes {
            render_overview_worklane(&mut output, worklane);
        }
        if window_index + 1 < nested_windows.len() {
            output.push('\n');
        }
    }
    output
}

fn nested_overview(
    windows: &[serde_json::Value],
    worklanes: &[serde_json::Value],
    panes: &[serde_json::Value],
) -> Vec<serde_json::Value> {
    windows
        .iter()
        .map(|window| {
            let window_id = string(window, "id");
            let nested_worklanes = worklanes
                .iter()
                .filter(|worklane| string(worklane, "windowID") == window_id)
                .map(|worklane| {
                    let worklane_id = string(worklane, "id");
                    let nested_panes = panes
                        .iter()
                        .filter(|pane| {
                            string(pane, "windowID") == window_id
                                && string(pane, "worklaneID") == worklane_id
                        })
                        .cloned()
                        .collect::<Vec<_>>();
                    let mut nested = serde_json::json!({
                        "id": worklane["id"],
                        "order": worklane["order"],
                        "title": worklane["title"],
                        "isFocused": worklane["isFocused"],
                        "columnCount": worklane["columnCount"],
                        "focusedPaneID": worklane["focusedPaneID"],
                        "panes": nested_panes,
                    });
                    if let Some(object) = nested.as_object_mut() {
                        object.retain(|_, value| !value.is_null());
                    }
                    nested
                })
                .collect::<Vec<_>>();
            serde_json::json!({
                "id": window["id"],
                "order": window["order"],
                "isFocused": window["isFocused"],
                "worklanes": nested_worklanes,
            })
        })
        .collect()
}

fn render_overview_worklane(output: &mut String, worklane: &serde_json::Value) {
    let nested_panes = worklane["panes"].as_array().expect("array built above");
    let title = optional_string(worklane, "title")
        .map(|value| format!("{}  ", pad(&truncate_tail(value.trim(), 28), 28)))
        .unwrap_or_default();
    writeln!(
        output,
        "  worklane {}  {}  {}{}  panes:{}",
        focus_marker(worklane),
        integer(worklane, "order"),
        title,
        string(worklane, "id"),
        nested_panes.len()
    )
    .expect("writing to a string cannot fail");
    for pane in nested_panes {
        let cwd = optional_string(pane, "workingDirectory")
            .map(abbreviate_home)
            .map_or_else(|| "-".to_owned(), |value| truncate_leading(&value, 42));
        let title = non_empty(string(pane, "title"));
        let agent = agent_summary(
            optional_string(pane, "agentTool"),
            optional_string(pane, "agentStatus"),
        )
        .map(|value| format!("  {value}"))
        .unwrap_or_default();
        writeln!(
            output,
            "    pane {}  {}  {}  {}  {}{}",
            focus_marker(pane),
            pad(&integer(pane, "index"), 2),
            string(pane, "id"),
            pad(&truncate_tail(title, 42), 42),
            cwd,
            agent
        )
        .expect("writing to a string cannot fail");
    }
}

fn string<'a>(value: &'a serde_json::Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or("-")
}

fn optional_string<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    value[key].as_str().filter(|value| !value.is_empty())
}

fn integer(value: &serde_json::Value, key: &str) -> String {
    value[key]
        .as_u64()
        .map_or_else(|| "-".to_owned(), |value| value.to_string())
}

fn focus_marker(value: &serde_json::Value) -> &'static str {
    if value["isFocused"] == true { "*" } else { " " }
}

fn pad(value: &str, width: usize) -> String {
    let length = value.chars().count();
    if length >= width {
        value.to_owned()
    } else {
        value.to_owned() + &" ".repeat(width - length)
    }
}

fn truncate_tail(value: &str, limit: usize) -> String {
    let length = value.chars().count();
    if length <= limit || limit <= 1 {
        value.to_owned()
    } else {
        value.chars().take(limit - 1).collect::<String>() + "…"
    }
}

fn truncate_leading(value: &str, limit: usize) -> String {
    let length = value.chars().count();
    if length <= limit || limit <= 1 {
        value.to_owned()
    } else {
        "…".to_owned() + &value.chars().skip(length - limit + 1).collect::<String>()
    }
}

fn abbreviate_home(value: &str) -> String {
    std::env::var_os("HOME").map_or_else(
        || value.to_owned(),
        |home| abbreviate_home_with(value, &home.to_string_lossy()),
    )
}

fn abbreviate_home_with(value: &str, home: &str) -> String {
    if home.is_empty() {
        return value.to_owned();
    }
    value
        .strip_prefix(home)
        .map_or_else(|| value.to_owned(), |suffix| format!("~{suffix}"))
}

fn non_empty(value: &str) -> &str {
    let trimmed = value.trim();
    if trimmed.is_empty() { "-" } else { trimmed }
}

fn agent_summary(tool: Option<&str>, status: Option<&str>) -> Option<String> {
    let parts = [tool, status]
        .into_iter()
        .flatten()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| truncate_tail(value, 14))
        .collect::<Vec<_>>();
    (!parts.is_empty()).then(|| format!("[{}]", parts.join(" ")))
}

fn render_selected_pane(
    panes: &[serde_json::Value],
    arguments: &[String],
    caller_pane_id: &str,
    socket_path: &str,
) -> Result<String, String> {
    let matches = if let Some(pane_id) = option_value(arguments, "--pane-id") {
        panes
            .iter()
            .filter(|pane| pane["id"] == pane_id)
            .collect::<Vec<_>>()
    } else if let Some(index) = option_value(arguments, "--pane-index") {
        let index = index.parse::<u64>().map_err(|_| "invalid pane index")?;
        panes
            .iter()
            .filter(|pane| pane["index"] == index)
            .collect::<Vec<_>>()
    } else {
        panes
            .iter()
            .filter(|pane| pane["id"] == caller_pane_id)
            .collect::<Vec<_>>()
    };
    let [selected] = matches.as_slice() else {
        return Err(if matches.is_empty() {
            "could not resolve a pane for the requested selectors".to_owned()
        } else {
            "pane selectors resolved more than one target".to_owned()
        });
    };
    if arguments.iter().any(|argument| argument == "--shell") {
        let mut output = format!(
            "export ZENTTY_INSTANCE_SOCKET='{}'\nexport ZENTTY_WINDOW_ID='{}'\nexport ZENTTY_WORKLANE_ID='{}'\nexport ZENTTY_PANE_ID='{}'\n",
            shell_escape(socket_path),
            shell_escape(selected["windowID"].as_str().unwrap_or("")),
            shell_escape(selected["worklaneID"].as_str().unwrap_or("")),
            shell_escape(selected["id"].as_str().unwrap_or("")),
        );
        if let Some(token) = selected["controlToken"].as_str() {
            writeln!(output, "export ZENTTY_PANE_TOKEN='{}'", shell_escape(token))
                .expect("writing to a string cannot fail");
        }
        Ok(output)
    } else {
        Ok(format!(
            "window {}\nworklane {}\npane {}\n",
            selected["windowID"].as_str().unwrap_or(""),
            selected["worklaneID"].as_str().unwrap_or(""),
            selected["id"].as_str().unwrap_or(""),
        ))
    }
}

fn option_value<'a>(arguments: &'a [String], option: &str) -> Option<&'a str> {
    arguments
        .windows(2)
        .find_map(|pair| (pair[0] == option).then_some(pair[1].as_str()))
}

fn pretty_json(value: &serde_json::Value) -> String {
    serde_json::to_string_pretty(value).expect("JSON values always serialize") + "\n"
}

fn shell_escape(value: &str) -> String {
    value.replace('\'', "'\"'\"'")
}

#[cfg(test)]
mod tests {
    use super::{
        abbreviate_home_with, agent_summary, discovery_worklane_filter, render_discovery,
        render_overview, render_rows, render_selected_pane, shell_escape, truncate_leading,
        truncate_tail,
    };
    use serde_json::json;

    fn pane(id: &str, index: u64, token: &str) -> serde_json::Value {
        json!({
            "id": id,
            "windowID": "window-'$(literal)\n雪",
            "worklaneID": "lane-'two",
            "index": index,
            "controlToken": token,
        })
    }

    #[test]
    fn shell_exports_quote_hostile_values_without_interpolation() {
        let rendered = render_selected_pane(
            &[pane("pane-'$HOME\n$(touch nope)", 1, "token-'quoted")],
            &[
                "--pane-index".to_owned(),
                "1".to_owned(),
                "--shell".to_owned(),
                "--include-control-token".to_owned(),
            ],
            "caller",
            "/tmp/socket-'$(literal)\nnext",
        )
        .unwrap();
        assert_eq!(
            rendered,
            "export ZENTTY_INSTANCE_SOCKET='/tmp/socket-'\"'\"'$(literal)\nnext'\n\
export ZENTTY_WINDOW_ID='window-'\"'\"'$(literal)\n雪'\n\
export ZENTTY_WORKLANE_ID='lane-'\"'\"'two'\n\
export ZENTTY_PANE_ID='pane-'\"'\"'$HOME\n$(touch nope)'\n\
export ZENTTY_PANE_TOKEN='token-'\"'\"'quoted'\n"
        );
        assert_eq!(shell_escape("a'b"), "a'\"'\"'b");
    }

    #[test]
    fn explicit_selector_resolution_rejects_missing_and_duplicate_matches() {
        let panes = [pane("duplicate", 1, "one"), pane("duplicate", 2, "two")];
        assert_eq!(
            render_selected_pane(
                &panes,
                &["--pane-id".to_owned(), "missing".to_owned()],
                "caller",
                "/tmp/socket",
            ),
            Err("could not resolve a pane for the requested selectors".to_owned())
        );
        assert_eq!(
            render_selected_pane(
                &panes,
                &["--pane-id".to_owned(), "duplicate".to_owned()],
                "caller",
                "/tmp/socket",
            ),
            Err("pane selectors resolved more than one target".to_owned())
        );
        assert_eq!(
            render_selected_pane(&[pane("caller", 1, "token")], &[], "caller", "/tmp/socket")
                .unwrap(),
            "window window-'$(literal)\n雪\nworklane lane-'two\npane caller\n"
        );
    }

    #[test]
    fn discovery_defaults_and_dispatch_are_explicit() {
        let explicit = ["--worklane-id".to_owned(), "explicit".to_owned()];
        assert_eq!(
            discovery_worklane_filter("panes-current-worklane", &explicit, "caller"),
            Some("explicit")
        );
        assert_eq!(
            discovery_worklane_filter("panes-current-worklane", &[], "caller"),
            Some("caller")
        );
        assert_eq!(
            discovery_worklane_filter(
                "select-pane",
                &["--pane-index".to_owned(), "1".to_owned()],
                "caller",
            ),
            Some("caller")
        );
        assert_eq!(
            discovery_worklane_filter("select-pane", &[], "caller"),
            None
        );
        assert_eq!(
            render_discovery("panes", vec![], vec![], vec![], &[], "caller", "/s", false),
            Ok("No panes.\n".to_owned())
        );
        assert_eq!(
            render_discovery(
                "windows",
                vec![],
                vec![],
                vec![],
                &[],
                "caller",
                "/s",
                false
            ),
            Ok("No windows.\n".to_owned())
        );
        assert_eq!(
            render_discovery(
                "worklanes",
                vec![],
                vec![],
                vec![],
                &[],
                "caller",
                "/s",
                false
            ),
            Ok("No worklanes.\n".to_owned())
        );
        assert_eq!(
            render_discovery(
                "overview",
                vec![],
                vec![],
                vec![],
                &[],
                "caller",
                "/s",
                false
            ),
            Ok("No windows.\n".to_owned())
        );
        assert_eq!(
            render_discovery(
                "select-pane",
                vec![],
                vec![],
                vec![pane("caller", 1, "token")],
                &[],
                "caller",
                "/s",
                false,
            )
            .unwrap(),
            "window window-'$(literal)\n雪\nworklane lane-'two\npane caller\n"
        );
        assert!(
            render_discovery(
                "unknown",
                vec![],
                vec![],
                vec![],
                &[],
                "caller",
                "/s",
                false,
            )
            .is_err()
        );
    }

    #[test]
    fn source_compatible_text_outputs_match_reviewed_goldens() {
        let window = json!({
            "id": "<WINDOW_ID>", "order": 1, "isFocused": true,
            "worklaneCount": 1, "paneCount": 1
        });
        let worklane = json!({
            "id": "<WORKLANE_ID>", "windowID": "<WINDOW_ID>", "order": 1,
            "title": "<TITLE>", "isFocused": true, "paneCount": 1,
            "columnCount": 1, "focusedPaneID": "<PANE_ID>"
        });
        let pane = json!({
            "id": "<PANE_ID>", "windowID": "<WINDOW_ID>",
            "worklaneID": "<WORKLANE_ID>", "index": 1, "column": 1,
            "title": "<TITLE>", "workingDirectory": "<CWD>", "isFocused": true
        });
        assert_eq!(
            render_rows("windows", vec![window.clone()], false),
            include_str!("../../../../docs/design/cli-goldens/windows-v1.txt")
        );
        assert_eq!(
            render_rows("worklanes", vec![worklane.clone()], false),
            include_str!("../../../../docs/design/cli-goldens/worklanes-v1.txt")
        );
        assert_eq!(
            render_rows("panes", vec![pane.clone()], false),
            include_str!("../../../../docs/design/cli-goldens/panes-v1.txt")
        );
        let mut overview_worklane = worklane;
        overview_worklane.as_object_mut().unwrap().remove("title");
        assert_eq!(
            render_overview(&[window], &[overview_worklane], &[pane], false),
            include_str!("../../../../docs/design/cli-goldens/topology-overview-v1.txt")
        );
    }

    #[test]
    fn overview_nesting_and_display_helpers_preserve_boundaries() {
        let windows = vec![
            json!({"id":"window-a","order":1,"isFocused":true}),
            json!({"id":"window-b","order":2,"isFocused":false}),
        ];
        let worklanes = vec![json!({
            "id":"lane-shared","windowID":"window-a","order":1,
            "isFocused":true,"columnCount":1
        })];
        let panes = vec![
            json!({
                "id":"pane-a","windowID":"window-a","worklaneID":"lane-shared",
                "index":1,"column":1,"title":"a","isFocused":true,
                "agentTool":"codex","agentStatus":"working"
            }),
            json!({
                "id":"pane-wrong-window","windowID":"window-b","worklaneID":"lane-shared",
                "index":2,"column":1,"title":"b","isFocused":false
            }),
        ];
        let json_output = render_overview(&windows, &worklanes, &panes, true);
        let parsed: serde_json::Value = serde_json::from_str(&json_output).unwrap();
        assert_eq!(
            parsed["windows"][0]["worklanes"][0]["panes"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            parsed["windows"][0]["worklanes"][0]["panes"][0]["id"],
            "pane-a"
        );
        assert!(
            parsed["windows"][1]["worklanes"]
                .as_array()
                .unwrap()
                .is_empty()
        );

        let text = render_overview(&windows, &worklanes, &panes, false);
        assert!(text.contains("[codex working]"));
        assert!(text.contains("\n\nwindow    2  window-b"));
        assert_eq!(truncate_tail("abcdef", 4), "abc…");
        assert_eq!(truncate_leading("abcdef", 4), "…def");
        assert_eq!(
            abbreviate_home_with("/home/test/project", "/home/test"),
            "~/project"
        );
        assert_eq!(
            abbreviate_home_with("/srv/project", "/home/test"),
            "/srv/project"
        );
        assert_eq!(abbreviate_home_with("/srv/project", ""), "/srv/project");
        assert_eq!(
            agent_summary(Some("codex"), Some("working")),
            Some("[codex working]".to_owned())
        );
        assert_eq!(agent_summary(Some(" "), None), None);
    }
}
