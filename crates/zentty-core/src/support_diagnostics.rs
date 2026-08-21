use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub const DIAGNOSTIC_SCHEMA_VERSION: u8 = 1;
pub const MAX_DIAGNOSTIC_DETAIL_BYTES: usize = 4096;
pub const MAX_DIAGNOSTIC_CONTEXT_FIELDS: usize = 16;
pub const MAX_DIAGNOSTIC_CONTEXT_VALUE_BYTES: usize = 256;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticState {
    Local,
    PendingReview,
    Sent,
    Failed,
    Cleared,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticReason {
    ManualSupport,
    ControlledCrash,
    Panic,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DiagnosticReport {
    pub schema_version: u8,
    pub report_id: String,
    pub created_at_epoch: u64,
    pub state: DiagnosticState,
    pub reason: DiagnosticReason,
    pub application_version: String,
    pub build_commit: String,
    pub platform: String,
    pub detail: String,
    pub context: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticDraft<'a> {
    pub report_id: &'a str,
    pub created_at_epoch: u64,
    pub reason: DiagnosticReason,
    pub application_version: &'a str,
    pub build_commit: &'a str,
    pub platform: &'a str,
    pub detail: &'a str,
    pub context: &'a BTreeMap<String, String>,
    pub home_directory: Option<&'a str>,
}

impl DiagnosticReport {
    #[must_use]
    pub fn from_draft(draft: &DiagnosticDraft<'_>) -> Self {
        let context = draft
            .context
            .iter()
            .filter(|(key, _)| allowed_context_key(key))
            .take(MAX_DIAGNOSTIC_CONTEXT_FIELDS)
            .map(|(key, value)| {
                (
                    key.clone(),
                    redact_text(
                        &truncate_utf8(value, MAX_DIAGNOSTIC_CONTEXT_VALUE_BYTES),
                        draft.home_directory,
                    ),
                )
            })
            .collect();
        Self {
            schema_version: DIAGNOSTIC_SCHEMA_VERSION,
            report_id: safe_identifier(draft.report_id),
            created_at_epoch: draft.created_at_epoch,
            state: DiagnosticState::Local,
            reason: draft.reason,
            application_version: safe_identifier(draft.application_version),
            build_commit: safe_identifier(draft.build_commit),
            platform: safe_identifier(draft.platform),
            detail: redact_text(
                &truncate_utf8(draft.detail, MAX_DIAGNOSTIC_DETAIL_BYTES),
                draft.home_directory,
            ),
            context,
        }
    }

    /// Moves a local report through the explicit review/submission lifecycle.
    ///
    /// # Errors
    ///
    /// Returns an error when a report would bypass review or leave a terminal
    /// state through an unsupported transition.
    pub fn transition(&mut self, next: DiagnosticState) -> Result<(), &'static str> {
        let valid = matches!(
            (self.state, next),
            (
                DiagnosticState::Local | DiagnosticState::Failed,
                DiagnosticState::PendingReview
            ) | (
                DiagnosticState::PendingReview,
                DiagnosticState::Local | DiagnosticState::Sent | DiagnosticState::Failed
            ) | (_, DiagnosticState::Cleared)
        );
        if !valid {
            return Err("invalid diagnostic state transition");
        }
        self.state = next;
        Ok(())
    }
}

#[must_use]
pub fn redact_text(value: &str, home_directory: Option<&str>) -> String {
    let mut redacted = value.replace(['\r', '\0'], " ");
    if let Some(home) = home_directory.filter(|home| !home.is_empty()) {
        redacted = redacted.replace(home, "$HOME");
    }
    redacted = redact_user_home_paths(&redacted);
    redacted = redact_url_queries(&redacted);
    redacted = redact_named_values(&redacted);
    redacted = redact_identity_tokens(&redacted);
    truncate_utf8(&redacted, MAX_DIAGNOSTIC_DETAIL_BYTES)
}

fn redact_user_home_paths(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut cursor = 0;
    while let Some(remainder) = value.get(cursor..) {
        if remainder.is_empty() {
            break;
        }
        let next = ["/home/", "/Users/"]
            .into_iter()
            .filter_map(|prefix| remainder.find(prefix).map(|offset| (offset, prefix)))
            .min_by_key(|(offset, _)| *offset);
        let Some((offset, prefix)) = next else {
            result.push_str(remainder);
            break;
        };
        let start = cursor + offset;
        let boundary_ok = start == 0
            || value[..start]
                .chars()
                .next_back()
                .is_some_and(|character| !character.is_ascii_alphanumeric());
        result.push_str(&value[cursor..start]);
        if !boundary_ok {
            result.push_str(prefix);
            cursor = start.saturating_add(prefix.len());
            continue;
        }
        let end = value[start..]
            .find(|character: char| {
                character.is_whitespace() || matches!(character, ',' | ';' | ')' | ']' | '}')
            })
            .map_or(value.len(), |relative| start.saturating_add(relative));
        result.push_str("$HOME/<redacted>");
        cursor = end;
    }
    result
}

fn allowed_context_key(key: &str) -> bool {
    matches!(
        key,
        "async_backend"
            | "desktop"
            | "display_backend"
            | "distribution"
            | "gtk_version"
            | "locale"
            | "operation"
            | "package_source"
            | "renderer"
            | "stage"
    )
}

fn redact_url_queries(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut characters = value.char_indices().peekable();
    while let Some((_, character)) = characters.next() {
        result.push(character);
        if character != '?' {
            continue;
        }
        while let Some((_, next)) = characters.peek() {
            if next.is_whitespace() || matches!(next, ')' | ']' | '}' | '"' | '\'') {
                break;
            }
            characters.next();
        }
        result.push_str("<redacted>");
    }
    result
}

fn redact_named_values(value: &str) -> String {
    const NAMES: &[&str] = &[
        "api_key",
        "apikey",
        "authorization",
        "cookie",
        "credential",
        "dsn",
        "password",
        "private_key",
        "secret",
        "token",
    ];
    let lower = value.to_ascii_lowercase();
    let mut ranges = Vec::new();
    for name in NAMES {
        let mut offset = 0;
        while let Some(relative) = lower[offset..].find(name) {
            let start = offset + relative;
            let before_ok = start == 0 || !lower.as_bytes()[start - 1].is_ascii_alphanumeric();
            let mut cursor = start.saturating_add(name.len());
            while cursor < value.len()
                && matches!(value.as_bytes()[cursor], b' ' | b'\t' | b':' | b'=')
            {
                cursor = cursor.saturating_add(1);
            }
            if before_ok && cursor > start + name.len() {
                let credential_scheme = if *name == "authorization" {
                    let remainder = value[cursor..].to_ascii_lowercase();
                    ["bearer ", "basic "]
                        .into_iter()
                        .find(|scheme| remainder.starts_with(scheme))
                } else {
                    None
                };
                let value_start =
                    credential_scheme.map_or(cursor, |scheme| cursor.saturating_add(scheme.len()));
                let end = value[value_start..]
                    .find(|character: char| {
                        character.is_whitespace()
                            || matches!(character, ',' | ';' | ')' | ']' | '}')
                    })
                    .map_or(value.len(), |relative| value_start.saturating_add(relative));
                ranges.push((cursor, end));
            }
            offset = start.saturating_add(name.len());
        }
    }
    replace_ranges(value, ranges)
}

fn redact_identity_tokens(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    for token in value.split_inclusive(char::is_whitespace) {
        let trimmed = token.trim_end_matches(char::is_whitespace);
        let suffix = &token[trimmed.len()..];
        let lower = trimmed.to_ascii_lowercase();
        let sensitive_prefix = ["pane-", "session-", "ipc-", "bearer "]
            .into_iter()
            .find(|prefix| lower.starts_with(prefix));
        if let Some(prefix) = sensitive_prefix {
            result.push_str(prefix.trim_end());
            result.push_str("<redacted>");
        } else if looks_like_uuid(trimmed) || looks_like_secret_blob(trimmed) {
            result.push_str("<redacted>");
        } else {
            result.push_str(trimmed);
        }
        result.push_str(suffix);
    }
    result
}

fn looks_like_uuid(value: &str) -> bool {
    value.len() == 36
        && value
            .chars()
            .enumerate()
            .all(|(index, character)| match index {
                8 | 13 | 18 | 23 => character == '-',
                _ => character.is_ascii_hexdigit(),
            })
}

fn looks_like_secret_blob(value: &str) -> bool {
    value.len() >= 32
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
        && value
            .chars()
            .any(|character| character.is_ascii_alphabetic())
        && value.chars().any(|character| character.is_ascii_digit())
}

fn replace_ranges(value: &str, mut ranges: Vec<(usize, usize)>) -> String {
    ranges.sort_unstable();
    let mut result = String::with_capacity(value.len());
    let mut cursor = 0;
    for (start, end) in ranges {
        if start < cursor || start > end || end > value.len() {
            continue;
        }
        result.push_str(&value[cursor..start]);
        result.push_str("<redacted>");
        cursor = end;
    }
    result.push_str(&value[cursor..]);
    result
}

fn safe_identifier(value: &str) -> String {
    let value = truncate_utf8(value.trim(), 80);
    if !value.is_empty()
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_')
        })
    {
        value
    } else {
        "unknown".to_owned()
    }
}

