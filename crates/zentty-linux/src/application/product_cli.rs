use super::ApplicationCoordinator;
use zentty_agent_ipc::{AuthenticatedProductRequest, ProductIpcKind, ProductIpcReply};

impl ApplicationCoordinator {
    pub(super) fn handle_product_commands(&mut self, commands: Vec<AuthenticatedProductRequest>) {
        for command in commands {
            let reply = match command.request.kind() {
                ProductIpcKind::Discover => self.handle_discovery_command(&command),
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

    fn handle_discovery_command(&self, command: &AuthenticatedProductRequest) -> ProductIpcReply {
        let arguments = command.request.arguments();
        let window_filter = option_value(arguments, "--window-id");
        let mut worklane_filter = option_value(arguments, "--worklane-id");
        if command.request.subcommand() == "panes-current-worklane" && worklane_filter.is_none() {
            worklane_filter = Some(command.target.worklane_id.as_str());
        }
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
        let output = match command.request.subcommand() {
            "windows" => render_rows("windows", windows, json),
            "worklanes" => render_rows("worklanes", worklanes, json),
            "panes" | "panes-current-worklane" => render_rows("panes", panes, json),
            "overview" => {
                let overview = serde_json::json!({
                    "windows": windows,
                    "worklanes": worklanes,
                    "panes": panes,
                });
                Ok(if json {
                    pretty_json(&overview)
                } else {
                    format!(
                        "WINDOWS {}  WORKLANES {}  PANES {}\n",
                        overview["windows"].as_array().map_or(0, Vec::len),
                        overview["worklanes"].as_array().map_or(0, Vec::len),
                        overview["panes"].as_array().map_or(0, Vec::len),
                    )
                })
            }
            "select-pane" => render_selected_pane(
                &panes,
                arguments,
                &command.target.pane_id,
                &self.agent_runtime.borrow().socket_path_for_cli(),
            ),
            _ => Err(format!(
                "unsupported discovery command {:?}",
                command.request.subcommand()
            )),
        };
        output.map_or_else(
            |message| {
                ProductIpcReply::failure("invalid_request", message)
                    .expect("bounded discovery diagnostic")
            },
            |stdout| ProductIpcReply::success(stdout).expect("bounded discovery output"),
        )
    }
}

fn render_rows(kind: &str, rows: Vec<serde_json::Value>, json: bool) -> Result<String, String> {
    if json {
        return Ok(pretty_json(&serde_json::Value::Array(rows)));
    }
    if rows.is_empty() {
        return Ok(format!("No {kind}.\n"));
    }
    Ok(rows
        .iter()
        .map(|row| match kind {
            "windows" => format!(
                "{}{} worklanes={} panes={}",
                if row["isFocused"] == true { "* " } else { "  " },
                row["id"].as_str().unwrap_or("-"),
                row["worklaneCount"],
                row["paneCount"],
            ),
            "worklanes" => format!(
                "{}{} {} panes={}",
                if row["isFocused"] == true { "* " } else { "  " },
                row["windowID"].as_str().unwrap_or("-"),
                row["id"].as_str().unwrap_or("-"),
                row["paneCount"],
            ),
            _ => format!(
                "{}{} {} {} {}",
                if row["isFocused"] == true { "* " } else { "  " },
                row["windowID"].as_str().unwrap_or("-"),
                row["worklaneID"].as_str().unwrap_or("-"),
                row["id"].as_str().unwrap_or("-"),
                row["title"].as_str().unwrap_or("-"),
            ),
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n")
}

fn render_selected_pane(
    panes: &[serde_json::Value],
    arguments: &[String],
    caller_pane_id: &str,
    socket_path: &str,
) -> Result<String, String> {
    let selected = if let Some(pane_id) = option_value(arguments, "--pane-id") {
        panes.iter().find(|pane| pane["id"] == pane_id)
    } else if let Some(index) = option_value(arguments, "--pane-index") {
        let index = index.parse::<u64>().map_err(|_| "invalid pane index")?;
        panes.iter().find(|pane| pane["index"] == index)
    } else {
        panes.iter().find(|pane| pane["id"] == caller_pane_id)
    }
    .ok_or_else(|| "could not resolve a pane for the requested selectors".to_owned())?;
    if arguments.iter().any(|argument| argument == "--shell") {
        let mut output = format!(
            "export ZENTTY_INSTANCE_SOCKET='{}'\nexport ZENTTY_WINDOW_ID='{}'\nexport ZENTTY_WORKLANE_ID='{}'\nexport ZENTTY_PANE_ID='{}'\n",
            shell_escape(socket_path),
            shell_escape(selected["windowID"].as_str().unwrap_or("")),
            shell_escape(selected["worklaneID"].as_str().unwrap_or("")),
            shell_escape(selected["id"].as_str().unwrap_or("")),
        );
        if let Some(token) = selected["controlToken"].as_str() {
            output.push_str(&format!(
                "export ZENTTY_PANE_TOKEN='{}'\n",
                shell_escape(token)
            ));
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
