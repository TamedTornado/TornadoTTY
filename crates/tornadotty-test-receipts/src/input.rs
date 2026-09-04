//! Compositor-visible input actions bound to a supervised product session.

use std::path::Path;
use std::process::{Command, Output};

use crate::session;

/// Stable error categories reported by compositor-input commands.
#[derive(Debug)]
pub enum InputError {
    Arguments(String),
    Identity(String),
    Delivery(String),
}

impl std::fmt::Display for InputError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (category, detail) = match self {
            Self::Arguments(detail) => ("arguments", detail),
            Self::Identity(detail) => ("identity", detail),
            Self::Delivery(detail) => ("delivery", detail),
        };
        write!(formatter, "input-{category}: {detail}")
    }
}

impl std::error::Error for InputError {}

const MAX_KEY_BYTES: usize = 64;
const MAX_TEXT_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy)]
enum Transport {
    X11,
    OuterX11,
    Wayland,
}

/// Runs an `input` subcommand for the journey-driver binary.
///
/// # Errors
///
/// Returns an error for invalid arguments, stale product identity, foreign
/// X11 windows, missing controlled-Wayland attestation, or failed input tools.
pub fn run(arguments: &[String]) -> Result<(), InputError> {
    match arguments {
        [command, session, transport, window] if command == "verify" => {
            let transport = parse_transport(transport).map_err(InputError::Arguments)?;
            verify_target(Path::new(session), transport, window).map_err(InputError::Identity)
        }
        [command, session, transport, window, chord] if command == "key" => {
            let transport = parse_transport(transport).map_err(InputError::Arguments)?;
            validate_key(chord).map_err(InputError::Arguments)?;
            verify_target(Path::new(session), transport, window).map_err(InputError::Identity)?;
            send_key(transport, window, chord).map_err(InputError::Delivery)
        }
        [command, session, transport, window, value] if command == "type" => {
            let transport = parse_transport(transport).map_err(InputError::Arguments)?;
            if value.len() > MAX_TEXT_BYTES {
                return Err(InputError::Arguments(
                    "input text exceeds 64 KiB".to_owned(),
                ));
            }
            verify_target(Path::new(session), transport, window).map_err(InputError::Identity)?;
            type_text(transport, window, value).map_err(InputError::Delivery)
        }
        _ => Err(InputError::Arguments(usage().to_owned())),
    }
}

fn parse_transport(value: &str) -> Result<Transport, String> {
    match value {
        "x11" => Ok(Transport::X11),
        "outer-x11" => Ok(Transport::OuterX11),
        "wayland" => Ok(Transport::Wayland),
        _ => Err(format!("unsupported input transport: {value}")),
    }
}

fn verify_target(session: &Path, transport: Transport, window: &str) -> Result<(), String> {
    let product_pid = session::live_product_pid(session)?;
    match transport {
        Transport::X11 => verify_x11_owner(window, product_pid),
        Transport::OuterX11 => {
            require_wayland_attestation()?;
            if std::env::var("ZENTTY_NESTED_WAYLAND_OUTER_X11_INPUT").as_deref() != Ok("1") {
                return Err("outer-X11 input lacks controlled compositor attestation".to_owned());
            }
            let expected = std::env::var("ZENTTY_NESTED_WAYLAND_OUTER_X11_PID")
                .map_err(|_| "outer-X11 input lacks compositor PID".to_owned())?
                .parse::<u32>()
                .map_err(|_| "outer-X11 compositor PID is malformed".to_owned())?;
            verify_x11_owner(window, expected)
        }
        Transport::Wayland => {
            require_wayland_attestation()?;
            if window != "-" {
                return Err("native Wayland input must not name an X11 window".to_owned());
            }
            if product_pid == 0 {
                return Err("supervised product identity is invalid".to_owned());
            }
            Ok(())
        }
    }
}

fn require_wayland_attestation() -> Result<(), String> {
    let session_id = std::env::var("ZENTTY_NESTED_WAYLAND_INPUT_SESSION_ID")
        .map_err(|_| "Wayland input is outside the controlled input session".to_owned())?;
    if session_id.len() != 64 || !session_id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("controlled Wayland input session identity is malformed".to_owned());
    }
    Ok(())
}