fn truncate_utf8(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_owned();
    }
    let end = value
        .char_indices()
        .map(|(index, _)| index)
        .take_while(|index| *index <= max_bytes)
        .last()
        .unwrap_or(0);
    value[..end].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft<'a>(detail: &'a str, context: &'a BTreeMap<String, String>) -> DiagnosticDraft<'a> {
        DiagnosticDraft {
            report_id: "report-1",
            created_at_epoch: 42,
            reason: DiagnosticReason::ManualSupport,
            application_version: "1.2.3",
            build_commit: "abc123",
            platform: "linux-x86_64",
            detail,
            context,
            home_directory: Some("/home/jason"),
        }
    }

    #[test]
    fn report_keeps_only_bounded_allowlisted_context() {
        let context = BTreeMap::from([
            ("display_backend".to_owned(), "wayland".to_owned()),
            ("command".to_owned(), "rm -rf /".to_owned()),
            ("terminal_output".to_owned(), "secret".to_owned()),
            ("agent_prompt".to_owned(), "private".to_owned()),
        ]);
        let report = DiagnosticReport::from_draft(&draft("render failed", &context));
        assert_eq!(
            report.context,
            BTreeMap::from([("display_backend".to_owned(), "wayland".to_owned())])
        );
        assert_eq!(report.state, DiagnosticState::Local);
    }

    #[test]
    fn redaction_covers_secret_paths_urls_credentials_and_identifiers() {
        let value = "path=/home/jason/private URL=https://example.test/a?token=raw API_KEY=hunter2 Authorization: Bearer abc123 pane-pane123 session-123 550e8400-e29b-41d4-a716-446655440000 abcdefghijklmnopqrstuvwxyz123456";
        let redacted = redact_text(value, Some("/home/jason"));
        for forbidden in [
            "/home/jason",
            "token=raw",
            "hunter2",
            "abc123",
            "pane123",
            "session-123",
            "550e8400-e29b-41d4-a716-446655440000",
            "abcdefghijklmnopqrstuvwxyz123456",
        ] {
            assert!(
                !redacted.contains(forbidden),
                "leaked {forbidden}: {redacted}"
            );
        }
        assert!(redacted.contains("$HOME/private"));
        assert!(redacted.contains("?<redacted>"));
    }

    #[test]
    fn redaction_removes_non_current_user_home_paths() {
        let redacted = redact_text(
            "source=/home/another-user/private crash=/Users/operator/Library/file",
            Some("/tmp/test-home"),
        );
        assert_eq!(redacted, "source=$HOME/<redacted> crash=$HOME/<redacted>");
    }

    #[test]
    fn redaction_helpers_preserve_boundaries_and_remove_every_secret_form() {
        assert_eq!(
            redact_user_home_paths("/home/alice/private x/home/bob/public /Users/carol/file"),
            "$HOME/<redacted> x/home/bob/public $HOME/<redacted>"
        );
        assert_eq!(
            redact_url_queries("https://one.test/a?q=secret) https://two.test/b?k=private rest"),
            "https://one.test/a?<redacted>) https://two.test/b?<redacted> rest"
        );
        assert_eq!(
            redact_named_values(
                "token=a, password=b; Authorization: Bearer c) mytoken=public token"
            ),
            "token=<redacted>, password=<redacted>; Authorization: <redacted>) mytoken=public token"
        );
    }

    #[test]
    fn identity_classifiers_reject_near_misses() {
        assert!(looks_like_uuid("550e8400-e29b-41d4-a716-446655440000"));
        for invalid in [
            "550e8400e29b-41d4-a716-446655440000",
            "550e8400-e29b-41d4-a716-44665544000",
            "550e8400-e29b-41d4-a716-44665544000z",
            "550e8400_e29b_41d4_a716_446655440000",
        ] {
            assert!(!looks_like_uuid(invalid), "accepted invalid UUID {invalid}");
        }
        assert!(looks_like_secret_blob("abcdefghijklmnopqrstuvwxyz123456"));
        for invalid in [
            "abcdefghijklmnopqrstuvwxyzabcdef",
            "12345678901234567890123456789012",
            "abcdefghijklmnopqrstuvwxyz12!456",
            "short-token-123",
        ] {
            assert!(
                !looks_like_secret_blob(invalid),
                "accepted invalid secret blob {invalid}"
            );
        }
    }

    #[test]
    fn range_replacement_rejects_invalid_and_overlapping_ranges() {
        assert_eq!(
            replace_ranges("abcdef", vec![(1, 3), (2, 4)]),
            "a<redacted>def"
        );
        assert_eq!(replace_ranges("abcdef", vec![(4, 2)]), "abcdef");
        assert_eq!(replace_ranges("abcdef", vec![(1, 9)]), "abcdef");
        assert_eq!(
            replace_ranges("abcdef", vec![(1, 2), (2, 3)]),
            "a<redacted><redacted>def"
        );
        assert_eq!(
            replace_ranges("abcdef", vec![(2, 2), (3, 6)]),
            "ab<redacted>c<redacted>"
        );
    }

    #[test]
    fn identifier_and_utf8_bounds_are_exact() {
        assert_eq!(safe_identifier(" abc.DEF-1_2 "), "abc.DEF-1_2");
        assert_eq!(safe_identifier(""), "unknown");
        assert_eq!(safe_identifier("contains space"), "unknown");
        assert_eq!(safe_identifier("slash/value"), "unknown");
        assert_eq!(truncate_utf8("abcé", 4), "abc");
        assert_eq!(truncate_utf8("abcé", 5), "abcé");
    }

    #[test]
    fn report_and_context_bounds_preserve_utf8_boundaries() {
        let context = BTreeMap::from([("stage".to_owned(), "é".repeat(300))]);
        let report = DiagnosticReport::from_draft(&draft(&"é".repeat(3000), &context));
        assert!(report.detail.len() <= MAX_DIAGNOSTIC_DETAIL_BYTES);
        assert!(report.context["stage"].len() <= MAX_DIAGNOSTIC_CONTEXT_VALUE_BYTES);
    }

    #[test]
    fn state_machine_requires_review_before_send_and_allows_clear() {
        let mut report = DiagnosticReport::from_draft(&draft("failure", &BTreeMap::new()));
        assert_eq!(
            report.transition(DiagnosticState::Sent),
            Err("invalid diagnostic state transition")
        );
        report.transition(DiagnosticState::PendingReview).unwrap();
        report.transition(DiagnosticState::Failed).unwrap();
        report.transition(DiagnosticState::PendingReview).unwrap();
        report.transition(DiagnosticState::Sent).unwrap();
        report.transition(DiagnosticState::Cleared).unwrap();
    }
}
