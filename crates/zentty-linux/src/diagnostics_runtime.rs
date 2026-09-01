use std::collections::BTreeMap;
use std::env;
use std::io::Write;
use std::panic::PanicHookInfo;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use zentty_core::{
    DiagnosticDraft, DiagnosticReason, DiagnosticReport, DiagnosticState, redact_text,
};

use crate::diagnostic_store::DiagnosticStore;

const REVIEWED_ENDPOINT: Option<&str> = option_env!("ZENTTY_DIAGNOSTICS_ENDPOINT");
static NEXT_REPORT_ID: AtomicU64 = AtomicU64::new(1);

pub(crate) fn default_diagnostic_store() -> Result<DiagnosticStore, String> {
    let root = env::var_os("XDG_STATE_HOME").map_or_else(
        || {
            env::var_os("HOME")
                .map(PathBuf::from)
                .map(|home| home.join(".local/state"))
                .ok_or_else(|| "HOME and XDG_STATE_HOME are both unavailable".to_owned())
        },
        |path| Ok(PathBuf::from(path)),
    )?;
    DiagnosticStore::new(root.join("zentty/diagnostics"))
}

pub(crate) fn create_manual_report() -> Result<DiagnosticReport, String> {
    create_report(
        DiagnosticReason::ManualSupport,
        "The user explicitly created a local Tornado TTY support report.",
    )
}

pub(crate) fn install_local_panic_capture(enabled: bool) {
    if !enabled {
        eprintln!("zentty-linux: diagnostics crash-capture=disabled network=disabled");
        return;
    }
    std::panic::set_hook(Box::new(move |information| {
        if let Err(error) = capture_panic(information) {
            eprintln!("zentty-linux: diagnostics panic-capture=failed detail={error}");
        }
    }));
    eprintln!("zentty-linux: diagnostics crash-capture=local-only network=disabled");
}

pub(crate) fn maybe_inject_controlled_crash(enabled: bool) {
    if !enabled || !controlled_test_session() {
        return;
    }
    if let Ok(detail) = env::var("ZENTTY_DIAGNOSTICS_CONTROLLED_CRASH") {
        panic!("controlled diagnostics crash: {detail}");
    }
}

pub(crate) fn transport_description() -> &'static str {
    if configured_endpoint().is_some() {
        "Explicit submission is available after local review and confirmation."
    } else {
        "No reviewed submission endpoint is configured; reports remain local."
    }
}

pub(crate) fn submission_available() -> bool {
    configured_endpoint().is_some()
}

pub(crate) fn mark_pending_review(report_id: &str) -> Result<DiagnosticReport, String> {
    default_diagnostic_store()?.transition(report_id, DiagnosticState::PendingReview)
}

pub(crate) fn mark_local(report_id: &str) -> Result<DiagnosticReport, String> {
    default_diagnostic_store()?.transition(report_id, DiagnosticState::Local)
}

pub(crate) fn clear_reports() -> Result<usize, String> {
    default_diagnostic_store()?.clear()
}

pub(crate) fn list_reports() -> Result<Vec<DiagnosticReport>, String> {
    default_diagnostic_store()?.list()
}

pub(crate) fn submit_reviewed_report(report_id: &str) -> Result<DiagnosticReport, String> {
    let endpoint = configured_endpoint()
        .ok_or_else(|| "no reviewed diagnostic submission endpoint is configured".to_owned())?;
    let store = default_diagnostic_store()?;
    let report = store
        .list()?
        .into_iter()
        .find(|report| report.report_id == report_id)
        .ok_or_else(|| "diagnostic report no longer exists".to_owned())?;
    if report.state != DiagnosticState::PendingReview {
        return Err("diagnostic report must be reviewed before submission".to_owned());
    }
    let payload = serde_json::to_vec(&report)
        .map_err(|error| format!("could not encode reviewed diagnostic report: {error}"))?;
    if let Err(error) = run_reviewed_transport(&endpoint, &payload) {
        let _ = store.transition(report_id, DiagnosticState::Failed);
        return Err(error);
    }
    store.transition(report_id, DiagnosticState::Sent)
}

fn run_reviewed_transport(endpoint: &str, payload: &[u8]) -> Result<(), String> {
    let mut child = Command::new("curl")
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--max-time",
            "10",
            "--request",
            "POST",
            "--header",
            "Content-Type: application/json",
            "--data-binary",
            "@-",
            endpoint,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("could not start reviewed diagnostic transport: {error}"))?;
    let write_result = child
        .stdin
        .take()
        .ok_or_else(|| "diagnostic transport did not expose stdin".to_owned())
        .and_then(|mut input| {
            input
                .write_all(payload)
                .map_err(|error| format!("could not write diagnostic payload: {error}"))
        });
    if let Err(error) = write_result {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
    }
    let output = child
        .wait_with_output()
        .map_err(|error| format!("could not wait for diagnostic transport: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "diagnostic submission failed: {}",
            redact_text(
                &String::from_utf8_lossy(&output.stderr),
                env::var("HOME").ok().as_deref()
            )
        ));
    }
    Ok(())
}

