#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use serde_json::json;
use zentty_core::{RemoteUploadPath, parse_ssh_destination};
use zentty_linux::remote_transfer::{RemoteTransferRequest, execute_remote_transfer};

fn main() -> Result<(), String> {
    let arguments = std::env::args().collect::<Vec<_>>();
    let separator = arguments
        .iter()
        .position(|argument| argument == "--ssh-argv")
        .ok_or_else(|| "remote transfer actor requires --ssh-argv".to_owned())?;
    let source = required_value(&arguments[..separator], "--source").map(PathBuf::from)?;
    let filename = required_value(&arguments[..separator], "--filename")?;
    let timestamp = required_value(&arguments[..separator], "--timestamp")?
        .parse::<u64>()
        .map_err(|_| "--timestamp must be an unsigned integer".to_owned())?;
    let nonce = required_value(&arguments[..separator], "--nonce")?;
    let ssh_argv = arguments[separator + 1..]
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let destination = parse_ssh_destination(&ssh_argv)
        .ok_or_else(|| "remote transfer actor could not parse SSH argv".to_owned())?;
    let upload_path = RemoteUploadPath::for_file(filename, timestamp, nonce)
        .map_err(|error| format!("invalid remote upload path: {error:?}"))?;
    let receipt = execute_remote_transfer(
        &RemoteTransferRequest {
            source,
            destination,
            upload_path,
            maximum_bytes: 500 * 1024 * 1024,
            timeout: Duration::from_secs(20),
        },
        &AtomicBool::new(false),
    )
    .map_err(|error| format!("{:?}: {}", error.failure, error.detail))?;
    println!(
        "{}",
        json!({
            "remotePath": receipt.remote_path,
            "byteCount": receipt.byte_count,
            "sha256": receipt.sha256,
        })
    );
    Ok(())
}

fn required_value<'a>(arguments: &'a [String], name: &str) -> Result<&'a str, String> {
    let position = arguments
        .iter()
        .position(|argument| argument == name)
        .ok_or_else(|| format!("remote transfer actor requires {name}"))?;
    arguments
        .get(position + 1)
        .map(String::as_str)
        .ok_or_else(|| format!("{name} requires a value"))
}
