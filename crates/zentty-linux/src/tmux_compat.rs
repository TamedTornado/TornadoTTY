use std::collections::BTreeMap;
use zentty_core::{AgentTarget, PaneState, WorklaneState, WorkspaceState};
use zentty_tmux_compat::{
    Command, FormatRenderer, PaneTarget, ParsedArguments, SendKeys, TeamStore, TmuxCompatReply,
    TmuxCompatRequest,
};

const DEFAULT_LIST_PANES: &str = "#{pane_id} #{pane_index} #{pane_title} #{?pane_active,*,-}";
const DEFAULT_LIST_WINDOWS: &str = "#{window_id} #{window_index} #{window_name}";
const DEFAULT_DISPLAY_MESSAGE: &str = "#{pane_id}";
const DEFAULT_SPLIT_PRINT: &str = "#{pane_id}";

#[derive(Default)]
pub(crate) struct TmuxCompatProduct {
    store: TeamStore,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum TmuxProductAction {
    Noop,
    SendText { pane_id: String, text: String },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SplitDisposition {
    RightGolden,
    StackBelow,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct SplitPlan {
    pub worklane_id: String,
    pub leader_pane_id: String,
    pub insertion_pane_id: String,
    pub disposition: SplitDisposition,
    pub detached: bool,
    pub print_format: Option<String>,
}

impl TmuxCompatProduct {
    pub(crate) fn prepare_split(
        &self,
        state: &WorkspaceState,
        target: &AgentTarget,
        request: &TmuxCompatRequest,
    ) -> Result<SplitPlan, (&'static str, String)> {
        let worklane = target_worklane(state, target)?;
        let pane_ids = pane_entries(worklane)
            .into_iter()
            .map(|(_, pane, _)| pane.id.clone())
            .collect::<Vec<_>>();
        let parsed = parsed(
            request.arguments(),
            &["-F", "-c", "-l", "-t"],
            &["-P", "-b", "-d", "-h", "-v"],
        );
        validate_explicit_pane_target(parsed.value("-t"), &pane_ids)?;
        validate_explicit_pane_target(Some(&target.pane_id), &pane_ids)?;
        let (leader_pane_id, insertion_pane_id, disposition) =
            self.store.anchor(&target.worklane_id).map_or_else(
                || {
                    (
                        target.pane_id.clone(),
                        target.pane_id.clone(),
                        SplitDisposition::RightGolden,
                    )
                },
                |anchor| {
                    (
                        anchor.leader_pane_id.clone(),
                        anchor
                            .column_pane_ids
                            .last()
                            .cloned()
                            .unwrap_or_else(|| anchor.leader_pane_id.clone()),
                        SplitDisposition::StackBelow,
                    )
                },
            );
        Ok(SplitPlan {
            worklane_id: target.worklane_id.clone(),
            leader_pane_id,
            insertion_pane_id,
            disposition,
            detached: parsed.has_flag("-d"),
            print_format: parsed
                .has_flag("-P")
                .then(|| parsed.value("-F").unwrap_or(DEFAULT_SPLIT_PRINT).to_owned()),
        })
    }

    pub(crate) fn record_split(
        &mut self,
        plan: &SplitPlan,
        new_pane_id: &str,
        pre_team_leader_width: Option<u32>,
    ) {
        let _ = self.store.record_split(
            &plan.worklane_id,
            &plan.leader_pane_id,
            new_pane_id,
            plan.detached,
            pre_team_leader_width,
        );
    }

    pub(crate) fn split_reply(
        &self,
        state: &WorkspaceState,
        plan: &SplitPlan,
        new_pane_id: &str,
    ) -> TmuxCompatReply {
        let Some(template) = plan.print_format.as_deref() else {
            return TmuxCompatReply::success(String::new())
                .expect("empty compatibility output fits protocol limits");
        };
        let Some(worklane) = state
            .worklanes()
            .iter()
            .find(|worklane| worklane.id == plan.worklane_id)
        else {
            return failure("target_not_found", "split worklane disappeared");
        };
        let Some((index, pane, focused)) = pane_entries(worklane)
            .into_iter()
            .find(|(_, pane, _)| pane.id == new_pane_id)
        else {
            return failure("target_not_found", "new split pane disappeared");
        };
        let context = pane_context(
            pane,
            index,
            focused,
            self.store.active_pane(&plan.worklane_id),
            worklane,
            worklane_index(state, &plan.worklane_id),
        );
        TmuxCompatReply::success(format!("{}\n", FormatRenderer::render(template, &context)))
            .unwrap_or_else(|_| failure("output_limit", "tmux compatibility output limit exceeded"))
    }

    pub(crate) fn prepare_send_keys(
        state: &WorkspaceState,
        target: &AgentTarget,
        request: &TmuxCompatRequest,
    ) -> Result<TmuxProductAction, (&'static str, String)> {
        let worklane = target_worklane(state, target)?;
        let pane_ids = pane_entries(worklane)
            .into_iter()
            .map(|(_, pane, _)| pane.id.clone())
            .collect::<Vec<_>>();
        let parsed = parsed(request.arguments(), &["-N", "-T", "-t"], &["-R", "-l"]);
        validate_explicit_pane_target(parsed.value("-t"), &pane_ids)?;
        let pane_id = PaneTarget::resolve(parsed.value("-t"), &pane_ids, &target.pane_id);
        let text = SendKeys::translate(request.arguments(), request.standard_input());
        if text.is_empty() {
            Ok(TmuxProductAction::Noop)
        } else {
            Ok(TmuxProductAction::SendText { pane_id, text })
        }
    }

    pub(crate) fn handle(
        &mut self,
        state: &mut WorkspaceState,
        target: &AgentTarget,
        request: &TmuxCompatRequest,
    ) -> TmuxCompatReply {
        let result = match request.command() {
            Command::ListPanes => self.list_panes(state, target, request.arguments()),
            Command::ListWindows => Ok(Self::list_windows(state, request.arguments())),
            Command::DisplayMessage => self.display_message(state, target, request.arguments()),
            Command::SelectPane => self.select_pane(state, target, request.arguments()),
            Command::ShowOptions => Ok(Self::show_options(request.arguments())),
            Command::NewSession => Ok(format!("@{}\n", target.worklane_id)),
            Command::SelectWindow
            | Command::RenameWindow
            | Command::NewWindow
            | Command::LastPane => Ok(String::new()),
            Command::Popup => Err(("unsupported", "popup is unsupported".to_owned())),
            command => Err((
                "not_implemented",
                format!("{command:?} is not implemented in the Linux product handler"),
            )),
        };
        match result {
            Ok(stdout) => TmuxCompatReply::success(stdout).unwrap_or_else(|_| {
                failure("output_limit", "tmux compatibility output limit exceeded")
            }),
            Err((code, message)) => failure(code, message),
        }
    }

    fn list_panes(
        &self,
        state: &WorkspaceState,
        target: &AgentTarget,
        arguments: &[String],
    ) -> Result<String, (&'static str, String)> {
        let worklane = target_worklane(state, target)?;
        let parsed = parsed(arguments, &["-F", "-t"], &[]);
        let template = parsed.value("-F").unwrap_or(DEFAULT_LIST_PANES);
        let active = self.store.active_pane(&target.worklane_id);
        let window_index = worklane_index(state, &target.worklane_id);
        let lines = pane_entries(worklane)
            .into_iter()
            .map(|(index, pane, focused)| {
                FormatRenderer::render(
                    template,
                    &pane_context(pane, index, focused, active, worklane, window_index),
                )
            })
            .collect::<Vec<_>>();
        Ok(lines_with_trailing_newline(&lines))
    }

    fn display_message(
        &self,
        state: &WorkspaceState,
        target: &AgentTarget,
        arguments: &[String],
    ) -> Result<String, (&'static str, String)> {
        let worklane = target_worklane(state, target)?;
        let parsed = parsed(arguments, &["-F", "-t"], &["-p"]);
        let template = parsed
            .positionals()
            .first()
            .map_or(DEFAULT_DISPLAY_MESSAGE, String::as_str);
        let entries = pane_entries(worklane);
        let pane_ids = entries
            .iter()
            .map(|(_, pane, _)| pane.id.clone())
            .collect::<Vec<_>>();
        let explicit = parsed
            .value("-t")
            .map(|selector| PaneTarget::resolve(Some(selector), &pane_ids, &target.pane_id));
        let selected = explicit
            .as_deref()
            .or_else(|| self.store.active_pane(&target.worklane_id))
            .or(Some(target.pane_id.as_str()));
        let (index, pane, focused) = selected
            .and_then(|selected| {
                entries
                    .iter()
                    .find(|(_, pane, _)| pane.id == selected)
                    .copied()
            })
            .or_else(|| entries.iter().find(|(_, _, focused)| *focused).copied())
            .or_else(|| entries.first().copied())
            .ok_or(("target_not_found", "worklane contains no panes".to_owned()))?;
        let context = pane_context(
            pane,
            index,
            focused,
            self.store.active_pane(&target.worklane_id),
            worklane,
            worklane_index(state, &target.worklane_id),
        );
        Ok(format!("{}\n", FormatRenderer::render(template, &context)))
    }

    fn select_pane(
        &mut self,
        state: &mut WorkspaceState,
        target: &AgentTarget,
        arguments: &[String],
    ) -> Result<String, (&'static str, String)> {
        let worklane = target_worklane(state, target)?;
        let pane_ids = pane_entries(worklane)
            .into_iter()
            .map(|(_, pane, _)| pane.id.clone())
            .collect::<Vec<_>>();
        let parsed = parsed(arguments, &["-t", "-T"], &["-P"]);
        let pane_id = PaneTarget::resolve(parsed.value("-t"), &pane_ids, &target.pane_id);
        self.store.record_active_pane(&target.worklane_id, &pane_id);
        if let Some(title) = parsed
            .value("-T")
            .map(str::trim)
            .filter(|title| !title.is_empty())
        {
            state.set_pane_title(&pane_id, title);
        }
        Ok(String::new())
    }

    fn list_windows(state: &WorkspaceState, arguments: &[String]) -> String {
        let parsed = parsed(arguments, &["-F", "-t"], &[]);
        let template = parsed.value("-F").unwrap_or(DEFAULT_LIST_WINDOWS);
        let lines = state
            .worklanes()
            .iter()
            .enumerate()
            .map(|(index, worklane)| {
                FormatRenderer::render(
                    template,
                    &BTreeMap::from([
                        ("session_name".to_owned(), "zentty".to_owned()),
                        ("window_id".to_owned(), format!("@{}", worklane.id)),
                        ("window_uuid".to_owned(), worklane.id.clone()),
                        ("window_index".to_owned(), index.to_string()),
                        (
                            "window_name".to_owned(),
                            worklane
                                .title
                                .clone()
                                .unwrap_or_else(|| format!("worklane-{index}")),
                        ),
                    ]),
                )
            })
            .collect::<Vec<_>>();
        lines_with_trailing_newline(&lines)
    }

    fn show_options(arguments: &[String]) -> String {
        let parsed = parsed(arguments, &["-t"], &["-A", "-g", "-v", "-w"]);
        let name = parsed.positionals().last().map_or("", String::as_str);
        if name.is_empty() {
            return String::new();
        }
        let value = match name {
            "focus-events" | "mouse" | "synchronize-panes" => "off",
            _ => "",
        };
        if parsed.has_flag("-v") {
            format!("{value}\n")
        } else {
            format!("{name} {value}\n")
        }
    }
}

fn parsed(arguments: &[String], values: &[&str], flags: &[&str]) -> ParsedArguments {
    ParsedArguments::parse(arguments, &strings(values), &strings(flags))
}

fn validate_explicit_pane_target(
    selector: Option<&str>,
    pane_ids: &[String],
) -> Result<(), (&'static str, String)> {
    if let Some(selector) = selector {
        let candidate = selector.strip_prefix('%').unwrap_or(selector);
        if !pane_ids.iter().any(|pane_id| pane_id == candidate) {
            return Err((
                "target_not_found",
                format!("pane {selector} is unavailable in the routed worklane"),
            ));
        }
    }
    Ok(())
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn target_worklane<'a>(
    state: &'a WorkspaceState,
    target: &AgentTarget,
) -> Result<&'a WorklaneState, (&'static str, String)> {
    state
        .worklanes()
        .iter()
        .find(|worklane| worklane.id == target.worklane_id)
        .ok_or((
            "target_not_found",
            format!("worklane {} is unavailable", target.worklane_id),
        ))
}

fn worklane_index(state: &WorkspaceState, worklane_id: &str) -> usize {
    state
        .worklanes()
        .iter()
        .position(|worklane| worklane.id == worklane_id)
        .unwrap_or(0)
}

fn pane_entries(worklane: &WorklaneState) -> Vec<(usize, &PaneState, bool)> {
    worklane
        .columns
        .iter()
        .flat_map(|column| {
            column.panes.iter().map(move |pane| {
                let focused =
                    column.id == worklane.focused_column_id && pane.id == column.focused_pane_id;
                (pane, focused)
            })
        })
        .enumerate()
        .map(|(index, (pane, focused))| (index, pane, focused))
        .collect()
}

fn pane_context(
    pane: &PaneState,
    pane_index: usize,
    focused: bool,
    active_pane_id: Option<&str>,
    worklane: &WorklaneState,
    window_index: usize,
) -> BTreeMap<String, String> {
    let active = active_pane_id.map_or(focused, |active| active == pane.id);
    BTreeMap::from([
        ("session_name".to_owned(), "zentty".to_owned()),
        ("pane_id".to_owned(), format!("%{}", pane.id)),
        ("pane_uuid".to_owned(), pane.id.clone()),
        ("pane_index".to_owned(), pane_index.to_string()),
        ("pane_title".to_owned(), pane.display_title().to_owned()),
        (
            "pane_active".to_owned(),
            if active { "1" } else { "" }.to_owned(),
        ),
        (
            "pane_current_path".to_owned(),
            pane.working_directory.clone().unwrap_or_default(),
        ),
        ("window_id".to_owned(), format!("@{}", worklane.id)),
        ("window_index".to_owned(), window_index.to_string()),
        (
            "window_name".to_owned(),
            worklane
                .title
                .clone()
                .unwrap_or_else(|| format!("worklane-{window_index}")),
        ),
    ])
}

fn lines_with_trailing_newline(lines: &[String]) -> String {
    if lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", lines.join("\n"))
    }
}

fn failure(code: &'static str, message: impl Into<String>) -> TmuxCompatReply {
    TmuxCompatReply::failure(code, message).expect("static product diagnostic fits protocol limits")
}

#[cfg(test)]
mod tests {
    use super::{SplitDisposition, TmuxCompatProduct, TmuxProductAction};
    use zentty_core::AgentTarget;
    use zentty_core::WorkspaceState;
    use zentty_tmux_compat::TmuxCompatRequest;

