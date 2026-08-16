use zentty_core::{AgentEvent, AgentSignalConfidence, AgentSignalOrigin};

#[derive(Debug, Eq, PartialEq)]
pub(super) enum AgentLifecycleSignal {
    Event {
        event: AgentEvent,
        origin: AgentSignalOrigin,
        confidence: AgentSignalConfidence,
    },
    AttachPid {
        pid: i32,
        tool: Option<String>,
        session_id: Option<String>,
        parent_session_id: Option<String>,
    },
    ClearPid {
        session_id: Option<String>,
    },
}

pub(super) fn parse_agent_lifecycle_signal(
    arguments: &[String],
) -> Result<AgentLifecycleSignal, (&'static str, String)> {
    let parsed = super::signal_arguments::parse_signal_arguments(arguments, "agent signal")?;
    match parsed.kind.as_str() {
        "lifecycle" => parse_lifecycle(&parsed.positionals, &parsed.options),
        "pid" => parse_pid(&parsed.positionals, &parsed.options),
        kind => Err((
            "invalid_request",
            format!("unsupported agent lifecycle signal kind {kind:?}"),
        )),
    }
}

fn parse_lifecycle(
    positionals: &[String],
    options: &std::collections::BTreeMap<String, String>,
) -> Result<AgentLifecycleSignal, (&'static str, String)> {
    super::signal_arguments::validate_signal_options(
        options,
        &[
            "window-id",
            "worklane-id",
            "pane-id",
            "origin",
            "tool",
            "text",
            "interaction-kind",
            "confidence",
            "session-id",
            "parent-session-id",
            "artifact-kind",
            "artifact-label",
            "artifact-url",
        ],
    )?;
    validate_common_values(options)?;
    let [verb] = positionals else {
        return Err((
            "invalid_request",
            "agent lifecycle signal requires exactly one state".to_owned(),
        ));
    };
    let event_name = match verb.as_str() {
        "running" => "agent.running",
        "needs-input" => "agent.needs-input",
        "idle" | "completed" => "agent.idle",
        "clear" => "session.end",
        value => {
            return Err((
                "invalid_request",
                format!("unsupported agent lifecycle state {value:?}"),
            ));
        }
    };
    let interaction = options
        .get("interaction-kind")
        .filter(|kind| kind.as_str() != "none")
        .map(|kind| {
            serde_json::json!({
                "kind": kind,
                "text": options.get("text"),
            })
        });
    let value = serde_json::json!({
        "version": 1,
        "event": event_name,
        "agent": {"name": options.get("tool")},
        "session": {
            "id": options.get("session-id"),
            "parentId": options.get("parent-session-id"),
        },
        "state": {
            "text": options.get("text"),
            "interaction": interaction,
        },
        "artifact": {
            "kind": options.get("artifact-kind"),
            "label": options.get("artifact-label"),
            "url": options.get("artifact-url"),
        },
    });
    let bytes = serde_json::to_vec(&value).map_err(|error| {
        (
            "invalid_request",
            format!("could not encode agent lifecycle signal: {error}"),
        )
    })?;
    let event =
        AgentEvent::parse(&bytes).map_err(|error| ("invalid_request", error.to_string()))?;
    let origin = match options
        .get("origin")
        .map(String::as_str)
        .unwrap_or("compatibility")
    {
        "explicit-hook" => AgentSignalOrigin::ExplicitHook,
        "explicit-api" => AgentSignalOrigin::ExplicitApi,
        "heuristic" => AgentSignalOrigin::Heuristic,
        "shell" => AgentSignalOrigin::Shell,
        "inferred" => AgentSignalOrigin::Inferred,
        _ => AgentSignalOrigin::Compatibility,
    };
    let confidence = options.get("confidence").map_or_else(
        || match origin {
            AgentSignalOrigin::ExplicitHook | AgentSignalOrigin::ExplicitApi => {
                AgentSignalConfidence::Explicit
            }
            AgentSignalOrigin::Heuristic | AgentSignalOrigin::Compatibility => {
                AgentSignalConfidence::Strong
            }
            AgentSignalOrigin::Shell | AgentSignalOrigin::Inferred => AgentSignalConfidence::Weak,
        },
        |value| match value.as_str() {
            "explicit" => AgentSignalConfidence::Explicit,
            "strong" => AgentSignalConfidence::Strong,
            _ => AgentSignalConfidence::Weak,
        },
    );
    Ok(AgentLifecycleSignal::Event {
        event,
        origin,
        confidence,
    })
}

