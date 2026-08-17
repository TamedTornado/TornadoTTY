#![forbid(unsafe_code)]

use serde_json::{Value, json};
use std::fs::{OpenOptions, read, remove_file, rename};
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
        if exceeds_limit(bytes.len(), MAX_HEADER_BYTES) {
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
    if exceeds_limit(content_length, MAX_BODY_BYTES) {
        return Err("controlled request body exceeded 16 MiB".to_owned());
    }
    let received_body_bytes = bytes.len() - header_end;
    let missing_body_bytes = content_length.saturating_sub(received_body_bytes);
    let body_end = bytes.len() + missing_body_bytes;
    bytes.resize(body_end, 0);
    stream
        .read_exact(&mut bytes[body_end - missing_body_bytes..body_end])
        .map_err(|error| format!("controlled request body read failed: {error}"))?;
    Ok(HttpRequest {
        method,
        path,
        body: bytes[header_end..header_end + content_length].to_vec(),
    })
}

fn exceeds_limit(actual: usize, maximum: usize) -> bool {
    actual > maximum
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
    let mut line = serde_json::to_vec(&receipt)
        .map_err(|error| format!("could not serialize controlled receipt: {error}"))?;
    line.push(b'\n');
    publish_receipt(path, &line, || Ok(()))
}