fn capture_panic(information: &PanicHookInfo<'_>) -> Result<(), String> {
    let location = information.location().map_or_else(
        || "unknown".to_owned(),
        |location| {
            let file = std::path::Path::new(location.file())
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("unknown");
            format!("{file}:{}:{}", location.line(), location.column())
        },
    );
    create_report(
        DiagnosticReason::Panic,
        &format!(
            "{} encountered an internal panic at {location}.",
            zentty_core::PRODUCT_NAME
        ),
    )?;
    Ok(())
}

fn create_report(reason: DiagnosticReason, detail: &str) -> Result<DiagnosticReport, String> {
    let now = now_epoch();
    let report_id = format!(
        "{now}-{}-{}",
        std::process::id(),
        NEXT_REPORT_ID.fetch_add(1, Ordering::Relaxed)
    );
    let context = safe_context();
    let home = env::var("HOME").ok();
    let report = DiagnosticReport::from_draft(&DiagnosticDraft {
        report_id: &report_id,
        created_at_epoch: now,
        reason,
        application_version: env!("CARGO_PKG_VERSION"),
        build_commit: option_env!("ZENTTY_BUILD_COMMIT").unwrap_or("unknown"),
        platform: env::consts::ARCH,
        detail,
        context: &context,
        home_directory: home.as_deref(),
    });
    let store = default_diagnostic_store()?;
    store.prune(now)?;
    let path = store.save(&report)?;
    eprintln!(
        "zentty-linux: diagnostics report={} state=local path={}",
        report.report_id,
        path.display()
    );
    Ok(report)
}

fn safe_context() -> BTreeMap<String, String> {
    [
        ("desktop", "XDG_CURRENT_DESKTOP"),
        ("display_backend", "XDG_SESSION_TYPE"),
        ("locale", "LANG"),
    ]
    .into_iter()
    .filter_map(|(field, variable)| {
        env::var(variable)
            .ok()
            .map(|value| (field.to_owned(), value))
    })
    .collect()
}

fn configured_endpoint() -> Option<String> {
    if let Some(endpoint) = REVIEWED_ENDPOINT.filter(|endpoint| safe_https_endpoint(endpoint)) {
        return Some(endpoint.to_owned());
    }
    let endpoint = env::var("ZENTTY_DIAGNOSTICS_TEST_ENDPOINT").ok()?;
    safe_test_endpoint(&endpoint, controlled_test_session()).then_some(endpoint)
}

fn safe_test_endpoint(endpoint: &str, controlled_session: bool) -> bool {
    let loopback = endpoint
        .strip_prefix("http://127.0.0.1:")
        .and_then(|tail| tail.split_once('/'))
        .is_some_and(|(port, _)| port.parse::<u16>().is_ok());
    controlled_session && loopback
}

fn controlled_test_session() -> bool {
    env::var("ZENTTY_NESTED_X11_SESSION_ID")
        .ok()
        .or_else(|| env::var("ZENTTY_NESTED_WAYLAND_INPUT_SESSION_ID").ok())
        .is_some_and(|value| valid_controlled_session_id(&value))
}

fn valid_controlled_session_id(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|character| character.is_ascii_hexdigit())
}

fn safe_https_endpoint(endpoint: &str) -> bool {
    let Some(authority_and_path) = endpoint.strip_prefix("https://") else {
        return false;
    };
    let authority = authority_and_path.split('/').next().unwrap_or_default();
    !authority.is_empty()
        && !authority.contains(['@', ':'])
        && authority.contains('.')
        && !endpoint.contains(['\n', '\r', ' '])
}

fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_transport_is_absent_or_a_build_reviewed_https_origin() {
        assert!(safe_https_endpoint("https://support.example/v1/reports"));
        if let Some(endpoint) = REVIEWED_ENDPOINT {
            assert!(safe_https_endpoint(endpoint));
        }
        for hostile in [
            "http://support.example/report",
            "https://token@support.example/report",
            "https://localhost/report",
            "https://support.example:8443/report",
            "https://support.example/report\nfile:///tmp/pwn",
        ] {
            assert!(!safe_https_endpoint(hostile));
        }
    }

    #[test]
    fn context_allowlist_never_collects_commands_tokens_or_paths() {
        let context = safe_context();
        assert!(
            context
                .keys()
                .all(|key| matches!(key.as_str(), "desktop" | "display_backend" | "locale"))
        );
    }

    #[test]
    fn test_transport_requires_wrapper_proof_and_exact_ipv4_loopback_shape() {
        assert!(safe_test_endpoint(
            "http://127.0.0.1:39000/v1/reports",
            true
        ));
        for (endpoint, controlled) in [
            ("http://127.0.0.1:39000/v1/reports", false),
            ("http://localhost:39000/v1/reports", true),
            ("http://127.0.0.1:not-a-port/v1/reports", true),
            ("http://127.0.0.1:39000", true),
            ("https://127.0.0.1:39000/v1/reports", true),
        ] {
            assert!(!safe_test_endpoint(endpoint, controlled));
        }
        assert!(valid_controlled_session_id(&"a".repeat(64)));
        assert!(!valid_controlled_session_id(&"a".repeat(63)));
        assert!(!valid_controlled_session_id(&format!(
            "{}z",
            "a".repeat(63)
        )));
    }
}
