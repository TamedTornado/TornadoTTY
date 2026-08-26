use crate::{AgentInteractionKind, AgentProgress};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodexTitlePhase {
    Starting,
    Running,
    NeedsInput,
    Idle,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexTitleSignal {
    pub phase: CodexTitlePhase,
    pub subject: String,
    pub interaction: AgentInteractionKind,
    pub progress: Option<AgentProgress>,
    pub background_wait: bool,
}

/// Parses the source-defined realtime status encoded in Codex terminal titles.
#[must_use]
pub fn classify_codex_terminal_title(value: &str) -> Option<CodexTitleSignal> {
    let raw = trimmed(value)?;
    let (title, progress) = split_trailing_task_progress(raw)
        .map_or((raw, None), |(title, progress)| (title, Some(progress)));

    if let Some((subject, interaction)) = parse_thread_status(title) {
        return Some(CodexTitleSignal {
            phase: CodexTitlePhase::NeedsInput,
            subject,
            interaction,
            progress,
            background_wait: false,
        });
    }
    if let Some(subject) = parse_action_required(title) {
        return Some(CodexTitleSignal {
            phase: CodexTitlePhase::NeedsInput,
            subject,
            interaction: AgentInteractionKind::GenericInput,
            progress,
            background_wait: false,
        });
    }

    let first_word = title
        .chars()
        .take_while(|character| character.is_alphabetic())
        .collect::<String>();
    let mut phase = match first_word.to_ascii_lowercase().as_str() {
        "working" | "thinking" => CodexTitlePhase::Running,
        "starting" => CodexTitlePhase::Starting,
        "waiting" => CodexTitlePhase::NeedsInput,
        "ready" => CodexTitlePhase::Idle,
        _ => return None,
    };
    let subject = status_subject(title, first_word.len())?;
    let mut background_wait = false;
    let interaction = if first_word.eq_ignore_ascii_case("waiting") {
        if requires_human_input(title) {
            phase = CodexTitlePhase::NeedsInput;
            interaction_for_waiting_message(title).unwrap_or(AgentInteractionKind::GenericInput)
        } else {
            phase = CodexTitlePhase::Idle;
            background_wait = true;
            AgentInteractionKind::None
        }
    } else {
        AgentInteractionKind::None
    };
    Some(CodexTitleSignal {
        phase,
        subject: subject.to_lowercase(),
        interaction,
        progress,
        background_wait,
    })
}

/// Returns a stable title for the UI while preserving the source title's
/// wording. Codex animates its running title with Braille spinner frames; those
/// frames are process activity, not distinct pane identities or UI content.
#[must_use]
pub fn stable_codex_terminal_title(value: &str) -> Option<String> {
    classify_codex_terminal_title(value)?;
    let mut stable = String::with_capacity(value.len());
    let mut replaced_spinner = false;
    for character in value.chars() {
        if matches!(
            character,
            '⠋' | '⠙' | '⠹' | '⠸' | '⠼' | '⠴' | '⠦' | '⠧' | '⠇' | '⠏'
        ) {
            if !replaced_spinner {
                stable.push('·');
                replaced_spinner = true;
            }
        } else {
            stable.push(character);
        }
    }
    Some(stable)
}

fn split_trailing_task_progress(value: &str) -> Option<(&str, AgentProgress)> {
    let (title, counts) = value.rsplit_once(" | Tasks ")?;
    let title = trimmed(title)?;
    let (done, total) = counts.trim().split_once('/')?;
    let done = done.parse::<i64>().ok()?;
    let total = total.parse::<u64>().ok()?;
    if total == 0 {
        return None;
    }
    let done = if done <= 0 {
        0
    } else {
        u64::try_from(done).unwrap_or(u64::MAX).min(total)
    };
    Some((title, AgentProgress { done, total }))
}

fn parse_thread_status(value: &str) -> Option<(String, AgentInteractionKind)> {
    let words = value
        .split(|character: char| !character.is_alphabetic())
        .filter(|word| !word.is_empty())
        .take(3)
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    if words.len() != 3
        || !matches!(words[0].as_str(), "main" | "parent")
        || words[1] != "needs"
        || words[2] != "input"
    {
        return None;
    }
    Some((value.to_lowercase(), AgentInteractionKind::GenericInput))
}

fn parse_action_required(value: &str) -> Option<String> {
    let stripped = strip_title_badge(value);
    stripped
        .to_ascii_lowercase()
        .starts_with("action required")
        .then(|| stripped.to_lowercase())
}

fn strip_title_badge(value: &str) -> &str {
    let value = value.trim();
    let Some(remainder) = value.strip_prefix('[') else {
        return value;
    };
    let Some((badge, remainder)) = remainder.split_once(']') else {
        return value;
    };
    if !matches!(badge.trim(), "!" | ".") {
        return value;
    }
    remainder.trim()
}

fn status_subject(value: &str, first_word_bytes: usize) -> Option<&str> {
    let mut remainder = value.get(first_word_bytes..)?.trim();
    let first_token = remainder.split_whitespace().next()?;
    if !first_token.chars().any(char::is_alphanumeric) {
        remainder = remainder.get(first_token.len()..)?.trim();
    }
    trimmed(remainder)
}

fn interaction_for_waiting_message(value: &str) -> Option<AgentInteractionKind> {
    let normalized = value.to_ascii_lowercase();
    if normalized.contains("plan-mode-prompt") || normalized.contains("plan mode prompt") {
        return Some(AgentInteractionKind::Approval);
    }
    if normalized.contains("question requested") || normalized.contains("questions requested") {
        return Some(AgentInteractionKind::Decision);
    }
    if normalized.contains("log in") || normalized.contains("login") {
        return Some(AgentInteractionKind::Auth);
    }
    if [
        "action required",
        "approval-requested",
        "approval requested",
        "permission",
        "approve",
        "approval",
        "allow ",
        "grant access",
        "wants to edit",
    ]
    .iter()
    .any(|marker| normalized.contains(marker))
    {
        return Some(AgentInteractionKind::Approval);
    }
    if normalized.contains('?') {
        return Some(if has_decision_options(value) {
            AgentInteractionKind::Decision
        } else {
            AgentInteractionKind::Question
        });
    }
    requires_human_input(value).then_some(AgentInteractionKind::GenericInput)
}

fn requires_human_input(value: &str) -> bool {
    let normalized = value.to_ascii_lowercase();
    [
        "waiting for your input",
        "waiting for input",
        "needs your input",
        "needs input",
        "needs your attention",
        "action required",
        "input-requested",
        "input requested",
        "approval-requested",
        "approval requested",
        "question requested",
        "questions requested",
        "plan-mode-prompt",
        "plan mode prompt",
        "permission",
        "approve",
        "approval",
        "allow ",
        "wants to edit",
        "confirm",
        "select ",
        "choose ",
        "grant access",
        "press enter",
        "log in",
        "login",
    ]
    .iter()
    .any(|marker| normalized.contains(marker))
        || normalized.contains('?')
}

fn has_decision_options(value: &str) -> bool {
    (value.contains('[') && value.contains(']'))
        || value.lines().any(|line| {
            line.trim().split_once('.').is_some_and(|(prefix, rest)| {
                prefix.parse::<u64>().is_ok() && !rest.trim().is_empty()
            })
        })
}

fn trimmed(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}
