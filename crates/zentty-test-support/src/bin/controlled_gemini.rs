#![forbid(unsafe_code)]

use serde_json::{Value, json};
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

const MAX_HEADER_BYTES: usize = 65_536;
const MAX_BODY_BYTES: usize = 16_777_216;
const PROBE: &str = "Zentty installed Gemini integration probe";

fn main() -> Result<(), String> {
    let receipt = parse_receipt_path(std::env::args_os().skip(1))?;
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .map_err(|error| format!("could not bind controlled Gemini endpoint: {error}"))?;
    let address = listener
        .local_addr()
        .map_err(|error| format!("could not identify controlled Gemini endpoint: {error}"))?;
    println!("CONTROLLED_GEMINI_READY=http://{address}");
    std::io::stdout()
        .flush()
        .map_err(|error| format!("could not publish Gemini endpoint readiness: {error}"))?;

    let (mut stream, peer) = listener
        .accept()
        .map_err(|error| format!("controlled Gemini accept failed: {error}"))?;
    if !peer.ip().is_loopback() {
        return Err("controlled Gemini endpoint rejected a non-loopback peer".to_owned());
    }
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|error| format!("could not bound Gemini request reads: {error}"))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|error| format!("could not bound Gemini response writes: {error}"))?;
    let request = read_request(&mut stream)?;
    let body = validate_request(&request)?;
    append_receipt(&receipt, &request.path, &body)?;
    write_completed_turn(&mut stream)
}

fn parse_receipt_path(
    mut arguments: impl Iterator<Item = std::ffi::OsString>,
) -> Result<PathBuf, String> {
    if arguments.next().as_deref() != Some(std::ffi::OsStr::new("--receipt")) {
        return Err("usage: controlled_gemini --receipt PATH".to_owned());
    }
    let receipt = arguments
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| "--receipt requires a path".to_owned())?;
    if arguments.next().is_some() || !receipt.is_absolute() {
        return Err("controlled Gemini receipt must be one absolute path".to_owned());
    }
    Ok(receipt)
}

struct HttpRequest {
    path: String,
    body: Vec<u8>,
}

fn read_request(stream: &mut TcpStream) -> Result<HttpRequest, String> {
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 8192];
    let header_end = loop {
        let count = stream
            .read(&mut chunk)
            .map_err(|error| format!("controlled Gemini header read failed: {error}"))?;
        if count == 0 {
            return Err("controlled Gemini request ended before its headers".to_owned());
        }
        bytes.extend_from_slice(&chunk[..count]);
        if bytes.len() > MAX_HEADER_BYTES {
            return Err("controlled Gemini headers exceeded 64 KiB".to_owned());
        }
        if let Some(offset) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break offset + 4;
        }
    };
    let headers = std::str::from_utf8(&bytes[..header_end])
        .map_err(|_| "controlled Gemini headers were not UTF-8".to_owned())?;
    let mut lines = headers.lines();
    let request_line = lines
        .next()
        .ok_or_else(|| "controlled Gemini request line is missing".to_owned())?;
    let mut parts = request_line.split_whitespace();
    if parts.next() != Some("POST") {
        return Err("controlled Gemini endpoint requires POST".to_owned());
    }
    let path = parts.next().unwrap_or_default().to_owned();
    if parts.next() != Some("HTTP/1.1") || parts.next().is_some() {
        return Err("controlled Gemini endpoint requires HTTP/1.1".to_owned());
    }
    if !path.starts_with("/v1beta/models/") || !path.contains(":streamGenerateContent") {
        return Err(format!(
            "controlled Gemini endpoint rejected target: {path}"
        ));
    }
    let content_length = lines
        .filter_map(|line| line.split_once(':'))
        .find_map(|(name, value)| {
            name.eq_ignore_ascii_case("content-length")
                .then_some(value.trim())
        })
        .ok_or_else(|| "controlled Gemini request is missing Content-Length".to_owned())?
        .parse::<usize>()
        .map_err(|_| "controlled Gemini Content-Length is invalid".to_owned())?;
    if content_length > MAX_BODY_BYTES {
        return Err("controlled Gemini body exceeded 16 MiB".to_owned());
    }
    let received = bytes.len() - header_end;
    let missing = content_length.saturating_sub(received);
    let body_end = bytes.len() + missing;
    bytes.resize(body_end, 0);
    stream
        .read_exact(&mut bytes[body_end - missing..body_end])
        .map_err(|error| format!("controlled Gemini body read failed: {error}"))?;
    Ok(HttpRequest {
        path,
        body: bytes[header_end..header_end + content_length].to_vec(),
    })
}

fn validate_request(request: &HttpRequest) -> Result<Value, String> {
    let body: Value = serde_json::from_slice(&request.body)
        .map_err(|error| format!("controlled Gemini JSON is invalid: {error}"))?;
    if !value_contains(&body, PROBE) {
        return Err("installed Gemini request omitted the controlled probe".to_owned());
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

fn append_receipt(path: &Path, request_path: &str, body: &Value) -> Result<(), String> {
    let receipt = json!({
        "path": request_path,
        "containsProbe": value_contains(body, PROBE),
        "contentCount": body["contents"].as_array().map_or(0, Vec::len),
    });
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| format!("could not open Gemini receipt: {error}"))?;
    writeln!(file, "{receipt}").map_err(|error| format!("could not write Gemini receipt: {error}"))
}

fn completed_turn_body() -> String {
    let response = json!({
        "candidates": [{
            "content": {"role": "model", "parts": [{"text": "Zentty controlled Gemini turn complete."}]},
            "finishReason": "STOP",
            "index": 0
        }],
        "usageMetadata": {"promptTokenCount": 1, "candidatesTokenCount": 1, "totalTokenCount": 2},
        "modelVersion": "gemini-2.5-flash"
    });
    format!("data: {response}\n\n")
}

fn write_completed_turn(stream: &mut TcpStream) -> Result<(), String> {
    let body = completed_turn_body();
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    )
    .map_err(|error| format!("could not write controlled Gemini turn: {error}"))
}

#[cfg(test)]
mod tests {
    use super::{HttpRequest, PROBE, completed_turn_body, validate_request};
    use serde_json::json;

    #[test]
    fn request_requires_the_real_probe() {
        let valid = HttpRequest {
            path: "/v1beta/models/gemini-2.5-flash:streamGenerateContent?alt=sse".to_owned(),
            body: json!({"contents":[{"parts":[{"text":PROBE}]}]})
                .to_string()
                .into_bytes(),
        };
        assert!(validate_request(&valid).is_ok());
        let invalid = HttpRequest {
            body: b"{}".to_vec(),
            ..valid
        };
        assert!(validate_request(&invalid).is_err());
    }

    #[test]
    fn response_is_one_finished_streaming_turn() {
        let body = completed_turn_body();
        assert!(body.starts_with("data: "));
        assert!(body.contains("Zentty controlled Gemini turn complete."));
        assert!(body.contains("\"finishReason\":\"STOP\""));
        assert!(body.ends_with("\n\n"));
    }
}
