//! Bounded real-client/PTY producer for the existing agent IPC journey.
use std::fs::{self, OpenOptions};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use zentty_agent_ipc::{AgentIpcClient, AgentIpcError};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(std::env::var("ZENTTY_LOAD_DIRECTORY")?);
    let helper = std::env::var("ZENTTY_CLI_BIN")?;
    if std::env::var("ZENTTY_PANE_ID")? != "pane-1" {
        fs::write(root.join("quiet-ready"), b"ready")?;
        for line in std::io::stdin().lock().lines() {
            let line = line?;
            let result = Command::new(&helper)
                .args(["list", "panes", "--json"])
                .output()?;
            if !result.status.success() {
                return Err("quiet pane IPC failed".into());
            }
            writeln!(
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(root.join("probes"))?,
                "{line}"
            )?;
        }
        return Ok(());
    }
    if !Command::new(&helper)
        .args(["split", "right", "--equal"])
        .output()?
        .status
        .success()
    {
        return Err("could not create quiet pane".into());
    }
    let socket = PathBuf::from(std::env::var("ZENTTY_INSTANCE_SOCKET")?);
    let token = std::env::var("ZENTTY_PANE_TOKEN")?;
    let token = token
        .strip_prefix("@file:")
        .map_or_else(|| Ok(token.clone()), fs::read_to_string)?;
    let token = token.trim().to_owned();
    if token.len() != 64 {
        return Err("invalid private fixture credential".into());
    }
    fs::write(root.join("busy-ready"), b"ready")?;
    wait_for(&root.join("start"))?;
    let stop = Arc::new(AtomicBool::new(false));
    let event = serde_json::to_vec(&serde_json::json!({
        "version":1,"event":"agent.running","agent":{"name":"Codex","pid":std::process::id()},
        "session":{"id":"bounded-load"},"extension":"x".repeat(60000)
    }))?;
    let workers = (0..4)
        .map(|_| {
            let (socket, token, event, stop) = (
                socket.clone(),
                token.clone(),
                event.clone(),
                Arc::clone(&stop),
            );
            std::thread::spawn(move || {
                let (mut accepted, mut rejected) = (0_u64, 0_u64);
                while !stop.load(Ordering::Acquire) {
                    match AgentIpcClient::send_event(&socket, &token, &event, None) {
                        Ok(()) => accepted += 1,
                        Err(AgentIpcError::Remote { code, .. }) if code == "ingress_full" => {
                            rejected += 1
                        }
                        Err(error) => return Err(error.to_string()),
                    }
                }
                Ok((accepted, rejected))
            })
        })
        .collect::<Vec<_>>();
    fs::write(root.join("active"), b"active")?;
    let mut expected = fs::File::create(root.join("expected.txt"))?;
    let mut stdout = std::io::stdout().lock();
    let start = Instant::now();
    // 64 MiB through the real bounded PTY pipeline. The alternate screen has
    // no scrollback by design; leaving it must restore the untouched normal
    // screen before the exact retained-content phase below.
    write!(stdout, "\x1b[?1049h")?;
    let bulk = vec![b'f'; 262_144];
    for _ in 0..256 {
        stdout.write_all(&bulk)?;
        stdout.flush()?;
        std::thread::sleep(Duration::from_millis(16));
    }
    write!(stdout, "\x1b[?1049l")?;
    // Each line crosses real VT parsing (color, carriage return, erase line).
    // One pathological line spans 64 KiB; selection must rejoin its soft wraps.
    for index in 0..200 {
        let text = format!(
            "record-{index:04}: {} λ",
            "x".repeat(if index == 50 { 65536 } else { 4096 })
        );
        write!(stdout, "discard-me\r\x1b[2K\x1b[32m{text}\x1b[0m\r\n")?;
        stdout.flush()?;
        if index > 0 {
            writeln!(expected)?;
        }
        // Select All excludes the final empty row, retaining every record and
        // hard break between records (soft wraps are rejoined by the engine).
        write!(expected, "{text}")?;
        std::thread::sleep(Duration::from_millis(30));
    }
    // A distinct title is a real PTY/VT completion barrier, not producer ACK.
    write!(stdout, "\x1b]0;bounded-load-complete\x07")?;
    stdout.flush()?;
    drop(stdout);
    stop.store(true, Ordering::Release);
    let (mut accepted, mut rejected) = (0_u64, 0_u64);
    for worker in workers {
        let (a, r) = worker.join().map_err(|_| "load worker panicked")??;
        accepted += a;
        rejected += r;
    }
    fs::write(
        root.join("completed"),
        format!(
            "accepted={accepted}\nrejected={rejected}\nduration-ms={}\n",
            start.elapsed().as_millis()
        ),
    )?;
    wait_for(&root.join("finish"))?;
    Ok(())
}

fn wait_for(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let until = Instant::now() + Duration::from_secs(30);
    while !path.exists() {
        if Instant::now() >= until {
            return Err("fixture gate deadline".into());
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    Ok(())
}
