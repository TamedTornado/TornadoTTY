#![forbid(unsafe_code)]

use serde_json::{Value, json};
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const MAX_HEADER_BYTES: usize = 64 * 1024;
const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;
const MAX_REQUESTS: usize = 32;
const LIFETIME: Duration = Duration::from_secs(45);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RequestRole {
    Health,
    Title,
    LeaderInitial,
    LeaderFollowup,
    Teammate,
}

fn main() -> Result<(), String> {
    let receipt = parse_receipt_path(std::env::args_os().skip(1))?;
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .map_err(|error| format!("could not bind controlled Anthropic endpoint: {error}"))?;
    listener
        .set_nonblocking(true)
        .map_err(|error| format!("could not bound endpoint polling: {error}"))?;
    let address = listener
        .local_addr()
        .map_err(|error| format!("could not identify controlled endpoint: {error}"))?;
    println!("CONTROLLED_ANTHROPIC_READY=http://{address}");
    std::io::stdout()
        .flush()
        .map_err(|error| format!("could not publish endpoint readiness: {error}"))?;

    let deadline = Instant::now() + LIFETIME;
    let mut request_number = 0;
    let mut saw_leader_initial = false;
    while request_number < MAX_REQUESTS && Instant::now() < deadline {
        let (mut stream, peer) = match listener.accept() {
            Ok(connection) => connection,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(10));
                continue;
            }
            Err(error) => return Err(format!("controlled endpoint accept failed: {error}")),
        };
        if !peer.ip().is_loopback() {
            return Err("controlled endpoint rejected a non-loopback peer".to_owned());
        }
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .map_err(|error| format!("could not bound request reads: {error}"))?;
        stream
            .set_write_timeout(Some(Duration::from_secs(2)))
            .map_err(|error| format!("could not bound response writes: {error}"))?;
        let request = read_request(&mut stream)?;
        let role = classify_request(&request)?;
        if role == RequestRole::Health {
            write_empty_response(&mut stream)?;
            continue;
        }
        request_number += 1;
        saw_leader_initial |= role == RequestRole::LeaderInitial;
        append_receipt(&receipt, request_number, role, &request)?;
        write_model_response(&mut stream, request_number, role)?;
    }
    if !saw_leader_initial {
        return Err("controlled endpoint never received a leader request".to_owned());
    }
    Ok(())
}

fn parse_receipt_path(
    mut arguments: impl Iterator<Item = std::ffi::OsString>,
) -> Result<PathBuf, String> {
    if arguments.next().as_deref() != Some(std::ffi::OsStr::new("--receipt")) {
        return Err("usage: controlled_anthropic --receipt PATH".to_owned());
    }
    let receipt = arguments
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| "--receipt requires a path".to_owned())?;
    if arguments.next().is_some() {
        return Err("unexpected controlled endpoint argument".to_owned());
    }
    if !receipt.is_absolute() {
        return Err("controlled endpoint receipt path must be absolute".to_owned());
    }
    Ok(receipt)
}

struct HttpRequest {
    method: String,
    path: String,
    body: Vec<u8>,
}

