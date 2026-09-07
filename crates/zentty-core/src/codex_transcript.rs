use crate::AgentInteractionKind;
use serde_json::Value;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

const MAX_TAIL_BYTES: u64 = 256 * 1024;
const MAX_TRANSCRIPT_CANDIDATES: usize = 12;
const MAX_SESSION_DAY_DIRECTORIES: usize = 4;
const MAX_DISCOVERY_ENTRIES: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CodexTranscriptDiscoveryLimit;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexTranscriptQuestion {
    pub text: String,
    pub interaction: AgentInteractionKind,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CodexTranscriptCacheKey {
    pub path: PathBuf,
    pub file_size: u64,
    pub modification_time: Option<SystemTime>,
}

/// Extracts the newest user-question function call from a Codex JSONL
/// transcript.
#[must_use]
pub fn codex_question_from_transcript_text(text: &str) -> Option<CodexTranscriptQuestion> {
    text.lines().rev().find_map(|line| {
        let object = serde_json::from_str::<Value>(line.trim()).ok()?;
        let payload = if object.get("type").and_then(Value::as_str) == Some("response_item") {
            object.get("payload")?
        } else {
            &object
        };
        if payload.get("type").and_then(Value::as_str) != Some("function_call")
            || !is_question_tool_name(payload.get("name").and_then(Value::as_str))
        {
            return None;
        }
        let arguments = payload.get("arguments")?;
        let parsed;
        let input = if let Some(encoded) = arguments.as_str() {
            parsed = serde_json::from_str::<Value>(encoded).ok()?;
            &parsed
        } else {
            arguments
        };
        question_from_tool_input(input).map(|text| CodexTranscriptQuestion {
            text,
            interaction: if has_question_options(input) {
                AgentInteractionKind::Decision
            } else {
                AgentInteractionKind::Question
            },
        })
    })
}

/// Reads at most the source-owned 256-KiB tail and extracts its newest Codex
/// question. Symlinks and non-regular files are rejected before opening.
#[must_use]
pub fn codex_question_from_transcript_path(path: &Path) -> Option<CodexTranscriptQuestion> {
    let text = read_transcript_tail(path)?;
    codex_question_from_transcript_text(&text)
}

/// Locates the newest bounded Codex transcript that belongs to `working_directory`
/// and currently contains a user question.
#[must_use]
pub fn locate_recent_codex_transcript_path(
    codex_home: &Path,
    working_directory: &str,
) -> Option<PathBuf> {
    discover_recent_codex_transcript_path(codex_home, working_directory)
        .ok()
        .flatten()
}

/// Searches at most 4096 directory entries across the entire traversal, before
/// metadata filtering. Exhaustion returns no partial choice: directory order
/// is unspecified and a partial result need not be the newest session.
///
/// # Errors
/// Returns `CodexTranscriptDiscoveryLimit` when the entry budget is exceeded.
pub fn discover_recent_codex_transcript_path(
    codex_home: &Path,
    working_directory: &str,
) -> Result<Option<PathBuf>, CodexTranscriptDiscoveryLimit> {
    let mut remaining = MAX_DISCOVERY_ENTRIES;
    let normalized_working_directory = normalize_path(Path::new(working_directory));
    let sessions = codex_home.join("sessions");
    let mut day_directories = Vec::new();
    for year in directory_children(&sessions, &mut remaining)? {
        for month in directory_children(&year, &mut remaining)? {
            for day in directory_children(&month, &mut remaining)? {
                day_directories.push(day);
                day_directories.sort_by(|left, right| right.cmp(left));
                day_directories.truncate(MAX_SESSION_DAY_DIRECTORIES);
            }
        }
    }
    day_directories.sort_by(|left, right| right.cmp(left));
    day_directories.truncate(MAX_SESSION_DAY_DIRECTORIES);

    let mut candidates = Vec::new();
    for day in day_directories {
        for candidate in regular_jsonl_children(&day, &mut remaining)? {
            candidates.push(candidate);
            candidates
                .sort_by(|left, right| right.1.cmp(&left.1).then_with(|| right.0.cmp(&left.0)));
            candidates.truncate(MAX_TRANSCRIPT_CANDIDATES);
        }
    }

    Ok(candidates.into_iter().find_map(|(path, _)| {
        let text = read_transcript_tail(&path)?;
        if !transcript_matches_working_directory(&text, &normalized_working_directory)
            || codex_question_from_transcript_text(&text).is_none()
        {
            return None;
        }
        Some(path)
    }))
}

/// Returns the source-compatible file identity used to invalidate cached
/// transcript questions. Symlinks and non-regular files are rejected.
#[must_use]
pub fn codex_transcript_cache_key(path: &Path) -> Option<CodexTranscriptCacheKey> {
    let metadata = fs::symlink_metadata(path).ok()?;
    if !metadata.file_type().is_file() {
        return None;
    }
    Some(CodexTranscriptCacheKey {
        path: normalize_path(path),
        file_size: metadata.len(),
        modification_time: metadata.modified().ok(),
    })
}

fn read_transcript_tail(path: &Path) -> Option<String> {
    let metadata = fs::symlink_metadata(path).ok()?;
    // symlink_metadata reports a symlink as its own non-regular file type, so
    // this single check rejects symlinks, directories, sockets, and devices.
    if !metadata.file_type().is_file() {
        return None;
    }
    let mut file = File::open(path).ok()?;
    let length = file.seek(SeekFrom::End(0)).ok()?;
    read_captured_tail(&mut file, length)
}

fn read_captured_tail(file: &mut File, length: u64) -> Option<String> {
    let start = length.saturating_sub(MAX_TAIL_BYTES);
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut bytes = Vec::new();
    file.take(length - start).read_to_end(&mut bytes).ok()?;
    if start > 0 {
        let newline = bytes.iter().position(|byte| *byte == b'\n')?;
        bytes.drain(..=newline);
    }
    String::from_utf8(bytes).ok()
}

pub(crate) fn question_from_tool_input(input: &Value) -> Option<String> {
    let first = input
        .get("questions")
        .and_then(Value::as_array)
        .and_then(|questions| questions.first())
        .unwrap_or(input);
    let mut lines = Vec::new();
    if let Some(question) = string_at(first, &["question", "header"]) {
        lines.push(question.to_owned());
    }
    let labels = first
        .get("options")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|option| string_at(option, &["label"]))
        .map(|label| format!("[{label}]"))
        .collect::<Vec<_>>();
    if !labels.is_empty() {
        lines.push(labels.join(" "));
    }
    (!lines.is_empty()).then(|| lines.join("\n"))
}

