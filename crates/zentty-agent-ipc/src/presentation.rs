use crate::{ApplicationOperation, ApplicationRequest, ApplicationResult, ApplicationResultKind};
use std::fmt::Write as _;

/// Renders one structured application result using the source-compatible CLI
/// presentation requested by the parsed command.
///
/// # Errors
///
/// Returns an error when the result kind or payload does not match the request.
pub fn render_application_result(
    request: &ApplicationRequest,
    result: &ApplicationResult,
) -> Result<String, String> {
    match result.kind() {
        ApplicationResultKind::Empty => Ok(String::new()),
        ApplicationResultKind::Theme => result
            .value()
            .as_str()
            .map(|mode| format!("{mode}\n"))
            .ok_or_else(|| "theme result is not a string".to_owned()),
        ApplicationResultKind::Topology => {
            if has_flag(request, "--json") {
                Ok(pretty_json(result.value()))
            } else {
                render_topology_text(result.value())
            }
        }
        ApplicationResultKind::Selection => render_selection(request, result.value()),
        ApplicationResultKind::Discovery => render_discovery(request, result.value()),
    }
}

fn render_discovery(
    request: &ApplicationRequest,
    value: &serde_json::Value,
) -> Result<String, String> {
    if has_flag(request, "--json") {
        return Ok(pretty_json(value));
    }
    match request.operation() {
        ApplicationOperation::Overview => render_overview(value),
        ApplicationOperation::Windows => render_rows("windows", array(value)?),
        ApplicationOperation::Worklanes => render_rows("worklanes", array(value)?),
        ApplicationOperation::Panes | ApplicationOperation::PanesCurrentWorklane => {
            render_rows("panes", array(value)?)
        }
        _ => Err("discovery result does not match its operation".to_owned()),
    }
}

fn render_topology_text(value: &serde_json::Value) -> Result<String, String> {
    let action = string(value, "action")?;
    let created = array_field(value, "createdPaneIDs")?.len();
    let affected = array_field(value, "affectedPaneIDs")?.len();
    Ok(format!(
        "{action}: window={} worklane={} source={} focused={} created={created} affected={affected}\n",
        string(value, "windowID")?,
        string(value, "worklaneID")?,
        string(value, "sourcePaneID")?,
        string(value, "focusedPaneID")?,
    ))
}

fn render_selection(
    request: &ApplicationRequest,
    value: &serde_json::Value,
) -> Result<String, String> {
    let pane = value
        .get("pane")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| "selection result omitted its pane".to_owned())?;
    let field = |name: &str| {
        pane.get(name)
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| format!("selection result omitted {name}"))
    };
    if has_flag(request, "--shell") {
        let socket = value
            .get("socketPath")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "selection result omitted socketPath".to_owned())?;
        let mut output = format!(
            "export ZENTTY_INSTANCE_SOCKET='{}'\nexport ZENTTY_WINDOW_ID='{}'\nexport ZENTTY_WORKLANE_ID='{}'\nexport ZENTTY_PANE_ID='{}'\n",
            shell_escape(socket),
            shell_escape(field("windowID")?),
            shell_escape(field("worklaneID")?),
            shell_escape(field("id")?),
        );
        if let Some(token) = pane.get("controlToken").and_then(serde_json::Value::as_str) {
            writeln!(output, "export ZENTTY_PANE_TOKEN='{}'", shell_escape(token))
                .expect("writing to a string cannot fail");
        }
        Ok(output)
    } else {
        Ok(format!(
            "window {}\nworklane {}\npane {}\n",
            field("windowID")?,
            field("worklaneID")?,
            field("id")?,
        ))
    }
}

fn render_rows(kind: &str, rows: &[serde_json::Value]) -> Result<String, String> {
    if rows.is_empty() {
        return Ok(format!("No {kind}.\n"));
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
            "windows" => Ok(format!(
                "{}  {}  {}  {}  {}",
                pad(&integer(row, "order"), 5),
                focus_marker(row),
                pad(string(row, "id")?, 36),
                pad(&integer(row, "worklaneCount"), 9),
                integer(row, "paneCount")
            )),
            "worklanes" => Ok(format!(
                "{}  {}  {}  {}  {}  {}  {}",
                pad(string(row, "windowID")?, 36),
                pad(&integer(row, "order"), 5),
                focus_marker(row),
                pad(string(row, "id")?, 20),
                pad(
                    &truncate_tail(optional_string(row, "title").unwrap_or("-"), 16),
                    16
                ),
                pad(&integer(row, "columnCount"), 4),
                integer(row, "paneCount")
            )),
            _ => {
                let cwd = optional_string(row, "workingDirectory")
                    .map_or_else(|| "-".to_owned(), abbreviate_home);
                Ok(format!(
                    "{}  {}  {}  {}  {}  {}  {}  {}  {}",
                    pad(&truncate_tail(string(row, "windowID")?, 12), 12),
                    pad(&truncate_tail(string(row, "worklaneID")?, 20), 20),
                    pad(&integer(row, "index"), 3),
                    pad(&integer(row, "column"), 3),
                    focus_marker(row),
                    pad(&truncate_tail(string(row, "title")?, 16), 16),
                    pad(&truncate_tail(&cwd, 30), 30),
                    pad(
                        &truncate_tail(optional_string(row, "agentTool").unwrap_or("-"), 12),
                        12
                    ),
                    optional_string(row, "agentStatus").unwrap_or("-")
                ))
            }
        })
        .collect::<Result<Vec<_>, String>>()?
        .join("\n");
    Ok(format!("{header}\n{body}\n"))
}

