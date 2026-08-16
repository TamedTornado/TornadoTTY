#[derive(Debug, Eq, PartialEq)]
pub(super) enum ShellSignal {
    State {
        state: String,
        command: Option<String>,
    },
    RootPid {
        event: String,
        pid: Option<u32>,
    },
    Context {
        scope: String,
        path: Option<String>,
    },
}

pub(super) fn parse_shell_signal(
    arguments: &[String],
) -> Result<ShellSignal, (&'static str, String)> {
    let Some(kind) = arguments.first().map(String::as_str) else {
        return Err(("invalid_request", "shell signal kind is missing".to_owned()));
    };
    let mut positionals = Vec::new();
    let mut options = std::collections::BTreeMap::new();
    let mut index = 1;
    while index < arguments.len() {
        let argument = &arguments[index];
        if let Some(option) = argument.strip_prefix("--") {
            let Some(value) = arguments.get(index + 1) else {
                return Err((
                    "invalid_request",
                    format!("shell signal option {argument} is missing its value"),
                ));
            };
            if options.insert(option, value.clone()).is_some() {
                return Err((
                    "invalid_request",
                    format!("duplicate shell signal option {argument}"),
                ));
            }
            index += 2;
        } else {
            positionals.push(argument.clone());
            index += 1;
        }
    }
    match kind {
        "shell-state" => parse_shell_state(&positionals, &options),
        "pane-root-pid" => parse_root_pid(&positionals, &options),
        "pane-context" => parse_context(&positionals, &options),
        value => Err((
            "invalid_request",
            format!("unsupported shell signal kind {value:?}"),
        )),
    }
}

fn parse_shell_state(
    positionals: &[String],
    options: &std::collections::BTreeMap<&str, String>,
) -> Result<ShellSignal, (&'static str, String)> {
    validate_options(
        options,
        &[
            "command",
            "tool",
            "session-id",
            "parent-session-id",
            "origin",
        ],
    )?;
    let Some(state) = positionals.first() else {
        return Err(("invalid_request", "shell state is missing".to_owned()));
    };
    let state = match state.as_str() {
        "prompt" | "idle" => "prompt",
        "running" | "busy" | "command" => "running",
        "clear" | "unknown" => "unknown",
        value => {
            return Err((
                "invalid_request",
                format!("unsupported shell state {value:?}"),
            ));
        }
    };
    if positionals.len() != 1 {
        return Err((
            "invalid_request",
            "shell state has unexpected positional arguments".to_owned(),
        ));
    }
    Ok(ShellSignal::State {
        state: state.to_owned(),
        command: options.get("command").cloned(),
    })
}

fn parse_root_pid(
    positionals: &[String],
    options: &std::collections::BTreeMap<&str, String>,
) -> Result<ShellSignal, (&'static str, String)> {
    validate_options(options, &["session-id", "parent-session-id", "origin"])?;
    let (event, pid) = match positionals {
        [event, pid] if event == "attach" => (
            event,
            Some(
                pid.parse::<u32>()
                    .ok()
                    .filter(|pid| *pid > 0)
                    .ok_or_else(|| ("invalid_request", "pane root PID is invalid".to_owned()))?,
            ),
        ),
        [event] if event == "clear" => (event, None),
        [] => {
            return Err((
                "invalid_request",
                "pane root PID event is missing".to_owned(),
            ));
        }
        [event, ..] if event == "attach" || event == "clear" => {
            return Err((
                "invalid_request",
                "pane root PID signal has unexpected positional arguments".to_owned(),
            ));
        }
        [event, ..] => {
            return Err((
                "invalid_request",
                format!("unsupported pane root PID event {event:?}"),
            ));
        }
    };
    Ok(ShellSignal::RootPid {
        event: event.clone(),
        pid,
    })
}

fn parse_context(
    positionals: &[String],
    options: &std::collections::BTreeMap<&str, String>,
) -> Result<ShellSignal, (&'static str, String)> {
    validate_options(
        options,
        &[
            "path",
            "home",
            "user",
            "host",
            "git-branch",
            "session-id",
            "parent-session-id",
            "origin",
        ],
    )?;
    let Some(scope) = positionals.first() else {
        return Err((
            "invalid_request",
            "pane context scope is missing".to_owned(),
        ));
    };
    if positionals.len() != 1 || !["local", "remote", "clear"].contains(&scope.as_str()) {
        return Err((
            "invalid_request",
            format!("unsupported pane context scope {scope:?}"),
        ));
    }
    let path = if scope == "clear" {
        if options.contains_key("path") {
            return Err((
                "invalid_request",
                "cleared pane context may not include a path".to_owned(),
            ));
        }
        None
    } else {
        Some(
            options
                .get("path")
                .filter(|path| !path.is_empty())
                .cloned()
                .ok_or_else(|| ("invalid_request", "pane context path is missing".to_owned()))?,
        )
    };
    Ok(ShellSignal::Context {
        scope: scope.clone(),
        path,
    })
}