fn read_request(stream: &mut TcpStream) -> Result<HttpRequest, String> {
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 8192];
    let header_end = loop {
        let count = stream
            .read(&mut chunk)
            .map_err(|error| format!("controlled request header read failed: {error}"))?;
        if count == 0 {
            return Err("controlled request ended before its headers".to_owned());
        }
        bytes.extend_from_slice(&chunk[..count]);
        if bytes.len() > MAX_HEADER_BYTES {
            return Err("controlled request headers exceeded 64 KiB".to_owned());
        }
        if let Some(offset) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break offset + 4;
        }
    };
    let headers = std::str::from_utf8(&bytes[..header_end])
        .map_err(|_| "controlled request headers were not UTF-8".to_owned())?;
    let mut lines = headers.lines();
    let request_line = lines
        .next()
        .ok_or_else(|| "controlled request line is missing".to_owned())?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts.next().unwrap_or_default().to_owned();
    let path = request_parts.next().unwrap_or_default().to_owned();
    if request_parts.next() != Some("HTTP/1.1") || request_parts.next().is_some() {
        return Err("controlled endpoint requires HTTP/1.1".to_owned());
    }
    if method == "HEAD" {
        return Ok(HttpRequest {
            method,
            path,
            body: Vec::new(),
        });
    }
    if method != "POST" || path != "/v1/messages?beta=true" && path != "/v1/messages" {
        return Err(format!(
            "controlled endpoint rejected request target: {method} {path}"
        ));
    }
    let content_length = lines
        .filter_map(|line| line.split_once(':'))
        .find_map(|(name, value)| {
            name.eq_ignore_ascii_case("content-length")
                .then_some(value.trim())
        })
        .ok_or_else(|| "controlled request is missing Content-Length".to_owned())?
        .parse::<usize>()
        .map_err(|_| "controlled request Content-Length is invalid".to_owned())?;
    if content_length > MAX_BODY_BYTES {
        return Err("controlled request body exceeded 16 MiB".to_owned());
    }
    while bytes.len() - header_end < content_length {
        let count = stream
            .read(&mut chunk)
            .map_err(|error| format!("controlled request body read failed: {error}"))?;
        if count == 0 {
            return Err("controlled request ended before its body".to_owned());
        }
        bytes.extend_from_slice(&chunk[..count]);
    }
    Ok(HttpRequest {
        method,
        path,
        body: bytes[header_end..header_end + content_length].to_vec(),
    })
}

fn classify_request(request: &HttpRequest) -> Result<RequestRole, String> {
    if request.method == "HEAD" {
        if request.path != "/" {
            return Err("controlled health check used an unexpected path".to_owned());
        }
        return Ok(RequestRole::Health);
    }
    let body: Value = serde_json::from_slice(&request.body)
        .map_err(|error| format!("controlled request JSON is invalid: {error}"))?;
    let tools = body["tools"]
        .as_array()
        .ok_or_else(|| "controlled request has no tool inventory".to_owned())?;
    let tool_names = tools
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect::<Vec<_>>();
    if tool_names.is_empty() && value_contains(&body["system"], "title") {
        return Ok(RequestRole::Title);
    }
    for required in ["Agent", "SendMessage"] {
        if !tool_names.contains(&required) {
            return Err(format!(
                "installed Claude request is missing required tool: {required}; advertised tools: {}; model={}; messages={}; system_mentions_title={}; contains_probe_prompt={}",
                tool_names.join(","),
                body["model"].as_str().unwrap_or("<non-string>"),
                body["messages"].as_array().map_or(0, Vec::len),
                value_contains(&body["system"], "title"),
                value_contains(&body["messages"], "create one named teammate")
            ));
        }
    }
    if value_contains(&body["system"], "running as an agent in a team") {
        Ok(RequestRole::Teammate)
    } else if value_contains(&body["messages"], "tool_use")
        || value_contains(&body["messages"], "tool_result")
    {
        Ok(RequestRole::LeaderFollowup)
    } else {
        Ok(RequestRole::LeaderInitial)
    }
}

fn value_contains(value: &Value, needle: &str) -> bool {
    match value {
        Value::String(value) => value.to_ascii_lowercase().contains(needle),
        Value::Array(values) => values.iter().any(|value| value_contains(value, needle)),
        Value::Object(values) => values.values().any(|value| value_contains(value, needle)),
        _ => false,
    }
}

#[derive(Default)]
struct ToolResultSummary {
    count: usize,
    errors: usize,
}

fn summarize_tool_results(value: &Value, summary: &mut ToolResultSummary) {
    match value {
        Value::Array(values) => {
            for value in values {
                summarize_tool_results(value, summary);
            }
        }
        Value::Object(values) => {
            if values.get("type").and_then(Value::as_str) == Some("tool_result") {
                summary.count += 1;
                if values.get("is_error").and_then(Value::as_bool) == Some(true) {
                    summary.errors += 1;
                }
            }
            for value in values.values() {
                summarize_tool_results(value, summary);
            }
        }
        _ => {}
    }
}

