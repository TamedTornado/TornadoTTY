use serde_json::Value;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const MAX_TAIL_BYTES: u64 = 256 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexTranscriptQuestion {
    pub text: String,
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
    let text = String::from_utf8(bytes).ok()?;
    codex_question_from_transcript_text(&text)
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

fn string_at<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
}
