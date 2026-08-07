#![forbid(unsafe_code)]

use serde_json::{Value, json};
use std::fmt::Write as _;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

const MAX_HEADER_BYTES: usize = 65_536;
const MAX_BODY_BYTES: usize = 16_777_216;

fn main() -> Result<(), String> {
    let receipt = parse_receipt_path(std::env::args_os().skip(1))?;
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .map_err(|error| format!("could not bind controlled Responses endpoint: {error}"))?;
    let address = listener
        .local_addr()
        .map_err(|error| format!("could not identify controlled Responses endpoint: {error}"))?;
    println!("CONTROLLED_OPENAI_RESPONSES_READY=http://{address}");
    std::io::stdout()
        .flush()
        .map_err(|error| format!("could not publish endpoint readiness: {error}"))?;

    let (mut stream, peer) = listener
        .accept()
        .map_err(|error| format!("controlled Responses accept failed: {error}"))?;
    if !peer.ip().is_loopback() {
        return Err("controlled Responses endpoint rejected a non-loopback peer".to_owned());
    }
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|error| format!("could not bound controlled request reads: {error}"))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|error| format!("could not bound controlled response writes: {error}"))?;
    let request = read_request(&mut stream)?;
    let body = validate_request(&request)?;
    append_receipt(&receipt, &body)?;
    write_completed_turn(&mut stream)
}

fn parse_receipt_path(
    mut arguments: impl Iterator<Item = std::ffi::OsString>,
) -> Result<PathBuf, String> {
    if arguments.next().as_deref() != Some(std::ffi::OsStr::new("--receipt")) {
        return Err("usage: controlled_openai_responses --receipt PATH".to_owned());
    }
    let receipt = arguments
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| "--receipt requires a path".to_owned())?;
    if arguments.next().is_some() {
        return Err("unexpected controlled Responses endpoint argument".to_owned());
    }
    if !receipt.is_absolute() {
        return Err("controlled Responses receipt path must be absolute".to_owned());
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
            .map_err(|error| format!("controlled Responses header read failed: {error}"))?;
        if count == 0 {
            return Err("controlled Responses request ended before its headers".to_owned());
        }
        bytes.extend_from_slice(&chunk[..count]);
        if exceeds_limit(bytes.len(), MAX_HEADER_BYTES) {
            return Err("controlled Responses headers exceeded 64 KiB".to_owned());
        }
        if let Some(offset) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break offset + 4;
        }
    };
    let headers = std::str::from_utf8(&bytes[..header_end])
        .map_err(|_| "controlled Responses headers were not UTF-8".to_owned())?;
    let mut lines = headers.lines();
    let request_line = lines
        .next()
        .ok_or_else(|| "controlled Responses request line is missing".to_owned())?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_owned();
    let path = parts.next().unwrap_or_default().to_owned();
    if parts.next() != Some("HTTP/1.1") || parts.next().is_some() {
        return Err("controlled Responses endpoint requires HTTP/1.1".to_owned());
    }
    if method != "POST" || path != "/v1/responses" {
        return Err(format!(
            "controlled Responses endpoint rejected request target: {method} {path}"
        ));
    }
    let content_length = lines
        .filter_map(|line| line.split_once(':'))
        .find_map(|(name, value)| {
            name.eq_ignore_ascii_case("content-length")
                .then_some(value.trim())
        })
        .ok_or_else(|| "controlled Responses request is missing Content-Length".to_owned())?
        .parse::<usize>()
        .map_err(|_| "controlled Responses Content-Length is invalid".to_owned())?;
    if exceeds_limit(content_length, MAX_BODY_BYTES) {
        return Err("controlled Responses body exceeded 16 MiB".to_owned());
    }
    let received = bytes.len() - header_end;
    let missing = content_length.saturating_sub(received);
    let body_end = bytes.len() + missing;
    bytes.resize(body_end, 0);
    stream
        .read_exact(&mut bytes[body_end - missing..body_end])
        .map_err(|error| format!("controlled Responses body read failed: {error}"))?;
    Ok(HttpRequest {
        method,
        path,
        body: bytes[header_end..header_end + content_length].to_vec(),
    })
}

fn exceeds_limit(actual: usize, maximum: usize) -> bool {
    actual > maximum
}