    fn request(command: &str, arguments: &[&str]) -> TmuxCompatRequest {
        TmuxCompatRequest::new(
            1,
            command,
            arguments.iter().map(|value| (*value).to_owned()).collect(),
            None,
        )
        .unwrap()
    }

    fn target(pane_id: &str) -> AgentTarget {
        AgentTarget::new("window-1", "lane-1", pane_id)
    }

    fn workspace() -> WorkspaceState {
        let mut state = WorkspaceState::new("lane-1", "pane-1");
        state.set_worklane_title("lane-1", Some("Frontend"));
        state.set_pane_title("pane-1", "Editor");
        assert!(state.split_focused_pane_below("pane-2"));
        state.set_pane_title("pane-2", "Logs");
        assert!(state.select_worklane_and_pane("lane-1", "pane-1"));
        assert!(state.create_worklane("lane-2", "pane-3"));
        state.set_worklane_title("lane-2", Some("Backend"));
        assert!(state.select_worklane_and_pane("lane-1", "pane-1"));
        state
    }

    #[test]
    fn discovery_and_display_are_scoped_to_the_canonical_worklane() {
        let mut product = TmuxCompatProduct::default();
        let mut state = workspace();
        let panes = product.handle(&mut state, &target("pane-1"), &request("list-panes", &[]));
        assert_eq!(
            panes.stdout(),
            Some("%pane-1 0 Editor *\n%pane-2 1 Logs -\n")
        );
        let windows = product.handle(&mut state, &target("pane-1"), &request("list-windows", &[]));
        assert_eq!(
            windows.stdout(),
            Some("@lane-1 0 Frontend\n@lane-2 1 Backend\n")
        );
        let display = product.handle(
            &mut state,
            &target("pane-1"),
            &request("display-message", &["-p", "#{pane_id}:#{pane_title}"]),
        );
        assert_eq!(display.stdout(), Some("%pane-1:Editor\n"));
    }

