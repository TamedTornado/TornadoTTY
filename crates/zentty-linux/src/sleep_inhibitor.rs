use std::ffi::OsStr;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

const HELPER_ARGUMENT: &str = "--zentty-sleep-inhibitor-lease";
const READY_RECEIPT: &str = "zentty-sleep-inhibitor-ready";
const ACQUISITION_DEADLINE: Duration = Duration::from_secs(5);

pub(crate) fn run_helper_if_requested() -> Result<bool, String> {
    let mut arguments = std::env::args_os();
    let _program = arguments.next();
    if arguments.next().as_deref() != Some(OsStr::new(HELPER_ARGUMENT)) {
        return Ok(false);
    }
    if arguments.next().is_some() {
        return Err("sleep-inhibitor helper rejects additional arguments".to_owned());
    }
    run_lease_body(io::stdin().lock(), io::stdout().lock()).map_err(|error| error.to_string())?;
    Ok(true)
}

fn run_lease_body(mut input: impl Read, mut output: impl Write) -> io::Result<()> {
    writeln!(output, "{READY_RECEIPT}")?;
    output.flush()?;
    io::copy(&mut input, &mut io::sink())?;
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SleepInhibitorCapability {
    pub(crate) executable: Option<PathBuf>,
}

impl SleepInhibitorCapability {
    pub(crate) fn discover() -> Self {
        Self {
            executable: find_program("systemd-inhibit"),
        }
    }

    pub(crate) fn available(&self) -> bool {
        self.executable.is_some()
    }
}

pub(crate) struct SystemdSleepInhibitor {
    capability: SleepInhibitorCapability,
    lease: Option<Lease>,
}

impl SystemdSleepInhibitor {
    pub(crate) fn new(capability: SleepInhibitorCapability) -> Self {
        Self {
            capability,
            lease: None,
        }
    }

    pub(crate) fn acquire(&mut self) -> Result<(), String> {
        if self.lease.is_some() {
            return Ok(());
        }
        let executable = self
            .capability
            .executable
            .as_ref()
            .ok_or_else(|| "systemd-inhibit is not available".to_owned())?;
        let product = std::env::current_exe()
            .map_err(|error| format!("could not resolve Tornado TTY executable: {error}"))?;
        let mut child = Command::new(executable)
            .arg("--what=sleep")
            .arg("--mode=block")
            .arg("--who=Tornado TTY")
            .arg("--why=Tornado TTY agent is running")
            .arg(product)
            .arg(HELPER_ARGUMENT)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("could not start systemd-inhibit: {error}"))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "systemd-inhibit did not expose its lease pipe".to_owned())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "systemd-inhibit did not expose readiness output".to_owned())?;
        let (sender, readiness) = mpsc::sync_channel(1);
        let readiness_thread = match thread::Builder::new()
            .name("zentty-sleep-inhibitor-ready".to_owned())
            .spawn(move || {
                let mut line = String::new();
                let outcome = BufReader::new(stdout)
                    .take(256)
                    .read_line(&mut line)
                    .map_err(|error| error.to_string())
                    .and_then(|_| {
                        if line.trim_end() == READY_RECEIPT {
                            Ok(())
                        } else {
                            Err(format!("unexpected readiness receipt: {line:?}"))
                        }
                    });
                let _ = sender.send(outcome);
            }) {
            Ok(thread) => thread,
            Err(error) => {
                drop(stdin);
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "could not start inhibitor readiness reader: {error}"
                ));
            }
        };
        let pid = child.id();
        self.lease = Some(Lease {
            child,
            stdin: Some(stdin),
            readiness,
            readiness_thread: Some(readiness_thread),
            ready: false,
            acquisition_started: Instant::now(),
        });
        eprintln!("zentty-linux: sleep-inhibitor state=acquiring pid={pid} backend=systemd-logind");
        Ok(())
    }

    pub(crate) fn poll(&mut self) -> LeasePoll {
        let Some(lease) = self.lease.as_mut() else {
            return LeasePoll::Absent;
        };
        if !lease.ready {
            match lease.readiness.try_recv() {
                Ok(Ok(())) => {
                    lease.ready = true;
                    eprintln!(
                        "zentty-linux: sleep-inhibitor state=acquired pid={} backend=systemd-logind what=sleep mode=block",
                        lease.child.id()
                    );
                }
                Ok(Err(error)) => return self.lose_lease(&error),
                Err(TryRecvError::Disconnected) => {
                    return self.lose_lease("readiness reader disconnected");
                }
                Err(TryRecvError::Empty) => {}
            }
            if lease.acquisition_started.elapsed() >= ACQUISITION_DEADLINE {
                return self.lose_lease("acquisition readiness deadline expired");
            }
        }
        match lease.child.try_wait() {
            Ok(Some(status)) => self.lose_lease(&format!("systemd-inhibit exited with {status}")),
            Ok(None) if lease.ready => LeasePoll::Acquired,
            Ok(None) => LeasePoll::Acquiring,
            Err(error) => self.lose_lease(&format!("could not inspect systemd-inhibit: {error}")),
        }
    }

    pub(crate) fn release(&mut self, reason: &str) {
        let Some(mut lease) = self.lease.take() else {
            return;
        };
        lease.stdin.take();
        if lease.child.try_wait().ok().flatten().is_none() {
            let _ = lease.child.kill();
        }
        let _ = lease.child.wait();
        if let Some(thread) = lease.readiness_thread.take() {
            let _ = thread.join();
        }
        eprintln!("zentty-linux: sleep-inhibitor state=released reason={reason}");
    }

    fn lose_lease(&mut self, detail: &str) -> LeasePoll {
        let mut lease = self.lease.take().expect("lease exists while polling");
        lease.stdin.take();
        if lease.child.try_wait().ok().flatten().is_none() {
            let _ = lease.child.kill();
        }
        let _ = lease.child.wait();
        if let Some(thread) = lease.readiness_thread.take() {
            let _ = thread.join();
        }
        let mut stderr = String::new();
        if let Some(mut stream) = lease.child.stderr.take() {
            let _ = stream.by_ref().take(4096).read_to_string(&mut stderr);
        }
        let stderr = stderr.trim();
        eprintln!(
            "zentty-linux: sleep-inhibitor state=failed detail={} stderr={}",
            sanitize_log_field(detail),
            sanitize_log_field(if stderr.is_empty() { "none" } else { stderr })
        );
        LeasePoll::Lost
    }
}