fn render_overview(value: &serde_json::Value) -> Result<String, String> {
    let windows = value
        .get("windows")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "overview result omitted windows".to_owned())?;
    if windows.is_empty() {
        return Ok("No windows.\n".to_owned());
    }
    let worklane_count = windows.iter().try_fold(0_usize, |total, window| {
        Ok::<_, String>(total + array_field(window, "worklanes")?.len())
    })?;
    let pane_count = windows.iter().try_fold(0_usize, |total, window| {
        array_field(window, "worklanes")?
            .iter()
            .try_fold(total, |subtotal, lane| {
                Ok::<_, String>(subtotal + array_field(lane, "panes")?.len())
            })
    })?;
    let mut output = format!(
        "WINDOWS {}  WORKLANES {worklane_count}  PANES {pane_count}\n\n",
        windows.len()
    );
    for (window_index, window) in windows.iter().enumerate() {
        let worklanes = array_field(window, "worklanes")?;
        let window_panes = worklanes.iter().try_fold(0_usize, |total, lane| {
            Ok::<_, String>(total + array_field(lane, "panes")?.len())
        })?;
        writeln!(
            output,
            "window {}  {}  {}  worklanes:{}  panes:{}",
            focus_marker(window),
            integer(window, "order"),
            string(window, "id")?,
            worklanes.len(),
            window_panes
        )
        .expect("writing to a string cannot fail");
        for worklane in worklanes {
            render_overview_worklane(&mut output, worklane)?;
        }
        if window_index + 1 < windows.len() {
            output.push('\n');
        }
    }
    Ok(output)
}

fn render_overview_worklane(
    output: &mut String,
    worklane: &serde_json::Value,
) -> Result<(), String> {
    let panes = array_field(worklane, "panes")?;
    let title = optional_string(worklane, "title")
        .map(|value| format!("{}  ", pad(&truncate_tail(value.trim(), 28), 28)))
        .unwrap_or_default();
    writeln!(
        output,
        "  worklane {}  {}  {}{}  panes:{}",
        focus_marker(worklane),
        integer(worklane, "order"),
        title,
        string(worklane, "id")?,
        panes.len()
    )
    .expect("writing to a string cannot fail");
    for pane in panes {
        let cwd = optional_string(pane, "workingDirectory")
            .map(abbreviate_home)
            .map_or_else(|| "-".to_owned(), |value| truncate_leading(&value, 42));
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
            string(pane, "id")?,
            pad(&truncate_tail(non_empty(string(pane, "title")?), 42), 42),
            cwd,
            agent
        )
        .expect("writing to a string cannot fail");
    }
    Ok(())
}

fn has_flag(request: &ApplicationRequest, flag: &str) -> bool {
    request.arguments().iter().any(|argument| argument == flag)
}

fn array(value: &serde_json::Value) -> Result<&[serde_json::Value], String> {
    value
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| "application result is not an array".to_owned())
}

fn array_field<'a>(
    value: &'a serde_json::Value,
    key: &str,
) -> Result<&'a [serde_json::Value], String> {
    value
        .get(key)
        .and_then(serde_json::Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("application result omitted {key}"))
}

fn string<'a>(value: &'a serde_json::Value, key: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("application result omitted {key}"))
}

fn optional_string<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
}

fn integer(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(serde_json::Value::as_u64)
        .map_or_else(|| "-".to_owned(), |value| value.to_string())
}

fn focus_marker(value: &serde_json::Value) -> &'static str {
    if value.get("isFocused") == Some(&serde_json::Value::Bool(true)) {
        "*"
    } else {
        " "
    }
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
        |home| {
            let home = home.to_string_lossy();
            value
                .strip_prefix(home.as_ref())
                .map_or_else(|| value.to_owned(), |suffix| format!("~{suffix}"))
        },
    )
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

fn pretty_json(value: &serde_json::Value) -> String {
    serde_json::to_string_pretty(value).expect("application values always serialize") + "\n"
}

