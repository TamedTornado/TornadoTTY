use super::ApplicationCoordinator;
use crate::application_shell::ApplicationShell;
use crate::persistence_coordinator::WindowSnapshot;
use gtk::glib;
use zentty_agent_ipc::AuthenticatedProductRequest;
use zentty_api::{
    ApplicationAuthority, ApplicationOperation, ApplicationReply as ProductIpcReply,
    ApplicationRequest as ProductIpcRequest, ApplicationResult, ApplicationResultKind,
    ApplicationScope as ProductIpcKind, ApplicationTarget,
};
use zentty_core::{ColumnRecipe, PaneRecipe, WindowRecipe, WorklaneRecipe};

impl ApplicationCoordinator {
    pub(super) fn handle_product_commands(&mut self, commands: Vec<AuthenticatedProductRequest>) {
        for command in commands {
            if command.request.operation() == ApplicationOperation::Grid
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
                ProductIpcKind::Pane
                    if command.request.operation() == ApplicationOperation::Notify =>
                {
                    self.handle_pane_notification(&command)
                }
                ProductIpcKind::Pane => self.shells.get(&command.target.window_id).map_or_else(
                    || {
                        ProductIpcReply::failure("stale_target", "target window is unavailable")
                            .expect("bounded static product failure")
                    },
                    |shell| {
                        crate::application_shell::ApplicationShell::execute_application_request(
                            shell,
                            &command.target,
                            command.authority,
                            &command.request,
                        )
                    },
                ),
            };
            if should_present_product_target(&command.request, &reply)
                && let Some(shell) = self.shells.get(&command.target.window_id)
            {
                // Pane focus is also the source CLI's worklane/window
                // selection operation. Present the same GTK window owned by
                // the coordinator; do not create an IPC-specific focus model.
                shell.borrow().present();
                let changed = self.window_set.mark_active(&command.target.window_id);
                eprintln!(
                    "zentty-linux: cli-window-select window={} worklane={} pane={} changed={changed}",
                    command.target.window_id, command.target.worklane_id, command.target.pane_id
                );
            }
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
        let mut arguments = command
            .request
            .arguments()
            .iter()
            .filter(|argument| argument.as_str() != "--new-window")
            .cloned()
            .collect::<Vec<_>>();
        arguments.push("--destination-source-created".to_owned());
        let request = ProductIpcRequest::new(ProductIpcKind::Pane, "grid", arguments)
            .expect("validated grid request remains valid without destination flag");
        let target = ApplicationTarget::new(&destination_window_id, &worklane_id, &pane_id);
        let reply = ApplicationShell::execute_application_request(
            &destination,
            &target,
            ApplicationAuthority::Pane,
            &request,
        );
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
            ApplicationResult::empty()
        });
        result.map_or_else(
            |message| {
                ProductIpcReply::failure("invalid_request", message)
                    .expect("bounded notification diagnostic")
            },
            |result| ProductIpcReply::success(result).expect("bounded notification result"),
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
        let output = application_discovery_result(
            command.request.subcommand(),
            windows,
            worklanes,
            panes,
            arguments,
            &command.target.pane_id,
            &self.agent_runtime.borrow().socket_path_for_cli(),
        );
        output.map_or_else(
            |message| {
                ProductIpcReply::failure("invalid_request", message)
                    .expect("bounded discovery diagnostic")
            },
            |result| ProductIpcReply::success(result).expect("bounded discovery result"),
        )
    }
}

fn should_present_product_target(request: &ProductIpcRequest, reply: &ProductIpcReply) -> bool {
    request.kind() == ProductIpcKind::Pane
        && request.operation() == ApplicationOperation::Focus
        && reply.error().is_none()
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
fn application_discovery_result(
    subcommand: &str,
    windows: Vec<serde_json::Value>,
    worklanes: Vec<serde_json::Value>,
    panes: Vec<serde_json::Value>,
    arguments: &[String],
    caller_pane_id: &str,
    socket_path: &str,
) -> Result<ApplicationResult, String> {
    let (kind, value) = match subcommand {
        "windows" => (ApplicationResultKind::Discovery, windows.into()),
        "worklanes" => (ApplicationResultKind::Discovery, worklanes.into()),
        "panes" | "panes-current-worklane" => (ApplicationResultKind::Discovery, panes.into()),
        "overview" => (
            ApplicationResultKind::Discovery,
            serde_json::json!({"windows": nested_overview(&windows, &worklanes, &panes)}),
        ),
        "select-pane" => (
            ApplicationResultKind::Selection,
            selected_pane_value(&panes, arguments, caller_pane_id, socket_path)?,
        ),
        _ => return Err(format!("unsupported discovery command {subcommand:?}")),
    };
    Ok(ApplicationResult::new(kind, value))
}

fn selected_pane_value(
    panes: &[serde_json::Value],
    arguments: &[String],
    caller_pane_id: &str,
    socket_path: &str,
) -> Result<serde_json::Value, String> {
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
    Ok(serde_json::json!({
        "socketPath": socket_path,
        "pane": selected,
    }))
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

fn string<'a>(value: &'a serde_json::Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or("-")
}

fn option_value<'a>(arguments: &'a [String], option: &str) -> Option<&'a str> {
    arguments
        .windows(2)
        .find_map(|pair| (pair[0] == option).then_some(pair[1].as_str()))
}

