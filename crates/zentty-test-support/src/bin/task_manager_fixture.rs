#![forbid(unsafe_code)]

use std::hint::black_box;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

const LIFETIME: Duration = Duration::from_secs(90);
const PARENT_MEMORY_BYTES: usize = 32 * 1024 * 1024;
const CHILD_MEMORY_BYTES: usize = 24 * 1024 * 1024;

fn main() -> Result<(), String> {
    let (child_mode, receipt) = arguments()?;
    if child_mode {
        publish(&receipt, "child", std::process::id())?;
        exercise(CHILD_MEMORY_BYTES, true)
    } else {
        publish(&receipt, "root", std::process::id())?;
        let mut child = Command::new(
            std::env::current_exe()
                .map_err(|error| format!("could not resolve fixture executable: {error}"))?,
        )
        .args(["--child", "--receipt"])
        .arg(&receipt)
        .spawn()
        .map_err(|error| format!("could not spawn fixture child: {error}"))?;
        let result = exercise(PARENT_MEMORY_BYTES, false);
        terminate(&mut child);
        result
    }
}

fn arguments() -> Result<(bool, PathBuf), String> {
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    let child_mode = arguments.iter().any(|argument| argument == "--child");
    let receipt_index = arguments
        .iter()
        .position(|argument| argument == "--receipt")
        .ok_or_else(|| {
            "usage: task_manager_fixture [--child] --receipt ABSOLUTE_PATH".to_owned()
        })?;
    let receipt = arguments
        .get(receipt_index + 1)
        .map(PathBuf::from)
        .ok_or_else(|| "--receipt requires a path".to_owned())?;
    if !receipt.is_absolute() {
        return Err("task-manager receipt must be absolute".to_owned());
    }
    Ok((child_mode, receipt))
}

fn publish(receipt: &Path, role: &str, pid: u32) -> Result<(), String> {
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(receipt)
        .map_err(|error| format!("could not open fixture receipt: {error}"))?;
    writeln!(file, "{role}={pid}")
        .and_then(|()| file.flush())
        .map_err(|error| format!("could not publish fixture receipt: {error}"))
}

fn exercise(memory_bytes: usize, busy: bool) -> Result<(), String> {
    let mut memory = vec![0_u8; memory_bytes];
    for page in memory.chunks_mut(4096) {
        page[0] = 0x5a;
    }
    let deadline = Instant::now() + LIFETIME;
    let mut value = 1_u64;
    while Instant::now() < deadline {
        if busy {
            for _ in 0..100_000 {
                value = value.rotate_left(7).wrapping_mul(6_364_136_223_846_793_005);
            }
            black_box(value);
        } else {
            black_box(&memory);
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    Err("task-manager fixture exceeded its bounded lifetime".to_owned())
}

fn terminate(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}