fn shell_escape(value: &str) -> String {
    value.replace('\'', "'\"'\"'")
}

#[cfg(test)]
mod tests {
    use super::render_application_result;
    use crate::{ApplicationRequest, ApplicationResult, ApplicationResultKind, ApplicationScope};
    use serde_json::json;

    fn request(scope: ApplicationScope, operation: &str, arguments: &[&str]) -> ApplicationRequest {
        ApplicationRequest::new(
            scope,
            operation,
            arguments.iter().map(|value| (*value).to_owned()).collect(),
        )
        .unwrap()
    }

    #[test]
    fn source_compatible_tables_and_overview_match_reviewed_goldens() {
        let window = json!({
            "id": "<WINDOW_ID>", "order": 1, "isFocused": true,
            "worklaneCount": 1, "paneCount": 1
        });
        let worklane = json!({
            "id": "<WORKLANE_ID>", "windowID": "<WINDOW_ID>", "order": 1,
            "title": "<TITLE>", "isFocused": true, "paneCount": 1, "columnCount": 1,
            "focusedPaneID": "<PANE_ID>"
        });
        let pane = json!({
            "id": "<PANE_ID>", "windowID": "<WINDOW_ID>",
            "worklaneID": "<WORKLANE_ID>", "index": 1, "column": 1,
            "title": "<TITLE>", "workingDirectory": "<CWD>", "isFocused": true
        });
        for (operation, value, golden) in [
            (
                "windows",
                json!([window.clone()]),
                include_str!("../../../docs/design/cli-goldens/windows-v1.txt"),
            ),
            (
                "worklanes",
                json!([worklane.clone()]),
                include_str!("../../../docs/design/cli-goldens/worklanes-v1.txt"),
            ),
            (
                "panes",
                json!([pane.clone()]),
                include_str!("../../../docs/design/cli-goldens/panes-v1.txt"),
            ),
        ] {
            let result = ApplicationResult::new(ApplicationResultKind::Discovery, value);
            assert_eq!(
                render_application_result(
                    &request(ApplicationScope::Discover, operation, &[]),
                    &result,
                )
                .unwrap(),
                golden,
            );
        }
        let overview = ApplicationResult::new(
            ApplicationResultKind::Discovery,
            json!({"windows":[{
                "id":"<WINDOW_ID>", "order":1, "isFocused":true,
                "worklanes":[{
                    "id":"<WORKLANE_ID>", "order":1, "isFocused":true,
                    "columnCount":1, "focusedPaneID":"<PANE_ID>",
                    "panes":[pane]
                }]
            }]}),
        );
        assert_eq!(
            render_application_result(
                &request(ApplicationScope::Discover, "overview", &[]),
                &overview,
            )
            .unwrap(),
            include_str!("../../../docs/design/cli-goldens/topology-overview-v1.txt"),
        );
    }

    #[test]
    fn selection_shell_output_quotes_hostile_values_without_interpolation() {
        let result = ApplicationResult::new(
            ApplicationResultKind::Selection,
            json!({
                "socketPath":"/tmp/socket-'$(literal)\nnext",
                "pane":{
                    "id":"pane-'$HOME\n$(touch nope)",
                    "windowID":"window-'$(literal)\n雪",
                    "worklaneID":"lane-'two",
                    "controlToken":"token-'quoted"
                }
            }),
        );
        let rendered = render_application_result(
            &request(
                ApplicationScope::Discover,
                "select-pane",
                &["--shell", "--include-control-token"],
            ),
            &result,
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
    }

    #[test]
    fn topology_and_theme_presentation_are_cli_only_and_bounded_by_kind() {
        let topology = ApplicationResult::new(
            ApplicationResultKind::Topology,
            json!({
                "version":1, "action":"split", "windowID":"window-1",
                "worklaneID":"lane-1", "sourcePaneID":"pane-1",
                "focusedPaneID":"pane-2", "createdPaneIDs":["pane-2"],
                "affectedPaneIDs":["pane-1","pane-2"], "topology":{"columns":[]}
            }),
        );
        assert_eq!(
            render_application_result(
                &request(ApplicationScope::Pane, "split", &["right"]),
                &topology,
            )
            .unwrap(),
            "split: window=window-1 worklane=lane-1 source=pane-1 focused=pane-2 created=1 affected=2\n"
        );
        assert!(
            render_application_result(
                &request(ApplicationScope::Pane, "split", &["right", "--json"]),
                &topology,
            )
            .unwrap()
            .starts_with("{\n")
        );
        let theme = ApplicationResult::new(
            ApplicationResultKind::Theme,
            serde_json::Value::String("dark".to_owned()),
        );
        assert_eq!(
            render_application_result(
                &request(ApplicationScope::Pane, "theme", &["dark"]),
                &theme,
            )
            .unwrap(),
            "dark\n"
        );
    }
}