#[cfg(test)]
mod tests {
    use super::{
        application_discovery_result, discovery_worklane_filter, nested_overview,
        selected_pane_value, should_present_product_target,
    };
    use serde_json::json;
    use zentty_api::{
        ApplicationReply as ProductIpcReply, ApplicationRequest as ProductIpcRequest,
        ApplicationResult, ApplicationResultKind, ApplicationScope as ProductIpcKind,
    };

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
    fn only_successful_pane_focus_selects_the_containing_window() {
        let focus = ProductIpcRequest::new(ProductIpcKind::Pane, "focus", Vec::new()).unwrap();
        let split =
            ProductIpcRequest::new(ProductIpcKind::Pane, "split", vec!["right".into()]).unwrap();
        let discovery =
            ProductIpcRequest::new(ProductIpcKind::Discover, "panes", Vec::new()).unwrap();
        let success = ProductIpcReply::success(ApplicationResult::empty()).unwrap();
        let failure = ProductIpcReply::failure("stale_target", "target disappeared").unwrap();
        assert!(should_present_product_target(&focus, &success));
        assert!(!should_present_product_target(&focus, &failure));
        assert!(!should_present_product_target(&split, &success));
        assert!(!should_present_product_target(&discovery, &success));
    }

    #[test]
    fn selector_resolution_is_structured_and_rejects_ambiguous_targets() {
        let panes = [pane("duplicate", 1, "one"), pane("duplicate", 2, "two")];
        assert_eq!(
            selected_pane_value(
                &panes,
                &["--pane-id".to_owned(), "missing".to_owned()],
                "caller",
                "/tmp/socket",
            ),
            Err("could not resolve a pane for the requested selectors".to_owned())
        );
        assert_eq!(
            selected_pane_value(
                &panes,
                &["--pane-id".to_owned(), "duplicate".to_owned()],
                "caller",
                "/tmp/socket",
            ),
            Err("pane selectors resolved more than one target".to_owned())
        );
        let selected =
            selected_pane_value(&[pane("caller", 1, "token")], &[], "caller", "/tmp/socket")
                .unwrap();
        assert_eq!(selected["socketPath"], "/tmp/socket");
        assert_eq!(selected["pane"]["id"], "caller");
    }

    #[test]
    fn discovery_defaults_and_structured_dispatch_are_explicit() {
        let explicit = ["--worklane-id".to_owned(), "explicit".to_owned()];
        assert_eq!(
            discovery_worklane_filter("panes-current-worklane", &explicit, "caller"),
            Some("explicit")
        );
        assert_eq!(
            discovery_worklane_filter("panes-current-worklane", &[], "caller"),
            Some("caller")
        );
        let panes = application_discovery_result(
            "panes",
            vec![],
            vec![],
            vec![pane("caller", 1, "token")],
            &[],
            "caller",
            "/socket",
        )
        .unwrap();
        assert_eq!(panes.kind(), ApplicationResultKind::Discovery);
        assert_eq!(panes.value()[0]["id"], "caller");
        assert!(
            application_discovery_result(
                "unknown",
                vec![],
                vec![],
                vec![],
                &[],
                "caller",
                "/socket",
            )
            .is_err()
        );
    }

    #[test]
    fn overview_nesting_never_crosses_equal_worklane_ids_between_windows() {
        let windows = vec![json!({"id":"window-a"}), json!({"id":"window-b"})];
        let worklanes = vec![json!({
            "id":"lane-shared", "windowID":"window-a", "order":1,
            "isFocused":true, "columnCount":1
        })];
        let panes = vec![
            json!({"id":"pane-a","windowID":"window-a","worklaneID":"lane-shared"}),
            json!({"id":"wrong","windowID":"window-b","worklaneID":"lane-shared"}),
        ];
        let nested = nested_overview(&windows, &worklanes, &panes);
        assert_eq!(
            nested[0]["worklanes"][0]["panes"].as_array().unwrap().len(),
            1
        );
        assert_eq!(nested[0]["worklanes"][0]["panes"][0]["id"], "pane-a");
        assert!(nested[1]["worklanes"].as_array().unwrap().is_empty());
    }
}