fn message_shape(messages: &Value) -> Vec<Value> {
    messages
        .as_array()
        .into_iter()
        .flatten()
        .map(|message| {
            let content = &message["content"];
            json!({
                "role": message["role"].as_str().unwrap_or("<missing>"),
                "content": match content {
                    Value::String(_) => vec!["string"],
                    Value::Array(blocks) => blocks
                        .iter()
                        .map(|block| block["type"].as_str().unwrap_or("<missing>"))
                        .collect::<Vec<_>>(),
                    _ => vec!["<other>"],
                },
            })
        })
        .collect()
}

fn append_receipt(
    path: &Path,
    sequence: usize,
    role: RequestRole,
    request: &HttpRequest,
) -> Result<(), String> {
    let body: Value = serde_json::from_slice(&request.body)
        .map_err(|error| format!("could not sanitize controlled request: {error}"))?;
    let tool_names = body["tools"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|tool| tool["name"].as_str())
        .collect::<Vec<_>>();
    let mut tool_results = ToolResultSummary::default();
    summarize_tool_results(&body["messages"], &mut tool_results);
    let receipt = json!({
        "sequence": sequence,
        "role": match role {
            RequestRole::LeaderInitial => "leader_initial",
            RequestRole::LeaderFollowup => "leader_followup",
            RequestRole::Teammate => "teammate",
            RequestRole::Title => "title",
            RequestRole::Health => "health",
        },
        "path": request.path,
        "agentTool": tool_names.contains(&"Agent"),
        "sendMessageTool": tool_names.contains(&"SendMessage"),
        "toolResultCount": tool_results.count,
        "toolResultErrors": tool_results.errors,
        "resultMentionsAgentId": value_contains(&body["messages"], "agentid"),
        "resultMentionsTeam": value_contains(&body["messages"], "team"),
        "resultMentionsFailure": value_contains(&body["messages"], "fail"),
        "resultMentionsBackground": value_contains(&body["messages"], "background"),
        "messageShape": message_shape(&body["messages"]),
    });
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| format!("could not open controlled receipt: {error}"))?;
    writeln!(file, "{receipt}")
        .map_err(|error| format!("could not write controlled receipt: {error}"))
}

fn write_empty_response(stream: &mut TcpStream) -> Result<(), String> {
    stream
        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
        .map_err(|error| format!("could not write controlled health response: {error}"))
}

fn write_model_response(
    stream: &mut TcpStream,
    sequence: usize,
    role: RequestRole,
) -> Result<(), String> {
    let content = match role {
        RequestRole::LeaderInitial => json!({
            "type": "tool_use",
            "id": format!("toolu_zentty_{sequence}"),
            "name": "Agent",
            "input": {
                "description": "Run Zentty probe",
                "prompt": "Remain idle without reading or changing files.",
                "subagent_type": "general-purpose",
                "name": "zentty-probe",
                "mode": "dontAsk"
            }
        }),
        RequestRole::LeaderFollowup | RequestRole::Teammate => json!({
            "type": "text",
            "text": if role == RequestRole::Teammate {
                "Controlled teammate is ready."
            } else {
                "Controlled team launch complete."
            }
        }),
        RequestRole::Title => json!({
            "type": "text",
            "text": "Zentty agent team probe"
        }),
        RequestRole::Health => return write_empty_response(stream),
    };
    let stop_reason = if role == RequestRole::LeaderInitial {
        "tool_use"
    } else {
        "end_turn"
    };
    let response = json!({
        "id": format!("msg_zentty_{sequence}"),
        "type": "message",
        "role": "assistant",
        "model": "claude-controlled-zentty",
        "content": [content],
        "stop_reason": stop_reason,
        "stop_sequence": null,
        "usage": {"input_tokens": 1, "output_tokens": 1}
    });
    let body = response.to_string();
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    )
    .map_err(|error| format!("could not write controlled model response: {error}"))
}

#[cfg(test)]
mod tests {
    use super::{
        RequestRole, classify_request, message_shape, parse_receipt_path, summarize_tool_results,
        value_contains,
    };
    use serde_json::json;