fn has_question_options(input: &Value) -> bool {
    let first = input
        .get("questions")
        .and_then(Value::as_array)
        .and_then(|questions| questions.first())
        .unwrap_or(input);
    first
        .get("options")
        .and_then(Value::as_array)
        .is_some_and(|options| !options.is_empty())
}

fn is_question_tool_name(value: Option<&str>) -> bool {
    let Some(value) = value else {
        return false;
    };
    matches!(
        value
            .chars()
            .filter(|character| character.is_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect::<String>()
            .as_str(),
        "requestuserinput" | "askuserquestion" | "askuserquestiontool"
    )
}

fn bounded_entries(
    path: &Path,
    remaining: &mut usize,
) -> Result<Vec<fs::DirEntry>, CodexTranscriptDiscoveryLimit> {
    let Ok(children) = fs::read_dir(path) else {
        return Ok(Vec::new());
    };
    let mut entries = Vec::new();
    for entry in children {
        *remaining = remaining
            .checked_sub(1)
            .ok_or(CodexTranscriptDiscoveryLimit)?;
        if let Ok(entry) = entry {
            entries.push(entry);
        }
    }
    Ok(entries)
}

fn directory_children(
    path: &Path,
    remaining: &mut usize,
) -> Result<Vec<PathBuf>, CodexTranscriptDiscoveryLimit> {
    Ok(bounded_entries(path, remaining)?
        .into_iter()
        .map(|entry| entry.path())
        .filter(|path| {
            fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_dir())
        })
        .collect())
}

fn regular_jsonl_children(
    path: &Path,
    remaining: &mut usize,
) -> Result<Vec<(PathBuf, Option<SystemTime>)>, CodexTranscriptDiscoveryLimit> {
    Ok(bounded_entries(path, remaining)?
        .into_iter()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "jsonl")
        })
        .filter_map(|path| {
            let metadata = fs::symlink_metadata(&path).ok()?;
            metadata
                .file_type()
                .is_file()
                .then(|| (path, metadata.modified().ok()))
        })
        .collect())
}

fn transcript_matches_working_directory(text: &str, working_directory: &Path) -> bool {
    text.lines().rev().find_map(|line| {
        let object = serde_json::from_str::<Value>(line.trim()).ok()?;
        let payload = object.get("payload").unwrap_or(&object);
        let cwd = string_at(
            payload,
            &[
                "cwd",
                "current_working_directory",
                "currentWorkingDirectory",
            ],
        )?;
        Some(normalize_path(Path::new(cwd)) == working_directory)
    }) == Some(true)
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if normalized.file_name().is_some() {
                    normalized.pop();
                }
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

fn string_at<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod bounded_read_tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn append_after_length_capture_does_not_expand_the_read() {
        let path = std::env::temp_dir().join(format!(
            "tornadotty-codex-tail-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        // Unlink immediately so assertions cannot leave a fixture on disk.
        fs::remove_file(path).unwrap();
        for prefix_size in [0, usize::try_from(MAX_TAIL_BYTES).unwrap()] {
            file.set_len(0).unwrap();
            file.rewind().unwrap();
            if prefix_size != 0 {
                file.write_all(&vec![b'x'; prefix_size]).unwrap();
                file.write_all(b"\n").unwrap();
            }
            file.write_all(b"captured\n").unwrap();
            let captured_length = file.stream_position().unwrap();
            file.write_all(b"appended after capture\n").unwrap();
            assert_eq!(
                read_captured_tail(&mut file, captured_length).as_deref(),
                Some("captured\n")
            );
        }
    }
}
