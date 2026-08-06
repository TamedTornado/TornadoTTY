use serde_json::Value;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

const MAX_TAIL_BYTES: u64 = 256 * 1024;
const MAX_TRANSCRIPT_CANDIDATES: usize = 12;
const MAX_SESSION_DAY_DIRECTORIES: usize = 4;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexTranscriptQuestion {
    pub text: String,
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
        question_from_tool_input(input).map(|text| CodexTranscriptQuestion { text })
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
    let normalized_working_directory = normalize_path(Path::new(working_directory));
    let sessions = codex_home.join("sessions");
    let mut day_directories = directory_children(&sessions)
        .into_iter()
        .flat_map(|year| directory_children(&year))
        .flat_map(|month| directory_children(&month))
        .collect::<Vec<_>>();
    day_directories.sort_by(|left, right| right.cmp(left));
    day_directories.truncate(MAX_SESSION_DAY_DIRECTORIES);

    let mut candidates = day_directories
        .into_iter()
        .flat_map(|day| regular_jsonl_children(&day))
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| right.0.cmp(&left.0)));
    candidates.truncate(MAX_TRANSCRIPT_CANDIDATES);

    candidates.into_iter().find_map(|(path, _)| {
        let text = read_transcript_tail(&path)?;
        if !transcript_matches_working_directory(&text, &normalized_working_directory)
            || codex_question_from_transcript_text(&text).is_none()
        {
            return None;
        }
        Some(path)
    })
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
    let start = length.saturating_sub(MAX_TAIL_BYTES);
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
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

fn directory_children(path: &Path) -> Vec<PathBuf> {
    let Ok(children) = fs::read_dir(path) else {
        return Vec::new();
    };
    children
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_dir())
        })
        .collect()
}

fn regular_jsonl_children(path: &Path) -> Vec<(PathBuf, Option<SystemTime>)> {
    let Ok(children) = fs::read_dir(path) else {
        return Vec::new();
    };
    children
        .filter_map(Result::ok)
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
        .collect()
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