    #[test]
    fn receipt_path_is_absolute_and_argument_shape_is_exact() {
        assert!(
            parse_receipt_path(["--receipt".into(), "/tmp/receipt".into()].into_iter()).is_ok()
        );
        assert!(parse_receipt_path(std::iter::empty()).is_err());
        assert!(parse_receipt_path(["--receipt".into(), "relative".into()].into_iter()).is_err());
    }

    #[test]
    fn recursive_classification_distinguishes_leader_and_teammate_without_secrets() {
        assert!(value_contains(
            &json!([{"text": "You are running as an agent in a team."}]),
            "running as an agent in a team"
        ));
        let request = super::HttpRequest {
            method: "POST".to_owned(),
            path: "/v1/messages".to_owned(),
            body: serde_json::to_vec(&json!({
                "system": "ordinary leader",
                "tools": [{"name": "Agent"}, {"name": "SendMessage"}]
            }))
            .unwrap(),
        };
        assert_eq!(
            classify_request(&request).unwrap(),
            RequestRole::LeaderInitial
        );

        let followup = super::HttpRequest {
            method: "POST".to_owned(),
            path: "/v1/messages?beta=true".to_owned(),
            body: serde_json::to_vec(&json!({
                "system": "ordinary leader",
                "tools": [{"name": "Agent"}, {"name": "SendMessage"}],
                "messages": [
                    {"role": "assistant", "content": [{"type": "text", "text": "ready"}]},
                    {"role": "user", "content": [
                        {"type": "tool_result", "is_error": false},
                        {"type": "tool_result", "is_error": false},
                        {"type": "tool_result", "is_error": true}
                    ]}
                ]
            }))
            .unwrap(),
        };
        assert_eq!(
            classify_request(&followup).unwrap(),
            RequestRole::LeaderFollowup
        );
        let mut summary = super::ToolResultSummary::default();
        let body: serde_json::Value = serde_json::from_slice(&followup.body).unwrap();
        summarize_tool_results(&body["messages"], &mut summary);
        assert_eq!((summary.count, summary.errors), (3, 1));
        assert_eq!(message_shape(&body["messages"]).len(), 2);
    }

    #[test]
    fn title_and_schema_drift_are_explicit() {
        let health = super::HttpRequest {
            method: "HEAD".to_owned(),
            path: "/".to_owned(),
            body: Vec::new(),
        };
        assert_eq!(classify_request(&health).unwrap(), RequestRole::Health);
        let wrong_health = super::HttpRequest {
            path: "/unexpected".to_owned(),
            ..health
        };
        assert!(classify_request(&wrong_health).is_err());

        let title = super::HttpRequest {
            method: "POST".to_owned(),
            path: "/v1/messages".to_owned(),
            body: serde_json::to_vec(&json!({
                "model": "claude-opus-4-8",
                "system": "Generate a concise title",
                "messages": [{"role": "user", "content": "Create one named teammate"}],
                "tools": []
            }))
            .unwrap(),
        };
        assert_eq!(classify_request(&title).unwrap(), RequestRole::Title);

        let title_words_in_a_normal_request = super::HttpRequest {
            method: "POST".to_owned(),
            path: "/v1/messages".to_owned(),
            body: serde_json::to_vec(&json!({
                "system": "ordinary leader that can discuss a title",
                "messages": [],
                "tools": [{"name": "Agent"}, {"name": "SendMessage"}]
            }))
            .unwrap(),
        };
        assert_eq!(
            classify_request(&title_words_in_a_normal_request).unwrap(),
            RequestRole::LeaderInitial
        );

        let missing = super::HttpRequest {
            method: "POST".to_owned(),
            path: "/v1/messages".to_owned(),
            body: serde_json::to_vec(&json!({
                "system": "ordinary leader",
                "messages": [],
                "tools": [{"name": "SendMessage"}]
            }))
            .unwrap(),
        };
        assert!(
            classify_request(&missing)
                .unwrap_err()
                .contains("missing required tool: Agent")
        );
    }
}