    #[test]
    fn select_pane_updates_compatibility_active_state_and_source_title() {
        let mut product = TmuxCompatProduct::default();
        let mut state = workspace();
        let selected = product.handle(
            &mut state,
            &target("pane-1"),
            &request("select-pane", &["-t", "%pane-2", "-T", "Worker"]),
        );
        assert!(selected.is_ok());
        let display = product.handle(
            &mut state,
            &target("pane-1"),
            &request("display-message", &["-p", "#{pane_id}:#{pane_title}"]),
        );
        assert_eq!(display.stdout(), Some("%pane-2:Worker\n"));
        let panes = product.handle(
            &mut state,
            &target("pane-1"),
            &request("list-panes", &["-F", "#{pane_id}:#{pane_active}"]),
        );
        assert_eq!(panes.stdout(), Some("%pane-1:\n%pane-2:1\n"));
    }

    #[test]
    fn intentional_noops_and_unsupported_or_pending_commands_are_explicit() {
        let mut product = TmuxCompatProduct::default();
        let mut state = workspace();
        assert!(
            product
                .handle(
                    &mut state,
                    &target("pane-1"),
                    &request("rename-window", &[])
                )
                .is_ok()
        );
        let popup = product.handle(&mut state, &target("pane-1"), &request("popup", &[]));
        assert_eq!(popup.error().unwrap().code(), "unsupported");
        let pending = product.handle(&mut state, &target("pane-1"), &request("send-keys", &[]));
        assert_eq!(pending.error().unwrap().code(), "not_implemented");
    }