fn validate_request(request: &HttpRequest) -> Result<Value, String> {
    if request.method != "POST" || request.path != "/v1/responses" {
        return Err("controlled Responses request target changed after parsing".to_owned());
    }
    let body: Value = serde_json::from_slice(&request.body)
        .map_err(|error| format!("controlled Responses JSON is invalid: {error}"))?;
    if body["stream"].as_bool() != Some(true) {
        return Err("installed Codex did not request a streaming response".to_owned());
    }
    if body["model"].as_str() != Some("gpt-5.6-sol") {
        return Err(format!(
            "installed Codex model contract changed: {}",
            body["model"].as_str().unwrap_or("<non-string>")
        ));
    }
    if !value_contains(&body["input"], "Zentty installed Codex integration probe") {
        return Err("installed Codex request omitted the controlled probe".to_owned());
    }
    Ok(body)
}

fn value_contains(value: &Value, needle: &str) -> bool {
    match value {
        Value::String(value) => value.contains(needle),
        Value::Array(values) => values.iter().any(|value| value_contains(value, needle)),
        Value::Object(values) => values.values().any(|value| value_contains(value, needle)),
        _ => false,
    }
}

fn append_receipt(path: &Path, body: &Value) -> Result<(), String> {
    let receipt = json!({
        "path": "/v1/responses",
        "model": body["model"].as_str().unwrap_or("<missing>"),
        "stream": body["stream"].as_bool().unwrap_or(false),
        "inputItems": body["input"].as_array().map_or(0, Vec::len),
        "containsProbe": value_contains(&body["input"], "Zentty installed Codex integration probe"),
    });
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| format!("could not open controlled Responses receipt: {error}"))?;
    writeln!(file, "{receipt}")
        .map_err(|error| format!("could not write controlled Responses receipt: {error}"))
}

fn completed_turn_body() -> String {
    let events = [
        json!({"type":"response.created","response":{"id":"resp_zentty_controlled"}}),
        json!({
            "type":"response.output_item.done",
            "item": {
                "type":"message",
                "role":"assistant",
                "id":"msg_zentty_controlled",
                "content":[{"type":"output_text","text":"Zentty controlled turn complete."}]
            }
        }),
        json!({
            "type":"response.completed",
            "response": {
                "id":"resp_zentty_controlled",
                "usage": {
                    "input_tokens":1,
                    "input_tokens_details":null,
                    "output_tokens":1,
                    "output_tokens_details":null,
                    "total_tokens":2
                }
            }
        }),
    ];
    let mut body = String::new();
    for event in events {
        let event_type = event["type"].as_str().expect("event type");
        write!(&mut body, "event: {event_type}\ndata: {event}\n\n")
            .expect("writing to a String cannot fail");
    }
    body
}

fn write_completed_turn(stream: &mut TcpStream) -> Result<(), String> {
    let body = completed_turn_body();
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    )
    .map_err(|error| format!("could not write controlled Responses turn: {error}"))
}

#[cfg(test)]
mod tests {
    use super::{
        HttpRequest, MAX_BODY_BYTES, MAX_HEADER_BYTES, append_receipt, completed_turn_body,
        exceeds_limit, parse_receipt_path, read_request, validate_request, write_completed_turn,
    };
    use serde_json::json;
    use std::io::{Read, Write};
    use std::net::{Shutdown, TcpListener, TcpStream};

    fn request(body: &serde_json::Value) -> HttpRequest {
        HttpRequest {
            method: "POST".to_owned(),
            path: "/v1/responses".to_owned(),
            body: body.to_string().into_bytes(),
        }
    }

