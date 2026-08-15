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
            "windows" => Ok(render_rows("windows", windows, json)),
            "worklanes" => Ok(render_rows("worklanes", worklanes, json)),
            "panes" | "panes-current-worklane" => Ok(render_rows("panes", panes, json)),
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
    rows.iter()
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
        + "\n"
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