fn verify_x11_owner(window: &str, expected_pid: u32) -> Result<(), String> {
    let window_id = window
        .parse::<u64>()
        .map_err(|_| "X11 window ID must be a positive integer".to_owned())?;
    if window_id == 0 {
        return Err("X11 window ID must be positive".to_owned());
    }
    let output = Command::new("xdotool")
        .args(["getwindowpid", window])
        .output()
        .map_err(|error| format!("could not inspect X11 window owner: {error}"))?;
    require_success("xdotool getwindowpid", &output)?;
    let owner = String::from_utf8(output.stdout)
        .map_err(|_| "X11 window owner was not UTF-8".to_owned())?
        .trim()
        .parse::<u32>()
        .map_err(|_| "X11 window owner was malformed".to_owned())?;
    if owner != expected_pid {
        return Err(format!(
            "foreign X11 window rejected: window={window_id} owner={owner} expected={expected_pid}"
        ));
    }
    Ok(())
}

fn send_key(transport: Transport, window: &str, chord: &str) -> Result<(), String> {
    match transport {
        Transport::X11 | Transport::OuterX11 => {
            run_tool(
                "xdotool windowfocus",
                Command::new("xdotool").args(["windowfocus", "--sync", window]),
            )?;
            run_tool("xdotool key", Command::new("xdotool").args(["key", chord]))
        }
        Transport::Wayland => {
            let (modifiers, key) = parse_chord(chord)?;
            let mut command = Command::new("wtype");
            for modifier in &modifiers {
                command.args(["-M", modifier]);
            }
            command.args(["-k", key]);
            for modifier in modifiers.iter().rev() {
                command.args(["-m", modifier]);
            }
            run_tool("wtype key", &mut command)
        }
    }
}

fn type_text(transport: Transport, window: &str, value: &str) -> Result<(), String> {
    match transport {
        Transport::X11 | Transport::OuterX11 => {
            run_tool(
                "xdotool windowfocus",
                Command::new("xdotool").args(["windowfocus", "--sync", window]),
            )?;
            run_tool(
                "xdotool type",
                Command::new("xdotool").args(["type", "--delay", "5", "--", value]),
            )
        }
        Transport::Wayland => run_tool(
            "wtype text",
            Command::new("wtype").args(["-d", "5", "--", value]),
        ),
    }
}

fn parse_chord(chord: &str) -> Result<(Vec<&str>, &str), String> {
    let mut parts = chord.split('+').collect::<Vec<_>>();
    let key = parts.pop().ok_or_else(|| "key chord is empty".to_owned())?;
    for modifier in &parts {
        if !matches!(*modifier, "ctrl" | "alt" | "shift" | "super") {
            return Err(format!("unsupported key modifier: {modifier}"));
        }
    }
    if key.is_empty() {
        return Err("key chord has no key".to_owned());
    }
    Ok((parts, key))
}

fn validate_key(chord: &str) -> Result<(), String> {
    if chord.is_empty()
        || chord.len() > MAX_KEY_BYTES
        || !chord
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'_' | b'.'))
    {
        return Err("key chord must be bounded safe ASCII".to_owned());
    }
    Ok(())
}

fn run_tool(label: &str, command: &mut Command) -> Result<(), String> {
    let output = command
        .output()
        .map_err(|error| format!("could not execute {label}: {error}"))?;
    require_success(label, &output)
}

fn require_success(label: &str, output: &Output) -> Result<(), String> {
    if output.status.success() {
        Ok(())
    } else {
        Err(format!("{label} failed with {}", output.status))
    }
}

fn usage() -> &'static str {
    "input usage:\n  tornadotty-journey-driver input verify SESSION x11|outer-x11|wayland WINDOW-OR--\n  tornadotty-journey-driver input key SESSION x11|outer-x11|wayland WINDOW-OR-- CHORD\n  tornadotty-journey-driver input type SESSION x11|outer-x11|wayland WINDOW-OR-- TEXT"
}
