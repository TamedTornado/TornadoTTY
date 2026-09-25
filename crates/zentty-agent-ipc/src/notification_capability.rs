use std::io::Read;
use zentty_core::{
    NotificationCapability, codex_notification_capability, notification_launch_digest,
};

pub(crate) const LAUNCH_DIGEST_ENV: &str = "ZENTTY_CODEX_NOTIFICATION_ARGV_SHA256";

fn bounded_read(path: &std::path::Path) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(65_537)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() <= 65_536).then_some(bytes)
}

/// Confirms a managed launch declaration against a live native Codex ancestor,
/// not merely an inherited agent name or environment flag. Node launchers are
/// supported when their native child forwards the exact planned arguments.
/// Unreadable, changed, legacy and externally started processes fail closed.
#[must_use]
pub fn verified_codex_notification_capability() -> NotificationCapability {
    let Ok(digest) = std::env::var(LAUNCH_DIGEST_ENV) else {
        return NotificationCapability::Unknown;
    };
    verify_from_proc(std::path::Path::new("/proc"), std::process::id(), &digest)
}

fn verify_from_proc(
    proc_root: &std::path::Path,
    mut pid: u32,
    digest: &str,
) -> NotificationCapability {
    for _ in 0..16 {
        let root = proc_root.join(pid.to_string());
        let Some(stat) = bounded_read(&root.join("stat")) else {
            break;
        };
        let Ok(stat) = std::str::from_utf8(&stat) else {
            break;
        };
        let Some(parent) = stat
            .rsplit_once(')')
            .and_then(|(_, tail)| tail.split_whitespace().nth(1))
            .and_then(|value| value.parse::<u32>().ok())
        else {
            break;
        };
        let Ok(executable) = std::fs::read_link(root.join("exe")) else {
            return NotificationCapability::Unknown;
        };
        let Some(name) = executable.file_name() else {
            return NotificationCapability::Unknown;
        };
        if name == "codex (deleted)" {
            return NotificationCapability::Unknown;
        }
        if name == "codex" {
            let Some(bytes) = bounded_read(&root.join("cmdline")) else {
                return NotificationCapability::Unknown;
            };
            let Ok(text) = std::str::from_utf8(&bytes) else {
                return NotificationCapability::Unknown;
            };
            let arguments = text
                .trim_end_matches('\0')
                .split('\0')
                .skip(1)
                .map(str::to_owned)
                .collect::<Vec<_>>();
            if notification_launch_digest(&arguments) == digest {
                return codex_notification_capability(&arguments);
            }
            // Do not borrow an outer session's proof for a nested Codex.
            return NotificationCapability::Unknown;
        }
        if parent == 0 || parent == pid {
            break;
        }
        pid = parent;
    }
    NotificationCapability::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_argument_proof_rejects_changed_or_unreadable_nested_producers() {
        let root =
            std::env::temp_dir().join(format!("tornadotty-capability-proc-{}", std::process::id()));
        std::fs::create_dir_all(root.join("100")).unwrap();
        std::fs::create_dir_all(root.join("99")).unwrap();
        std::fs::write(root.join("100/stat"), "100 (hook) S 99").unwrap();
        std::fs::write(root.join("99/stat"), "99 (codex) S 0").unwrap();
        std::os::unix::fs::symlink("/bin/bash", root.join("100/exe")).unwrap();
        std::os::unix::fs::symlink("/reviewed/codex", root.join("99/exe")).unwrap();
        let args: Vec<String> = [
            "-c",
            "tui.notification_method=osc9",
            "-c",
            "tui.notifications=['approval-requested','agent-turn-complete']",
            "-c",
            "tui.notification_condition='always'",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        std::fs::write(
            root.join("99/cmdline"),
            format!("codex\0{}\0", args.join("\0")),
        )
        .unwrap();
        let digest = notification_launch_digest(&args);
        assert_eq!(
            verify_from_proc(&root, 100, &digest),
            NotificationCapability::CodexTuiAttentionV1
        );
        assert_eq!(
            verify_from_proc(&root, 100, "stale"),
            NotificationCapability::Unknown
        );
        std::fs::remove_file(root.join("100/exe")).unwrap();
        std::os::unix::fs::symlink("/nested/codex", root.join("100/exe")).unwrap();
        assert_eq!(
            verify_from_proc(&root, 100, &digest),
            NotificationCapability::Unknown,
            "unreadable nested process must not borrow parent proof"
        );
        std::fs::write(root.join("100/cmdline"), vec![b'x'; 65_537]).unwrap();
        assert_eq!(
            verify_from_proc(&root, 100, &digest),
            NotificationCapability::Unknown
        );
        std::fs::write(
            root.join("100/cmdline"),
            b"codex\0--config=tui.notifications=true\0",
        )
        .unwrap();
        assert_eq!(
            verify_from_proc(&root, 100, &digest),
            NotificationCapability::Unknown
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