fn parse_pid(
    positionals: &[String],
    options: &std::collections::BTreeMap<String, String>,
) -> Result<AgentLifecycleSignal, (&'static str, String)> {
    super::signal_arguments::validate_signal_options(
        options,
        &[
            "window-id",
            "worklane-id",
            "pane-id",
            "origin",
            "tool",
            "confidence",
            "session-id",
            "parent-session-id",
        ],
    )?;
    validate_common_values(options)?;
    match positionals {
        [event, pid] if event == "attach" => {
            let pid = pid
                .parse::<i32>()
                .ok()
                .filter(|pid| *pid > 0)
                .ok_or(("invalid_request", "agent PID is invalid".to_owned()))?;
            Ok(AgentLifecycleSignal::AttachPid {
                pid,
                tool: options.get("tool").cloned(),
                session_id: options.get("session-id").cloned(),
                parent_session_id: options.get("parent-session-id").cloned(),
            })
        }
        [event] if event == "clear" => Ok(AgentLifecycleSignal::ClearPid {
            session_id: options.get("session-id").cloned(),
        }),
        [] => Err(("invalid_request", "agent PID event is missing".to_owned())),
        [event, ..] if event == "attach" || event == "clear" => Err((
            "invalid_request",
            "agent PID signal has unexpected positional arguments".to_owned(),
        )),
        [event, ..] => Err((
            "invalid_request",
            format!("unsupported agent PID event {event:?}"),
        )),
    }
}

fn validate_common_values(
    options: &std::collections::BTreeMap<String, String>,
) -> Result<(), (&'static str, String)> {
    validate_value(
        options,
        "origin",
        &[
            "compatibility",
            "explicit-hook",
            "explicit-api",
            "heuristic",
            "shell",
            "inferred",
        ],
    )?;
    validate_value(options, "confidence", &["weak", "strong", "explicit"])?;
    validate_value(
        options,
        "interaction-kind",
        &[
            "none",
            "approval",
            "question",
            "decision",
            "auth",
            "generic-input",
        ],
    )?;
    Ok(())
}

fn validate_value(
    options: &std::collections::BTreeMap<String, String>,
    name: &str,
    allowed: &[&str],
) -> Result<(), (&'static str, String)> {
    if let Some(value) = options.get(name)
        && !allowed.contains(&value.as_str())
    {
        return Err((
            "invalid_request",
            format!("unsupported agent signal --{name} value {value:?}"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{AgentLifecycleSignal, parse_agent_lifecycle_signal};
    use zentty_core::{AgentTarget, AuthenticatedAgentEvent};

    fn event_kind(signal: AgentLifecycleSignal) -> &'static str {
        let AgentLifecycleSignal::Event { event, .. } = signal else {
            panic!("expected lifecycle event")
        };
        AuthenticatedAgentEvent {
            target: AgentTarget::new("window", "lane", "pane"),
            pane_token: "redacted".to_owned(),
            event,
        }
        .event_kind()
    }

    #[test]
    fn lifecycle_aliases_map_to_canonical_events() {
        for (verb, expected) in [
            ("running", "agent.running"),
            ("needs-input", "agent.needs-input"),
            ("idle", "agent.idle"),
            ("completed", "agent.idle"),
            ("clear", "session.end"),
        ] {
            assert_eq!(
                event_kind(
                    parse_agent_lifecycle_signal(&["lifecycle".to_owned(), verb.to_owned()])
                        .unwrap()
                ),
                expected
            );
        }
    }

    #[test]
    fn pid_and_invalid_value_contracts_are_exact() {
        assert_eq!(
            parse_agent_lifecycle_signal(&[
                "pid".to_owned(),
                "attach".to_owned(),
                "42".to_owned(),
                "--session-id".to_owned(),
                "child".to_owned(),
            ]),
            Ok(AgentLifecycleSignal::AttachPid {
                pid: 42,
                tool: None,
                session_id: Some("child".to_owned()),
                parent_session_id: None,
            })
        );
        for arguments in [
            vec!["pid", "attach", "0"],
            vec!["pid", "clear", "extra"],
            vec!["lifecycle", "elsewhere"],
            vec!["lifecycle", "running", "--confidence", "guessed"],
        ] {
            assert!(
                parse_agent_lifecycle_signal(
                    &arguments.into_iter().map(str::to_owned).collect::<Vec<_>>()
                )
                .is_err()
            );
        }
    }
}
