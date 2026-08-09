use std::collections::BTreeMap;
use zentty_core::{AgentTarget, PaneState, WorklaneState, WorkspaceState};
use zentty_tmux_compat::{
    Command, FormatRenderer, PaneTarget, ParsedArguments, SendKeys, StoreError, TeamStore,
    TmuxCompatReply, TmuxCompatRequest, WaitForAction, WaitForSignals,
};

const DEFAULT_LIST_PANES: &str = "#{pane_id} #{pane_index} #{pane_title} #{?pane_active,*,-}";
const DEFAULT_LIST_WINDOWS: &str = "#{window_id} #{window_index} #{window_name}";
const DEFAULT_DISPLAY_MESSAGE: &str = "#{pane_id}";
const DEFAULT_SPLIT_PRINT: &str = "#{pane_id}";

#[derive(Default)]
pub(crate) struct TmuxCompatProduct {
    store: TeamStore,
    persistence: Option<crate::tmux_store::TmuxStoreFile>,
    wait_for: WaitForSignals,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum TmuxProductAction {
    Noop,
    SendText {
        pane_id: String,
        text: String,
        deferred_launch_command: Option<String>,
    },
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
    pub working_directory: Option<String>,
    pub launch_command: Option<String>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct RespawnPlan {
    pub pane_id: String,
    pub command: String,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct CapturePlan {
    pub pane_id: String,
    pub print: bool,
    pub line_limit: Option<usize>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct KillPlan {
    pub worklane_id: String,
    pub target_pane_id: String,
    pub pane_ids: Vec<String>,
    pub leader_pane_id: Option<String>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct KillRestoration {
    pub leader_pane_id: String,
    pub width: u32,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct LayoutPlan {
    pub equalize_pane_id: Option<String>,
    pub golden_leader_pane_id: Option<String>,
}

impl TmuxCompatProduct {
    pub(crate) fn persistent(
        persistence: crate::tmux_store::TmuxStoreFile,
    ) -> Result<Self, String> {
        let store = persistence.load()?;
        Ok(Self {
            store,
            persistence: Some(persistence),
            wait_for: WaitForSignals::default(),
        })
    }

    pub(crate) fn refresh(&mut self) -> Result<(), String> {
        if let Some(persistence) = &self.persistence {
            self.store = persistence.load()?;
        }
        Ok(())
    }

    fn mutate_store<T>(
        &mut self,
        mutation: impl FnOnce(&mut TeamStore) -> Result<T, StoreError>,
    ) -> Result<T, String> {
        if let Some(persistence) = &self.persistence {
            let (store, value) = persistence.mutate(mutation)?;
            self.store = store;
            Ok(value)
        } else {
            mutation(&mut self.store).map_err(|error| error.to_string())
        }
    }

    pub(crate) fn prepare_respawn(
        state: &WorkspaceState,
        target: &AgentTarget,
        request: &TmuxCompatRequest,
    ) -> Result<RespawnPlan, (&'static str, String)> {
        let worklane = target_worklane(state, target)?;
        let pane_ids = pane_entries(worklane)
            .into_iter()
            .map(|(_, pane, _)| pane.id.clone())
            .collect::<Vec<_>>();
        let (option_arguments, delimited_command) = request
            .arguments()
            .iter()
            .position(|argument| argument == "--")
            .map_or((request.arguments(), None), |delimiter| {
                (
                    &request.arguments()[..delimiter],
                    Some(&request.arguments()[delimiter + 1..]),
                )
            });
        let parsed = parsed(option_arguments, &["-c", "-e", "-t"], &["-k"]);
        if !parsed.has_flag("-k") {
            return Err((
                "invalid_arguments",
                "respawn-pane requires -k while replacing a live pane".to_owned(),
            ));
        }
        if parsed.value("-c").is_some() || parsed.value("-e").is_some() {
            return Err((
                "unsupported",
                "respawn-pane -c and -e are not supported by Zentty".to_owned(),
            ));
        }
        if delimited_command.is_some() && !parsed.positionals().is_empty() {
            return Err((
                "invalid_arguments",
                "respawn-pane has unexpected arguments before --".to_owned(),
            ));
        }
        validate_explicit_pane_target(parsed.value("-t"), &pane_ids)?;
        let pane_id = PaneTarget::resolve(parsed.value("-t"), &pane_ids, &target.pane_id);
        let command_arguments = delimited_command.unwrap_or(parsed.positionals());
        if command_arguments.is_empty() {
            return Err((
                "invalid_arguments",
                "respawn-pane requires a shell command".to_owned(),
            ));
        }
        if command_arguments
            .iter()
            .any(|argument| argument.is_empty() || argument.contains('\0'))
        {
            return Err((
                "invalid_arguments",
                "respawn-pane command arguments must be nonempty and contain no NUL bytes"
                    .to_owned(),
            ));
        }
        Ok(RespawnPlan {
            pane_id,
            command: tmux_shell_command(command_arguments),
        })
    }

    pub(crate) fn prepare_layout(
        &self,
        state: &WorkspaceState,
        target: &AgentTarget,
        request: &TmuxCompatRequest,
    ) -> Result<LayoutPlan, (&'static str, String)> {
        let worklane = target_worklane(state, target)?;
        let pane_ids = pane_entries(worklane)
            .into_iter()
            .map(|(_, pane, _)| pane.id.clone())
            .collect::<Vec<_>>();
        validate_explicit_pane_target(Some(&target.pane_id), &pane_ids)?;
        let anchor = self.store.anchor(&target.worklane_id);
        match request.command() {
            Command::SelectLayout => {
                let parsed = parsed(request.arguments(), &["-t"], &[]);
                let preset = parsed
                    .positionals()
                    .first()
                    .map_or("main-vertical", String::as_str);
                if !matches!(preset, "main-vertical" | "even-vertical") {
                    return Ok(LayoutPlan {
                        equalize_pane_id: None,
                        golden_leader_pane_id: None,
                    });
                }
                let equalize_pane_id = anchor
                    .and_then(|anchor| {
                        anchor
                            .column_pane_ids
                            .iter()
                            .find(|pane_id| pane_ids.contains(pane_id))
                            .cloned()
                    })
                    .unwrap_or_else(|| target.pane_id.clone());
                let golden_leader_pane_id = (preset == "main-vertical")
                    .then(|| anchor.map(|anchor| anchor.leader_pane_id.clone()))
                    .flatten()
                    .filter(|pane_id| pane_ids.contains(pane_id));
                Ok(LayoutPlan {
                    equalize_pane_id: Some(equalize_pane_id),
                    golden_leader_pane_id,
                })
            }
            Command::ResizePane => {
                let parsed = parsed(
                    request.arguments(),
                    &["-t", "-x", "-y"],
                    &["-D", "-L", "-R", "-U"],
                );
                validate_explicit_pane_target(parsed.value("-t"), &pane_ids)?;
                let golden_leader_pane_id = parsed
                    .value("-x")
                    .filter(|width| width.ends_with('%'))
                    .and(anchor)
                    .map(|anchor| anchor.leader_pane_id.clone())
                    .filter(|pane_id| pane_ids.contains(pane_id));
                Ok(LayoutPlan {
                    equalize_pane_id: None,
                    golden_leader_pane_id,
                })
            }
            _ => Err((
                "unsupported",
                "layout planner received a non-layout command".to_owned(),
            )),
        }
    }

    pub(crate) fn prepare_kill(
        &self,
        state: &WorkspaceState,
        target: &AgentTarget,
        request: &TmuxCompatRequest,
    ) -> Result<KillPlan, (&'static str, String)> {
        let worklane = target_worklane(state, target)?;
        let pane_ids = pane_entries(worklane)
            .into_iter()
            .map(|(_, pane, _)| pane.id.clone())
            .collect::<Vec<_>>();
        let target_pane_id = if request.command() == Command::KillWindow {
            validate_explicit_pane_target(Some(&target.pane_id), &pane_ids)?;
            self.store.anchor(&target.worklane_id).map_or_else(
                || target.pane_id.clone(),
                |anchor| anchor.leader_pane_id.clone(),
            )
        } else {
            let parsed = parsed(request.arguments(), &["-t"], &[]);
            validate_explicit_pane_target(parsed.value("-t"), &pane_ids)?;
            PaneTarget::resolve(parsed.value("-t"), &pane_ids, &target.pane_id)
        };
        let anchor = self.store.anchor(&target.worklane_id);
        let leader_pane_id = anchor.map(|anchor| anchor.leader_pane_id.clone());
        let mut panes_to_close = anchor
            .filter(|anchor| anchor.leader_pane_id == target_pane_id)
            .map_or_else(Vec::new, |anchor| anchor.column_pane_ids.clone());
        panes_to_close.retain(|pane_id| pane_ids.contains(pane_id));
        panes_to_close.push(target_pane_id.clone());
        Ok(KillPlan {
            worklane_id: target.worklane_id.clone(),
            target_pane_id,
            pane_ids: panes_to_close,
            leader_pane_id,
        })
    }

    pub(crate) fn complete_kill(
        &mut self,
        plan: &KillPlan,
    ) -> Result<Option<KillRestoration>, String> {
        let width = self
            .mutate_store(|store| Ok(store.remove_pane(&plan.worklane_id, &plan.target_pane_id)))?;
        Ok(width.and_then(|width| {
            Some(KillRestoration {
                leader_pane_id: plan.leader_pane_id.clone()?,
                width,
            })
        }))
    }

    pub(crate) fn prepare_capture(
        state: &WorkspaceState,
        target: &AgentTarget,
        request: &TmuxCompatRequest,
    ) -> Result<CapturePlan, (&'static str, String)> {
        let worklane = target_worklane(state, target)?;
        let pane_ids = pane_entries(worklane)
            .into_iter()
            .map(|(_, pane, _)| pane.id.clone())
            .collect::<Vec<_>>();
        let parsed = parsed(
            request.arguments(),
            &["-E", "-S", "-t"],
            &["-J", "-N", "-p"],
        );
        validate_explicit_pane_target(parsed.value("-t"), &pane_ids)?;
        let line_limit = parsed
            .value("-S")
            .and_then(|value| value.strip_prefix('-'))
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|limit| *limit > 0);
        Ok(CapturePlan {
            pane_id: PaneTarget::resolve(parsed.value("-t"), &pane_ids, &target.pane_id),
            print: parsed.has_flag("-p"),
            line_limit,
        })
    }

    pub(crate) fn complete_capture(&mut self, plan: &CapturePlan, text: &str) -> TmuxCompatReply {
        let text = plan
            .line_limit
            .map_or_else(|| text.to_owned(), |limit| tail_terminal_lines(text, limit));
        if plan.print {
            let output = if text.ends_with('\n') {
                text
            } else {
                format!("{text}\n")
            };
            return TmuxCompatReply::success(output).unwrap_or_else(|_| {
                failure("output_limit", "tmux compatibility output limit exceeded")
            });
        }
        self.mutate_store(|store| store.set_buffer("default", &text))
            .map_or_else(
                |error| failure("store_failed", error),
                |()| {
                    TmuxCompatReply::success(String::new())
                        .expect("empty compatibility output fits protocol limits")
                },
            )
    }

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
        if parsed
            .value("-c")
            .is_some_and(|path| path.is_empty() || path.contains('\0'))
            || parsed
                .positionals()
                .iter()
                .any(|argument| argument.is_empty() || argument.contains('\0'))
        {
            return Err((
                "invalid_arguments",
                "split-window launch paths and arguments must be nonempty and contain no NUL bytes"
                    .to_owned(),
            ));
        }
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
            working_directory: parsed.value("-c").map(str::to_owned),
            launch_command: (!parsed.positionals().is_empty())
                .then(|| tmux_shell_command(parsed.positionals())),
        })
    }

    pub(crate) fn record_split(
        &mut self,
        plan: &SplitPlan,
        new_pane_id: &str,
        pre_team_leader_width: Option<u32>,
    ) -> Result<(), String> {
        self.mutate_store(|store| {
            let _ = store.record_split(
                &plan.worklane_id,
                &plan.leader_pane_id,
                new_pane_id,
                plan.detached,
                pre_team_leader_width,
            );
            Ok(())
        })
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
            let deferred_launch_command = submitted_launch_command(&text);
            Ok(TmuxProductAction::SendText {
                pane_id,
                text,
                deferred_launch_command,
            })
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
            Command::SaveBuffer | Command::ShowBuffer => Ok(self.save_buffer(request.arguments())),
            Command::SetBuffer | Command::LoadBuffer => {
                self.set_buffer(request.arguments(), request.standard_input())
            }
            Command::NewSession => Ok(format!("@{}\n", target.worklane_id)),
            Command::WaitFor => self.wait_for(request.arguments()),
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

    fn wait_for(&mut self, arguments: &[String]) -> Result<String, (&'static str, String)> {
        match WaitForAction::parse(arguments) {
            Ok(WaitForAction::Signal(name)) => self
                .wait_for
                .signal(name)
                .map(|()| String::new())
                .map_err(|message| ("wait_capacity", message.to_owned())),
            Ok(WaitForAction::Wait { name, .. }) if self.wait_for.consume(&name) => {
                Ok(String::new())
            }
            Ok(WaitForAction::Wait { .. }) => {
                Err(("wait_pending", "wait-for signal is not pending".to_owned()))
            }
            Err(message) => Err(("invalid_arguments", message.to_owned())),
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
        self.mutate_store(|store| {
            store.record_active_pane(&target.worklane_id, &pane_id);
            Ok(())
        })
        .map_err(|error| ("store_failed", error))?;
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

    fn save_buffer(&self, arguments: &[String]) -> String {
        let parsed = parsed(arguments, &["-b"], &[]);
        self.store.buffer(parsed.value("-b")).to_owned()
    }

    fn set_buffer(
        &mut self,
        arguments: &[String],
        standard_input: Option<&str>,
    ) -> Result<String, (&'static str, String)> {
        let parsed = parsed(arguments, &["-b"], &[]);
        self.mutate_store(|store| {
            store.set_buffer(
                parsed.value("-b").unwrap_or("default"),
                standard_input.unwrap_or(""),
            )
        })
        .map(|()| String::new())
        .map_err(|error| ("store_failed", error))
    }
}

fn parsed(arguments: &[String], values: &[&str], flags: &[&str]) -> ParsedArguments {
    ParsedArguments::parse(arguments, &strings(values), &strings(flags))
}

fn tmux_shell_command(arguments: &[String]) -> String {
    if let [command] = arguments {
        return command.clone();
    }
    arguments
        .iter()
        .map(|argument| shell_quote(argument))
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn shell_wrapped_command(command: &str, login_shell_path: Option<&str>) -> String {
    let shell = login_shell_path
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .filter(|path| {
            std::path::Path::new(path)
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| matches!(name, "bash" | "fish" | "zsh"))
        });
    shell.map_or_else(
        || format!("sh -c {}", shell_quote(command.trim())),
        |shell| {
            format!(
                "{} -lic {}",
                shell_quote(shell),
                shell_quote(command.trim())
            )
        },
    )
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn submitted_launch_command(text: &str) -> Option<String> {
    let without_submit = text
        .strip_suffix('\r')
        .or_else(|| text.strip_suffix('\n'))?;
    let command = without_submit.trim();
    (!command.is_empty() && !command.contains(['\r', '\n'])).then(|| command.to_owned())
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

fn tail_terminal_lines(text: &str, max_lines: usize) -> String {
    let had_trailing_newline = text.ends_with('\n');
    let mut lines = text.split('\n').collect::<Vec<_>>();
    if had_trailing_newline {
        debug_assert_eq!(lines.last(), Some(&""));
        lines.pop();
    }
    let start = lines.len().saturating_sub(max_lines);
    let mut output = lines[start..].join("\n");
    if had_trailing_newline && !output.is_empty() {
        output.push('\n');
    }
    output
}

fn failure(code: &'static str, message: impl Into<String>) -> TmuxCompatReply {
    TmuxCompatReply::failure(code, message).expect("static product diagnostic fits protocol limits")
}

#[cfg(test)]
mod tests {
    use super::{
        KillRestoration, LayoutPlan, SplitDisposition, TmuxCompatProduct, TmuxProductAction,
        shell_wrapped_command,
    };
    use zentty_core::AgentTarget;
    use zentty_core::WorkspaceState;
    use zentty_tmux_compat::TmuxCompatRequest;

    struct PersistentStoreFixture(std::path::PathBuf);

    impl PersistentStoreFixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "zentty-tmux-product-persistence-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn store(&self) -> crate::tmux_store::TmuxStoreFile {
            crate::tmux_store::TmuxStoreFile::new(self.0.join("store.json"))
        }
    }

    impl Drop for PersistentStoreFixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }

    fn request(command: &str, arguments: &[&str]) -> TmuxCompatRequest {
        TmuxCompatRequest::new(
            1,
            command,
            arguments.iter().map(|value| (*value).to_owned()).collect(),
            None,
        )
        .unwrap()
    }

    fn request_with_input(command: &str, arguments: &[&str], input: &str) -> TmuxCompatRequest {
        TmuxCompatRequest::new(
            1,
            command,
            arguments.iter().map(|value| (*value).to_owned()).collect(),
            Some(input.to_owned()),
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
    fn wait_for_signal_is_instance_scoped_and_consumed_once() {
        let mut first = TmuxCompatProduct::default();
        let mut second = TmuxCompatProduct::default();
        let mut state = workspace();
        let pending = first.handle(
            &mut state,
            &target("pane-1"),
            &request("wait-for", &["agent-ready"]),
        );
        assert_eq!(pending.error().unwrap().code(), "wait_pending");

        assert!(
            first
                .handle(
                    &mut state,
                    &target("pane-2"),
                    &request("wait-for", &["-S", "agent-ready"]),
                )
                .is_ok()
        );
        assert!(
            first
                .handle(
                    &mut state,
                    &target("pane-1"),
                    &request("wait-for", &["agent-ready"]),
                )
                .is_ok()
        );
        let consumed = first.handle(
            &mut state,
            &target("pane-1"),
            &request("wait-for", &["agent-ready"]),
        );
        assert_eq!(consumed.error().unwrap().code(), "wait_pending");
        let isolated = second.handle(
            &mut state,
            &target("pane-1"),
            &request("wait-for", &["agent-ready"]),
        );
        assert_eq!(isolated.error().unwrap().code(), "wait_pending");
    }

    #[test]
    fn wait_for_rejects_bad_names_and_preserves_independent_signals() {
        let mut product = TmuxCompatProduct::default();
        let mut state = workspace();
        let invalid = product.handle(
            &mut state,
            &target("pane-1"),
            &request("wait-for", &["line\nbreak"]),
        );
        assert_eq!(invalid.error().unwrap().code(), "invalid_arguments");

        for name in ["first", "second"] {
            assert!(
                product
                    .handle(
                        &mut state,
                        &target("pane-1"),
                        &request("wait-for", &["-S", name]),
                    )
                    .is_ok()
            );
        }
        assert!(
            product
                .handle(
                    &mut state,
                    &target("pane-1"),
                    &request("wait-for", &["second"]),
                )
                .is_ok()
        );
        assert!(
            product
                .handle(
                    &mut state,
                    &target("pane-1"),
                    &request("wait-for", &["first"]),
                )
                .is_ok()
        );
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
        for command in ["split-window", "send-keys", "capture-pane"] {
            let reply = product.handle(&mut state, &target("pane-1"), &request(command, &[]));
            let error = reply.error().unwrap();
            assert_eq!(error.code(), "not_implemented", "{command}");
            assert!(error.message().contains("not implemented"), "{command}");
        }
    }

    #[test]
    fn layout_plans_target_the_team_without_trusting_ambient_focus() {
        let mut state = WorkspaceState::new("lane-1", "leader");
        assert!(state.split_focused_pane_right("teammate-1"));
        assert!(state.split_focused_pane_below("teammate-2"));
        assert!(state.select_worklane_and_pane("lane-1", "leader"));
        assert!(state.create_worklane("lane-2", "foreground"));
        assert_eq!(state.active_worklane_id(), "lane-2");
        let mut product = TmuxCompatProduct::default();
        let _ = product
            .store
            .record_split("lane-1", "leader", "teammate-1", false, Some(777));
        let _ = product
            .store
            .record_split("lane-1", "leader", "teammate-2", true, None);

        let main = product
            .prepare_layout(
                &state,
                &target("leader"),
                &request("select-layout", &["-t", "@foreign", "main-vertical"]),
            )
            .unwrap();
        assert_eq!(
            main,
            LayoutPlan {
                equalize_pane_id: Some("teammate-1".to_owned()),
                golden_leader_pane_id: Some("leader".to_owned()),
            }
        );
        let even = product
            .prepare_layout(
                &state,
                &target("leader"),
                &request("select-layout", &["even-vertical"]),
            )
            .unwrap();
        assert_eq!(even.equalize_pane_id.as_deref(), Some("teammate-1"));
        assert_eq!(even.golden_leader_pane_id, None);
        let ignored = product
            .prepare_layout(
                &state,
                &target("leader"),
                &request("select-layout", &["tiled"]),
            )
            .unwrap();
        assert_eq!(
            ignored,
            LayoutPlan {
                equalize_pane_id: None,
                golden_leader_pane_id: None,
            }
        );

        let resized = product
            .prepare_layout(
                &state,
                &target("leader"),
                &request("resize-pane", &["-t", "%teammate-2", "-x", "30%"]),
            )
            .unwrap();
        assert_eq!(resized.equalize_pane_id, None);
        assert_eq!(resized.golden_leader_pane_id.as_deref(), Some("leader"));
        let absolute = product
            .prepare_layout(
                &state,
                &target("leader"),
                &request("resize-pane", &["-x", "30"]),
            )
            .unwrap();
        assert_eq!(absolute.golden_leader_pane_id, None);
        let outside = product.prepare_layout(
            &state,
            &target("leader"),
            &request("resize-pane", &["-t", "%outside", "-x", "30%"]),
        );
        assert_eq!(outside.unwrap_err().0, "target_not_found");
        let wrong_command =
            product.prepare_layout(&state, &target("leader"), &request("list-panes", &[]));
        assert_eq!(wrong_command.unwrap_err().0, "unsupported");
        assert_eq!(state.active_worklane_id(), "lane-2");
    }

    #[test]
    fn kill_plans_cascade_leaders_and_restore_only_the_final_teammate() {
        let mut state = WorkspaceState::new("lane-1", "leader");
        assert!(state.split_focused_pane_right("teammate-1"));
        assert!(state.split_focused_pane_below("teammate-2"));
        assert!(state.select_worklane_and_pane("lane-1", "leader"));
        let mut product = TmuxCompatProduct::default();
        let _ = product
            .store
            .record_split("lane-1", "leader", "teammate-1", false, Some(777));
        let _ = product
            .store
            .record_split("lane-1", "leader", "teammate-2", true, None);

        let first = product
            .prepare_kill(
                &state,
                &target("leader"),
                &request("kill-pane", &["-t", "%teammate-1"]),
            )
            .unwrap();
        assert_eq!(first.pane_ids, ["teammate-1"]);
        assert_eq!(product.complete_kill(&first), Ok(None));

        let final_teammate = product
            .prepare_kill(
                &state,
                &target("leader"),
                &request("kill-pane", &["-t", "%teammate-2"]),
            )
            .unwrap();
        assert_eq!(
            product.complete_kill(&final_teammate),
            Ok(Some(KillRestoration {
                leader_pane_id: "leader".to_owned(),
                width: 777,
            }))
        );

        let _ = product
            .store
            .record_split("lane-1", "leader", "teammate-1", false, Some(777));
        let _ = product
            .store
            .record_split("lane-1", "leader", "teammate-2", true, None);
        let leader = product
            .prepare_kill(&state, &target("leader"), &request("kill-window", &[]))
            .unwrap();
        assert_eq!(leader.target_pane_id, "leader");
        assert_eq!(leader.pane_ids, ["teammate-1", "teammate-2", "leader"]);
        assert_eq!(product.complete_kill(&leader), Ok(None));
        assert!(product.store.anchor("lane-1").is_none());

        let missing = product.prepare_kill(
            &state,
            &target("leader"),
            &request("kill-pane", &["-t", "%outside"]),
        );
        assert_eq!(missing.unwrap_err().0, "target_not_found");
    }

    #[test]
    fn send_keys_preserves_text_and_recognizes_only_one_submitted_launch_command() {
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
                deferred_launch_command: Some("echo ready".to_owned()),
            }
        );

        let incomplete = TmuxCompatProduct::prepare_send_keys(
            &state,
            &target("pane-1"),
            &request("send-keys", &["-t", "%pane-2", "echo", "ready"]),
        )
        .unwrap();
        assert_eq!(
            incomplete,
            TmuxProductAction::SendText {
                pane_id: "pane-2".to_owned(),
                text: "echo ready".to_owned(),
                deferred_launch_command: None,
            }
        );

        let multiline = TmuxCompatProduct::prepare_send_keys(
            &state,
            &target("pane-1"),
            &request(
                "send-keys",
                &["-t", "%pane-2", "one", "Enter", "two", "Enter"],
            ),
        )
        .unwrap();
        assert_eq!(
            multiline,
            TmuxProductAction::SendText {
                pane_id: "pane-2".to_owned(),
                text: "one\rtwo\r".to_owned(),
                deferred_launch_command: None,
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
    fn deferred_launch_uses_supported_login_shells_and_safe_fallback_quoting() {
        assert_eq!(
            shell_wrapped_command("printf '%s' ready", Some("/bin/bash")),
            "'/bin/bash' -lic 'printf '\\''%s'\\'' ready'"
        );
        assert_eq!(
            shell_wrapped_command("echo $HOME", Some("/usr/bin/nu")),
            "sh -c 'echo $HOME'"
        );
        assert_eq!(
            shell_wrapped_command("echo ready", Some("  ")),
            "sh -c 'echo ready'"
        );
    }

    #[test]
    fn source_buffer_commands_preserve_named_sorted_and_stdin_semantics() {
        let mut product = TmuxCompatProduct::default();
        let mut state = workspace();
        for (name, value) in [("z-last", "last"), ("a-first", "first")] {
            let reply = product.handle(
                &mut state,
                &target("pane-1"),
                &request_with_input("set-buffer", &["-b", name], value),
            );
            assert_eq!(reply.stdout(), Some(""));
        }
        let sorted = product.handle(&mut state, &target("pane-1"), &request("save-buffer", &[]));
        assert_eq!(sorted.stdout(), Some("first"));
        let named = product.handle(
            &mut state,
            &target("pane-1"),
            &request("show-buffer", &["-b", "z-last"]),
        );
        assert_eq!(named.stdout(), Some("last"));
        let loaded = product.handle(
            &mut state,
            &target("pane-1"),
            &request_with_input("load-buffer", &[], "default-value"),
        );
        assert_eq!(loaded.stdout(), Some(""));
        let default = product.handle(
            &mut state,
            &target("pane-1"),
            &request("show-buffer", &["-b", "default"]),
        );
        assert_eq!(default.stdout(), Some("default-value"));
    }

    #[test]
    fn persistent_product_loads_on_construction_and_refreshes_external_mutations() {
        let fixture = PersistentStoreFixture::new();
        let mut first = TmuxCompatProduct::persistent(fixture.store()).unwrap();
        let mut state = workspace();
        let set = first.handle(
            &mut state,
            &target("pane-1"),
            &request_with_input("set-buffer", &["-b", "agent"], "first"),
        );
        assert_eq!(set.stdout(), Some(""));

        let mut second = TmuxCompatProduct::persistent(fixture.store()).unwrap();
        let loaded = second.handle(
            &mut state,
            &target("pane-1"),
            &request("show-buffer", &["-b", "agent"]),
        );
        assert_eq!(loaded.stdout(), Some("first"));
        let changed = second.handle(
            &mut state,
            &target("pane-1"),
            &request_with_input("set-buffer", &["-b", "agent"], "second"),
        );
        assert_eq!(changed.stdout(), Some(""));

        first.refresh().unwrap();
        let refreshed = first.handle(
            &mut state,
            &target("pane-1"),
            &request("show-buffer", &["-b", "agent"]),
        );
        assert_eq!(refreshed.stdout(), Some("second"));
    }

    #[test]
    fn capture_plan_scopes_targets_and_completion_prints_or_buffers_exact_text() {
        let state = workspace();
        let print = TmuxCompatProduct::prepare_capture(
            &state,
            &target("pane-1"),
            &request("capture-pane", &["-p", "-J", "-S", "-2", "-t", "%pane-2"]),
        )
        .unwrap();
        assert_eq!(print.pane_id, "pane-2");
        assert!(print.print);
        assert_eq!(print.line_limit, Some(2));

        let mut product = TmuxCompatProduct::default();
        let printed = product.complete_capture(&print, "one\ntwo\nthree\n");
        assert_eq!(printed.stdout(), Some("two\nthree\n"));

        let buffered = TmuxCompatProduct::prepare_capture(
            &state,
            &target("pane-1"),
            &request("capture-pane", &["-S", "0"]),
        )
        .unwrap();
        assert_eq!(buffered.line_limit, None);
        assert_eq!(
            product.complete_capture(&buffered, "buffered").stdout(),
            Some("")
        );
        let mut state = state;
        assert_eq!(
            product
                .handle(
                    &mut state,
                    &target("pane-1"),
                    &request("show-buffer", &["-b", "default"]),
                )
                .stdout(),
            Some("buffered")
        );

        let no_lines = TmuxCompatProduct::prepare_capture(
            &state,
            &target("pane-1"),
            &request("capture-pane", &["-S", "-0"]),
        )
        .unwrap();
        assert_eq!(no_lines.line_limit, None);

        let tail = TmuxCompatProduct::prepare_capture(
            &state,
            &target("pane-1"),
            &request("capture-pane", &["-S", "-2"]),
        )
        .unwrap();
        assert_eq!(
            product.complete_capture(&tail, "one\ntwo\nthree").stdout(),
            Some("")
        );
        assert_eq!(
            product
                .handle(
                    &mut state,
                    &target("pane-1"),
                    &request("show-buffer", &["-b", "default"]),
                )
                .stdout(),
            Some("two\nthree")
        );
        assert_eq!(
            product
                .complete_capture(&tail, "one\ntwo\nthree\n")
                .stdout(),
            Some("")
        );
        assert_eq!(
            product
                .handle(
                    &mut state,
                    &target("pane-1"),
                    &request("show-buffer", &["-b", "default"]),
                )
                .stdout(),
            Some("two\nthree\n")
        );

        let missing = TmuxCompatProduct::prepare_capture(
            &state,
            &target("pane-1"),
            &request("capture-pane", &["-t", "%outside"]),
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
        assert_eq!(first.working_directory, None);
        assert_eq!(first.launch_command, None);

        let launched = product
            .prepare_split(
                &state,
                &target("pane-1"),
                &request(
                    "split-window",
                    &["-h", "-c", "/repo", "printf", "%s", "team ready"],
                ),
            )
            .unwrap();
        assert_eq!(launched.working_directory.as_deref(), Some("/repo"));
        assert_eq!(
            launched.launch_command.as_deref(),
            Some("'printf' '%s' 'team ready'")
        );

        let mut rendered_state = state.clone();
        assert!(rendered_state.split_focused_pane_right("pane-team-1"));
        assert!(rendered_state.select_worklane_and_pane("lane-1", "pane-1"));
        product
            .record_split(&first, "pane-team-1", Some(720))
            .unwrap();
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
        assert_eq!(second.working_directory, None);
        assert_eq!(second.launch_command, None);

        let missing = product.prepare_split(
            &state,
            &target("pane-1"),
            &request("split-window", &["-t", "%outside"]),
        );
        assert_eq!(missing.unwrap_err().0, "target_not_found");

        for arguments in [
            vec!["-c", ""],
            vec!["-c", "bad\0cwd"],
            vec![""],
            vec!["printf", "bad\0argument"],
        ] {
            let invalid = product.prepare_split(
                &state,
                &target("pane-1"),
                &request("split-window", &arguments),
            );
            assert_eq!(invalid.unwrap_err().0, "invalid_arguments");
        }
    }

    #[test]
    fn respawn_plan_resolves_one_existing_target_and_one_shell_command() {
        let state = workspace();
        let plan = TmuxCompatProduct::prepare_respawn(
            &state,
            &target("pane-1"),
            &request(
                "respawn-pane",
                &["-k", "-t", "%pane-2", "exec claude --agent-id probe"],
            ),
        )
        .unwrap();
        assert_eq!(plan.pane_id, "pane-2");
        assert_eq!(plan.command, "exec claude --agent-id probe");

        let missing = TmuxCompatProduct::prepare_respawn(
            &state,
            &target("pane-1"),
            &request("respawn-pane", &["-k", "-t", "%outside", "true"]),
        );
        assert_eq!(missing.unwrap_err().0, "target_not_found");

        let live_without_kill = TmuxCompatProduct::prepare_respawn(
            &state,
            &target("pane-1"),
            &request("respawn-pane", &["-t", "%pane-2", "true"]),
        );
        assert_eq!(live_without_kill.unwrap_err().0, "invalid_arguments");

        for unsupported in ["-c", "-e"] {
            let rejected = TmuxCompatProduct::prepare_respawn(
                &state,
                &target("pane-1"),
                &request(
                    "respawn-pane",
                    &["-k", "-t", "%pane-2", unsupported, "value", "true"],
                ),
            );
            assert_eq!(rejected.unwrap_err().0, "unsupported");
        }

        let direct = TmuxCompatProduct::prepare_respawn(
            &state,
            &target("pane-1"),
            &request(
                "respawn-pane",
                &["-k", "-t", "%pane-2", "printf", "%s", "a'b", "$(false)"],
            ),
        )
        .unwrap();
        assert_eq!(direct.command, "'printf' '%s' 'a'\\''b' '$(false)'");

        let delimited = TmuxCompatProduct::prepare_respawn(
            &state,
            &target("pane-1"),
            &request(
                "respawn-pane",
                &["-k", "-t", "%pane-2", "--", "cd /tmp && exec claude"],
            ),
        )
        .unwrap();
        assert_eq!(delimited.command, "cd /tmp && exec claude");

        let delimited_direct = TmuxCompatProduct::prepare_respawn(
            &state,
            &target("pane-1"),
            &request(
                "respawn-pane",
                &["-k", "-t", "%pane-2", "--", "env", "-e", "value"],
            ),
        )
        .unwrap();
        assert_eq!(delimited_direct.command, "'env' '-e' 'value'");

        for arguments in [
            vec!["-k", "-t", "%pane-2"],
            vec!["-k", "-t", "%pane-2", "--"],
            vec!["-k", "-t", "%pane-2", ""],
            vec!["-k", "-t", "%pane-2", "bad\0command"],
            vec!["-k", "unexpected", "--", "true"],
        ] {
            let invalid = TmuxCompatProduct::prepare_respawn(
                &state,
                &target("pane-1"),
                &request("respawn-pane", &arguments),
            );
            assert_eq!(invalid.unwrap_err().0, "invalid_arguments");
        }
    }
}