    #[test]
    fn source_formats_options_and_session_output_are_exact() {
        let mut product = TmuxCompatProduct::default();
        let mut state = workspace();
        let panes = product.handle(
            &mut state,
            &target("pane-1"),
            &request(
                "list-panes",
                &[
                    "-F",
                    "#{window_index}/#{window_name}/#{pane_index}/#{pane_active}",
                ],
            ),
        );
        assert_eq!(panes.stdout(), Some("0/Frontend/0/1\n0/Frontend/1/\n"));
        let windows = product.handle(
            &mut state,
            &target("pane-1"),
            &request("list-windows", &["-F", "#{window_index}:#{window_name}"]),
        );
        assert_eq!(windows.stdout(), Some("0:Frontend\n1:Backend\n"));
        let lane_two = AgentTarget::new("window-1", "lane-2", "pane-3");
        let second_lane_panes = product.handle(
            &mut state,
            &lane_two,
            &request("list-panes", &["-F", "#{window_index}:#{pane_id}"]),
        );
        assert_eq!(second_lane_panes.stdout(), Some("1:%pane-3\n"));
        let option = product.handle(
            &mut state,
            &target("pane-1"),
            &request("show-options", &["-v", "focus-events"]),
        );
        assert_eq!(option.stdout(), Some("off\n"));
        let named_unknown = product.handle(
            &mut state,
            &target("pane-1"),
            &request("show-options", &["unknown-option"]),
        );
        assert_eq!(named_unknown.stdout(), Some("unknown-option \n"));
        let unnamed = product.handle(&mut state, &target("pane-1"), &request("show-options", &[]));
        assert_eq!(unnamed.stdout(), Some(""));
        let session = product.handle(
            &mut state,
            &target("pane-1"),
            &request("new-session", &["-A"]),
        );
        assert_eq!(session.stdout(), Some("@lane-1\n"));
    }