impl Drop for SystemdSleepInhibitor {
    fn drop(&mut self) {
        self.release("application-shutdown");
    }
}

struct Lease {
    child: Child,
    stdin: Option<ChildStdin>,
    readiness: Receiver<Result<(), String>>,
    readiness_thread: Option<thread::JoinHandle<()>>,
    ready: bool,
    acquisition_started: Instant,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LeasePoll {
    Absent,
    Acquiring,
    Acquired,
    Lost,
}

fn find_program(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|directory| directory.join(name))
        .find(|candidate| is_executable_file(candidate))
}

#[cfg(unix)]
fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable_file(path: &Path) -> bool {
    path.is_file()
}

fn sanitize_log_field(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .take(512)
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::thread;
    use std::time::Duration;

    use super::{
        LeasePoll, READY_RECEIPT, SleepInhibitorCapability, SystemdSleepInhibitor, run_lease_body,
        sanitize_log_field,
    };

    #[test]
    fn helper_acknowledges_then_consumes_the_private_pipe_to_eof() {
        let mut output = Vec::new();
        run_lease_body(&b"lease remains held"[..], &mut output).unwrap();
        assert_eq!(
            String::from_utf8(output).unwrap(),
            format!("{READY_RECEIPT}\n")
        );
    }

    #[test]
    fn diagnostics_are_single_line_and_bounded() {
        let unsafe_value = format!("before\nafter{}", "x".repeat(600));
        let sanitized = sanitize_log_field(&unsafe_value);
        assert!(!sanitized.contains('\n'));
        assert_eq!(sanitized.chars().count(), 512);
    }

    #[test]
    fn backend_early_exit_is_observed_and_clears_the_lease() {
        let mut inhibitor = SystemdSleepInhibitor::new(SleepInhibitorCapability {
            executable: Some(PathBuf::from("/bin/false")),
        });
        inhibitor.acquire().unwrap();
        let outcome = (0..100).find_map(|_| {
            let outcome = inhibitor.poll();
            if outcome == LeasePoll::Acquiring {
                thread::sleep(Duration::from_millis(1));
                None
            } else {
                Some(outcome)
            }
        });
        assert_eq!(outcome, Some(LeasePoll::Lost));
        assert_eq!(inhibitor.poll(), LeasePoll::Absent);
    }
}