    fn read_raw_request(parts: Vec<Vec<u8>>) -> Result<HttpRequest, String> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let writer = std::thread::spawn(move || {
            let mut stream = TcpStream::connect(address).unwrap();
            for part in parts {
                stream.write_all(&part).unwrap();
            }
            stream.shutdown(Shutdown::Write).unwrap();
        });
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_millis(250)))
            .unwrap();
        let result = read_request(&mut stream);
        writer.join().unwrap();
        result
    }

    #[test]
    fn receipt_path_and_request_contract_are_exact() {
        assert!(
            parse_receipt_path(["--receipt".into(), "/tmp/receipt".into()].into_iter()).is_ok()
        );
        assert!(parse_receipt_path(std::iter::empty()).is_err());
        assert!(parse_receipt_path(["--receipt".into(), "relative".into()].into_iter()).is_err());
        assert!(validate_request(&request(&json!({"model":"gpt-5.6-sol","stream":true,"input":[{"content":"Zentty installed Codex integration probe"}]}))).is_ok());
        assert!(validate_request(&request(&json!({"model":"wrong","stream":true,"input":["Zentty installed Codex integration probe"]}))).is_err());
        assert!(validate_request(&request(&json!({"model":"gpt-5.6-sol","stream":false,"input":["Zentty installed Codex integration probe"]}))).is_err());
        assert!(
            validate_request(&request(
                &json!({"model":"gpt-5.6-sol","stream":true,"input":["different prompt"]})
            ))
            .is_err()
        );
        let valid_body = json!({"model":"gpt-5.6-sol","stream":true,"input":["Zentty installed Codex integration probe"]});
        let mut wrong_method = request(&valid_body);
        wrong_method.method = "GET".to_owned();
        assert!(validate_request(&wrong_method).is_err());
        let mut wrong_path = request(&valid_body);
        wrong_path.path = "/wrong".to_owned();
        assert!(validate_request(&wrong_path).is_err());
        assert!(!exceeds_limit(MAX_HEADER_BYTES, MAX_HEADER_BYTES));
        assert!(exceeds_limit(MAX_HEADER_BYTES + 1, MAX_HEADER_BYTES));
        assert!(!exceeds_limit(MAX_BODY_BYTES, MAX_BODY_BYTES));
        assert!(exceeds_limit(MAX_BODY_BYTES + 1, MAX_BODY_BYTES));
    }

    #[test]
    fn http_parser_accepts_fragmentation_and_rejects_every_bounded_failure() {
        let body = br#"{"model":"gpt-5.6-sol","stream":true,"input":["Zentty installed Codex integration probe"]}"#;
        let head = format!(
            "POST /v1/responses HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: {}\r\n\r\n",
            body.len()
        );
        let parsed = read_raw_request(vec![head.into_bytes(), body.to_vec()]).unwrap();
        assert_eq!(parsed.method, "POST");
        assert_eq!(parsed.path, "/v1/responses");
        assert_eq!(parsed.body, body);

        for raw in [
            b"GET /v1/responses HTTP/1.1\r\nContent-Length: 0\r\n\r\n".as_slice(),
            b"POST /wrong HTTP/1.1\r\nContent-Length: 0\r\n\r\n".as_slice(),
            b"POST /v1/responses HTTP/1.0\r\nContent-Length: 0\r\n\r\n".as_slice(),
            b"POST /v1/responses HTTP/1.1\r\n\r\n".as_slice(),
            b"POST /v1/responses HTTP/1.1\r\nContent-Length: nope\r\n\r\n".as_slice(),
            b"POST /v1/responses HTTP/1.1\r\nContent-Length: 4\r\n\r\nxx".as_slice(),
        ] {
            assert!(read_raw_request(vec![raw.to_vec()]).is_err());
        }
        let oversized_body = format!(
            "POST /v1/responses HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
            MAX_BODY_BYTES + 1
        );
        assert!(read_raw_request(vec![oversized_body.into_bytes()]).is_err());
        let oversized_headers = vec![b'a'; MAX_HEADER_BYTES + 1];
        assert!(read_raw_request(vec![oversized_headers]).is_err());
    }

    #[test]
    fn completed_turn_matches_codex_sse_contract() {
        let body = completed_turn_body();
        assert!(body.contains("event: response.created\n"));
        assert!(body.contains("event: response.output_item.done\n"));
        assert!(body.contains("Zentty controlled turn complete."));
        assert!(body.contains("event: response.completed\n"));
        assert!(body.contains("\"total_tokens\":2"));
    }

    #[test]
    fn completed_turn_writer_emits_the_exact_http_boundary() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let reader = std::thread::spawn(move || {
            let mut stream = TcpStream::connect(address).unwrap();
            let mut response = String::new();
            stream.read_to_string(&mut response).unwrap();
            response
        });
        let (mut stream, _) = listener.accept().unwrap();
        write_completed_turn(&mut stream).unwrap();
        drop(stream);
        let response = reader.join().unwrap();
        assert!(response.starts_with("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n"));
        let (headers, body) = response.split_once("\r\n\r\n").unwrap();
        let expected_length = completed_turn_body().len();
        assert!(headers.contains(&format!("Content-Length: {expected_length}")));
        assert_eq!(body, completed_turn_body());
    }

    #[test]
    fn sanitized_receipt_never_contains_prompt_or_credentials() {
        let root = std::env::temp_dir().join(format!(
            "zentty-controlled-responses-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("receipt.jsonl");
        let secret = "controlled-api-secret";
        let prompt_secret = "prompt-secret-Zentty installed Codex integration probe";
        let body = json!({"model":"gpt-5.6-sol","stream":true,"input":[prompt_secret],"metadata":{"secret":secret}});
        append_receipt(&path, &body).unwrap();
        let receipt = std::fs::read_to_string(&path).unwrap();
        assert!(!receipt.contains(secret));
        assert!(!receipt.contains(prompt_secret));
        assert!(receipt.contains("\"containsProbe\":true"));
        std::fs::remove_dir_all(root).unwrap();
    }
}