    #[test]
    fn canonical_scope_and_every_non_mutating_disposition_are_observable() {
        let mut product = TmuxCompatProduct::default();
        let mut state = workspace();
        let missing = AgentTarget::new("window-1", "missing-lane", "pane-1");
        let failure = product.handle(&mut state, &missing, &request("list-panes", &[]));
        assert_eq!(failure.error().unwrap().code(), "target_not_found");
        assert_eq!(
            failure.error().unwrap().message(),
            "worklane missing-lane is unavailable"
        );

        for command in ["select-window", "rename-window", "new-window", "last-pane"] {
            let reply = product.handle(&mut state, &target("pane-1"), &request(command, &[]));
            assert_eq!(reply.stdout(), Some(""), "{command}");
        }
        for command in [
            "split-window",
            "send-keys",
            "kill-pane",
            "kill-window",
            "select-layout",
            "resize-pane",
            "wait-for",
            "save-buffer",
            "show-buffer",
            "set-buffer",
            "load-buffer",
            "capture-pane",
        ] {
            let reply = product.handle(&mut state, &target("pane-1"), &request(command, &[]));
            let error = reply.error().unwrap();
            assert_eq!(error.code(), "not_implemented", "{command}");
            assert!(error.message().contains("not implemented"), "{command}");
        }
    }