fn validate_options(
    options: &std::collections::BTreeMap<&str, String>,
    allowed: &[&str],
) -> Result<(), (&'static str, String)> {
    if let Some(option) = options.keys().find(|option| !allowed.contains(option)) {
        return Err((
            "invalid_request",
            format!("unsupported shell signal option --{option}"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ShellSignal, parse_shell_signal};

    #[test]
    fn signals_preserve_hostile_values_and_canonicalize_activity() {
        assert_eq!(
            parse_shell_signal(&[
                "shell-state".to_owned(),
                "busy".to_owned(),
                "--command".to_owned(),
                "printf 'hello world'".to_owned(),
            ]),
            Ok(ShellSignal::State {
                state: "running".to_owned(),
                command: Some("printf 'hello world'".to_owned()),
            })
        );
        assert_eq!(
            parse_shell_signal(&[
                "shell-state".to_owned(),
                "--command".to_owned(),
                "printf 'options first'".to_owned(),
                "running".to_owned(),
            ]),
            Ok(ShellSignal::State {
                state: "running".to_owned(),
                command: Some("printf 'options first'".to_owned()),
            })
        );
        assert_eq!(
            parse_shell_signal(&["pane-root-pid".to_owned(), "clear".to_owned()]),
            Ok(ShellSignal::RootPid {
                event: "clear".to_owned(),
                pid: None,
            })
        );
        assert_eq!(
            parse_shell_signal(&[
                "pane-context".to_owned(),
                "local".to_owned(),
                "--path".to_owned(),
                "/tmp/space and λ".to_owned(),
                "--git-branch".to_owned(),
                String::new(),
            ]),
            Ok(ShellSignal::Context {
                scope: "local".to_owned(),
                path: Some("/tmp/space and λ".to_owned()),
            })
        );
        assert_eq!(
            parse_shell_signal(&[
                "pane-root-pid".to_owned(),
                "attach".to_owned(),
                "42".to_owned(),
            ]),
            Ok(ShellSignal::RootPid {
                event: "attach".to_owned(),
                pid: Some(42)
            })
        );
    }

    #[test]
    fn malformed_signals_fail_closed() {
        for arguments in [
            vec!["pane-root-pid", "attach"],
            vec!["pane-root-pid", "attach", "42", "extra"],
            vec!["pane-root-pid", "clear", "extra"],
        ] {
            assert_eq!(
                parse_shell_signal(&arguments.into_iter().map(str::to_owned).collect::<Vec<_>>(),),
                Err((
                    "invalid_request",
                    "pane root PID signal has unexpected positional arguments".to_owned(),
                ))
            );
        }
        assert_eq!(
            parse_shell_signal(&[
                "pane-root-pid".to_owned(),
                "elsewhere".to_owned(),
                "extra".to_owned(),
            ]),
            Err((
                "invalid_request",
                "unsupported pane root PID event \"elsewhere\"".to_owned(),
            ))
        );
        for arguments in [
            vec![],
            vec!["unknown"],
            vec!["shell-state"],
            vec!["shell-state", "prompt", "extra"],
            vec!["shell-state", "prompt", "--command"],
            vec!["shell-state", "prompt", "--wat", "value"],
            vec!["pane-root-pid", "attach", "0"],
            vec!["pane-root-pid", "attach", "not-a-pid"],
            vec!["pane-context", "elsewhere"],
            vec!["pane-context", "local"],
            vec!["pane-context", "local", "extra", "--path", "/tmp"],
            vec!["pane-context", "local", "--path", ""],
            vec!["pane-context", "clear", "--path", "/tmp"],
            vec!["pane-context", "local", "--path", "/tmp", "--path", "/var"],
        ] {
            assert!(
                parse_shell_signal(&arguments.into_iter().map(str::to_owned).collect::<Vec<_>>())
                    .is_err()
            );
        }
    }
}