fn publish_receipt(
    path: &Path,
    line: &[u8],
    before_rename: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    if line.is_empty() || !line.ends_with(b"\n") {
        return Err("controlled receipt record must end with one newline".to_owned());
    }
    let mut complete = match read(path) {
        Ok(existing) => existing,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(format!("could not read controlled receipt: {error}")),
    };
    if !complete.is_empty() && !complete.ends_with(b"\n") {
        return Err("controlled receipt contains an incomplete committed record".to_owned());
    }
    complete.extend_from_slice(line);

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "controlled receipt requires a UTF-8 file name".to_owned())?;
    let next = path.with_file_name(format!(".{file_name}.next-{}", std::process::id()));
    match remove_file(&next) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "could not clear stale receipt transaction: {error}"
            ));
        }
    }
    let mut transaction = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&next)
        .map_err(|error| format!("could not create controlled receipt transaction: {error}"))?;
    transaction
        .write_all(&complete)
        .map_err(|error| format!("could not write controlled receipt transaction: {error}"))?;
    transaction
        .sync_all()
        .map_err(|error| format!("could not sync controlled receipt transaction: {error}"))?;
    drop(transaction);
    before_rename()?;
    rename(&next, path)
        .map_err(|error| format!("could not publish controlled receipt transaction: {error}"))
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
        MAX_BODY_BYTES, MAX_HEADER_BYTES, RequestRole, append_receipt, classify_request,
        exceeds_limit, message_shape, parse_receipt_path, publish_receipt, read_request,
        summarize_tool_results, value_contains,
    };
    use serde_json::json;
    use std::io::Write;
    use std::net::{Shutdown, TcpListener, TcpStream};

    fn read_raw_request(bytes: Vec<u8>) -> Result<super::HttpRequest, String> {
        read_raw_request_parts(vec![bytes])
    }

    fn read_raw_request_parts(parts: Vec<Vec<u8>>) -> Result<super::HttpRequest, String> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let writer = std::thread::spawn(move || {
            let mut stream = TcpStream::connect(address).unwrap();
            for part in parts {
                stream.write_all(&part).unwrap();
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            stream.shutdown(Shutdown::Write).unwrap();
        });
        let (mut stream, peer) = listener.accept().unwrap();
        assert!(peer.ip().is_loopback());
        stream
            .set_read_timeout(Some(std::time::Duration::from_millis(100)))
            .unwrap();
        let result = read_request(&mut stream);
        writer.join().unwrap();
        result
    }

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

    #[test]
    fn http_parser_rejects_wrong_targets_lengths_versions_and_truncation() {
        assert!(!exceeds_limit(MAX_HEADER_BYTES, MAX_HEADER_BYTES));
        assert!(exceeds_limit(MAX_HEADER_BYTES + 1, MAX_HEADER_BYTES));
        assert!(!exceeds_limit(MAX_BODY_BYTES, MAX_BODY_BYTES));
        assert!(exceeds_limit(MAX_BODY_BYTES + 1, MAX_BODY_BYTES));
        for request in [
            b"GET / HTTP/1.1\r\n\r\n".to_vec(),
            b"POST /wrong HTTP/1.1\r\nContent-Length: 0\r\n\r\n".to_vec(),
            b"POST /v1/messages HTTP/1.0\r\nContent-Length: 0\r\n\r\n".to_vec(),
            b"POST /v1/messages HTTP/1.1\r\n\r\n".to_vec(),
            b"POST /v1/messages HTTP/1.1\r\nContent-Length: invalid\r\n\r\n".to_vec(),
            b"POST /v1/messages HTTP/1.1\r\nContent-Length: 4\r\n\r\n{}".to_vec(),
        ] {
            assert!(read_raw_request(request).is_err());
        }

        let oversized_body = format!(
            "POST /v1/messages HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
            MAX_BODY_BYTES + 1
        );
        assert!(read_raw_request(oversized_body.into_bytes()).is_err());

        let mut oversized_headers = b"POST /v1/messages HTTP/1.1\r\nX-Fill: ".to_vec();
        oversized_headers.extend(std::iter::repeat_n(b'x', MAX_HEADER_BYTES));
        assert!(read_raw_request(oversized_headers).is_err());

        let body = b"{}".to_vec();
        let header = format!(
            "POST /v1/messages HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
            body.len()
        )
        .into_bytes();
        assert_eq!(
            read_raw_request_parts(vec![header, body.clone()])
                .unwrap()
                .body,
            body
        );
    }

    #[test]
    fn authorization_headers_and_prompt_bodies_never_enter_the_receipt() {
        let secret = "controlled-header-and-prompt-secret";
        let body = serde_json::to_vec(&json!({
            "system": "ordinary leader",
            "messages": [{"role": "user", "content": secret}],
            "tools": [{"name": "Agent"}, {"name": "SendMessage"}]
        }))
        .unwrap();
        let raw = format!(
            "POST /v1/messages HTTP/1.1\r\nAuthorization: Bearer {secret}\r\nContent-Length: {}\r\n\r\n",
            body.len()
        )
        .into_bytes()
        .into_iter()
        .chain(body)
        .collect();
        let request = read_raw_request(raw).unwrap();
        let receipt = std::env::temp_dir().join(format!(
            "zentty-controlled-anthropic-receipt-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        append_receipt(&receipt, 1, RequestRole::LeaderInitial, &request).unwrap();
        let retained = std::fs::read_to_string(&receipt).unwrap();
        std::fs::remove_file(receipt).unwrap();
        assert!(!retained.contains(secret));
        assert!(retained.contains("\"role\":\"leader_initial\""));
    }

    #[test]
    fn interrupted_receipt_publication_preserves_complete_committed_records() {
        let receipt = std::env::temp_dir().join(format!(
            "zentty-controlled-anthropic-transaction-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::write(&receipt, b"{\"sequence\":1}\n").unwrap();

        let failure = publish_receipt(&receipt, b"{\"sequence\":2}\n", || {
            Err("controlled interruption before rename".to_owned())
        })
        .unwrap_err();
        assert_eq!(failure, "controlled interruption before rename");
        assert_eq!(std::fs::read(&receipt).unwrap(), b"{\"sequence\":1}\n");

        publish_receipt(&receipt, b"{\"sequence\":2}\n", || Ok(())).unwrap();
        let committed = std::fs::read_to_string(&receipt).unwrap();
        assert_eq!(committed, "{\"sequence\":1}\n{\"sequence\":2}\n");
        for line in committed.lines() {
            serde_json::from_str::<serde_json::Value>(line).unwrap();
        }
        std::fs::remove_file(receipt).unwrap();
    }
}