    #[test]
    fn send_keys_prepares_source_text_for_only_a_scoped_live_pane() {
        let state = workspace();
        let action = TmuxCompatProduct::prepare_send_keys(
            &state,
            &target("pane-1"),
            &request("send-keys", &["-t", "%pane-2", "echo", "ready", "Enter"]),
        )
        .unwrap();
        assert_eq!(
            action,
            TmuxProductAction::SendText {
                pane_id: "pane-2".to_owned(),
                text: "echo ready\r".to_owned(),
            }
        );

        let empty = TmuxCompatProduct::prepare_send_keys(
            &state,
            &target("pane-1"),
            &request("send-keys", &["-t", "%pane-2"]),
        )
        .unwrap();
        assert_eq!(empty, TmuxProductAction::Noop);

        let missing = TmuxCompatProduct::prepare_send_keys(
            &state,
            &target("pane-1"),
            &request("send-keys", &["-t", "%pane-other", "unsafe"]),
        );
        assert_eq!(missing.unwrap_err().0, "target_not_found");
    }

    #[test]
    fn split_plans_first_right_column_then_stacks_below_the_last_teammate() {
        let mut product = TmuxCompatProduct::default();
        let state = workspace();
        let first = product
            .prepare_split(
                &state,
                &target("pane-1"),
                &request(
                    "split-window",
                    &["-h", "-t", "%pane-2", "-P", "-F", "#{pane_id}"],
                ),
            )
            .unwrap();
        assert_eq!(first.leader_pane_id, "pane-1");
        assert_eq!(first.insertion_pane_id, "pane-1");
        assert_eq!(first.disposition, SplitDisposition::RightGolden);
        assert!(!first.detached);
        assert_eq!(first.print_format.as_deref(), Some("#{pane_id}"));

        let mut rendered_state = state.clone();
        assert!(rendered_state.split_focused_pane_right("pane-team-1"));
        assert!(rendered_state.select_worklane_and_pane("lane-1", "pane-1"));
        product.record_split(&first, "pane-team-1", Some(720));
        assert_eq!(
            product
                .split_reply(&rendered_state, &first, "pane-team-1")
                .stdout(),
            Some("%pane-team-1\n")
        );
        let second = product
            .prepare_split(
                &state,
                &target("pane-1"),
                &request("split-window", &["-v", "-d"]),
            )
            .unwrap();
        assert_eq!(second.leader_pane_id, "pane-1");
        assert_eq!(second.insertion_pane_id, "pane-team-1");
        assert_eq!(second.disposition, SplitDisposition::StackBelow);
        assert!(second.detached);
        assert_eq!(second.print_format, None);

        let missing = product.prepare_split(
            &state,
            &target("pane-1"),
            &request("split-window", &["-t", "%outside"]),
        );
        assert_eq!(missing.unwrap_err().0, "target_not_found");
    }
}
