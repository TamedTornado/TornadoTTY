use std::env;
use std::path::PathBuf;

use zentty_core::{
    SaveReason, SessionRestoreEnvelope, SessionRestoreStore, WindowRecipe, WorkspaceRecipe,
};

fn run() -> Result<(), String> {
    let mut arguments = env::args_os().skip(1);
    let state_directory = arguments
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| "missing state directory".to_owned())?;
    let payload_bytes = arguments
        .next()
        .ok_or_else(|| "missing payload byte count".to_owned())?
        .to_string_lossy()
        .parse::<usize>()
        .map_err(|error| format!("invalid payload byte count: {error}"))?;
    if arguments.next().is_some() {
        return Err("unexpected argument".to_owned());
    }

    let store = SessionRestoreStore::new(
        state_directory.join("restore-snapshot.json"),
        state_directory.join("restore-lifecycle.json"),
    );
    let payload = "n".repeat(payload_bytes);
    store
        .save_snapshot(&SessionRestoreEnvelope {
            schema_version: 1,
            saved_at: 807_321_600.0,
            reason: SaveReason::LiveSnapshot,
            workspace: WorkspaceRecipe {
                schema_version: Some(WorkspaceRecipe::CURRENT_SCHEMA_VERSION),
                windows: vec![WindowRecipe {
                    id: format!("window-{payload}"),
                    frame: None,
                    worklanes: Vec::new(),
                    active_worklane_id: None,
                }],
                active_window_id: None,
            },
            restore_draft_windows: Vec::new(),
        })
        .map_err(|error| error.to_string())?;
    println!("workspace-recovery-writer: COMPLETE payload_bytes={payload_bytes}");
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("workspace-recovery-writer: error: {error}");
        std::process::exit(1);
    }
}
