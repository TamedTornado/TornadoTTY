use std::collections::BTreeSet;
use std::fmt;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Deserialize;

const COMMAND_TIMEOUT: Duration = Duration::from_secs(8);
const MAX_COMMAND_OUTPUT: usize = 1_048_576;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GitReference {
    Branch(String),
    Detached(String),
}

impl GitReference {
    #[must_use]
    pub fn display(&self) -> String {
        match self {
            Self::Branch(branch) => branch.clone(),
            Self::Detached(commit) => format!("{commit} (detached)"),
        }
    }

    #[must_use]
    pub fn branch(&self) -> Option<&str> {
        match self {
            Self::Branch(branch) => Some(branch),
            Self::Detached(_) => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GitHostKind {
    GitHub,
    GitLab,
    Bitbucket,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitRemote {
    pub scheme: String,
    pub host: String,
    pub owner: String,
    pub repository: String,
    pub kind: GitHostKind,
}

impl GitRemote {
    #[must_use]
    pub fn repository_specifier(&self) -> String {
        format!("{}/{}", self.owner, self.repository)
    }

    #[must_use]
    pub fn branch_url(&self, branch: &str) -> Option<String> {
        let branch = encode_branch(branch)?;
        let middle = match self.kind {
            GitHostKind::GitLab => "/-/tree/",
            GitHostKind::Bitbucket => "/src/",
            GitHostKind::GitHub | GitHostKind::Unknown => "/tree/",
        };
        Some(format!(
            "{}://{}/{}/{}{}{}",
            self.scheme, self.host, self.owner, self.repository, middle, branch
        ))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PullRequestState {
    Draft,
    Open,
    Merged,
    Closed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChecksState {
    None,
    Running,
    Passed,
    Failing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewChipStyle {
    Neutral,
    Success,
    Warning,
    Danger,
    Info,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewChip {
    pub text: String,
    pub style: ReviewChipStyle,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PullRequestSummary {
    pub number: u64,
    pub url: Option<String>,
    pub state: PullRequestState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewContext {
    pub pull_request: PullRequestSummary,
    pub checks_state: ChecksState,
    pub chips: Vec<ReviewChip>,
    pub fetched_at_unix_seconds: u64,
}

impl ReviewContext {
    #[must_use]
    pub fn age_label(&self, now_unix_seconds: u64) -> String {
        let seconds = now_unix_seconds.saturating_sub(self.fetched_at_unix_seconds);
        match seconds {
            0..=44 => "just now".to_owned(),
            45..=89 => "1m ago".to_owned(),
            90..=3_599 => format!("{}m ago", (seconds + 30) / 60),
            3_600..=86_399 => format!("{}h ago", seconds / 3_600),
            _ => format!("{}d ago", seconds / 86_400),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectContext {
    pub repository_root: PathBuf,
    pub git_directory: PathBuf,
    pub reference: GitReference,
    pub dirty: bool,
    pub remote_name: Option<String>,
    pub remote: Option<GitRemote>,
    pub review: Option<ReviewContext>,
    pub review_error: Option<String>,
}

#[derive(Debug)]
pub enum ProjectContextError {
    Io(io::Error),
    TimedOut(String),
    OutputTooLarge(String),
    InvalidOutput(String),
}

impl fmt::Display for ProjectContextError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::TimedOut(command) => write!(formatter, "command timed out: {command}"),
            Self::OutputTooLarge(command) => {
                write!(formatter, "command output exceeded limit: {command}")
            }
            Self::InvalidOutput(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for ProjectContextError {}

impl From<io::Error> for ProjectContextError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Clone, Debug)]
pub struct SystemProjectContextResolver {
    search_path: String,
}

impl Default for SystemProjectContextResolver {
    fn default() -> Self {
        Self::with_search_path(
            std::env::var("PATH").unwrap_or_else(|_| "/usr/local/bin:/usr/bin:/bin".to_owned()),
        )
    }
}

impl SystemProjectContextResolver {
    #[must_use]
    pub fn with_search_path(search_path: String) -> Self {
        Self { search_path }
    }

    /// Resolves bounded Git and review state for one real working directory.
    ///
    /// # Errors
    ///
    /// Returns an error when a required Git command cannot be executed safely,
    /// exceeds its time/output bound, or reports malformed required output.
    pub fn resolve(
        &self,
        working_directory: &Path,
    ) -> Result<Option<ProjectContext>, ProjectContextError> {
        let root_result = self.run("git", &["rev-parse", "--show-toplevel"], working_directory)?;
        if !root_result.success {
            return Ok(None);
        }
        let raw_root = required_line(&root_result.stdout, "git returned an empty repository root")?;
        let repository_root = Path::new(raw_root)
            .canonicalize()
            .map_err(ProjectContextError::Io)?;
        let git_directory_result = self.run(
            "git",
            &["rev-parse", "--absolute-git-dir"],
            &repository_root,
        )?;
        if !git_directory_result.success {
            return Err(ProjectContextError::InvalidOutput(
                "git could not resolve its metadata directory".to_owned(),
            ));
        }
        let raw_git_directory = required_line(
            &git_directory_result.stdout,
            "git returned an empty metadata directory",
        )?;
        let git_directory = Path::new(raw_git_directory)
            .canonicalize()
            .map_err(ProjectContextError::Io)?;

        let branch_result = self.run(
            "git",
            &["symbolic-ref", "--quiet", "--short", "HEAD"],
            &repository_root,
        )?;
        let reference = if branch_result.success {
            GitReference::Branch(
                required_line(&branch_result.stdout, "git returned an empty branch")?.to_owned(),
            )
        } else {
            let commit = self.run("git", &["rev-parse", "--short=7", "HEAD"], &repository_root)?;
            if !commit.success {
                return Err(ProjectContextError::InvalidOutput(
                    "git could not resolve HEAD".to_owned(),
                ));
            }
            GitReference::Detached(
                required_line(&commit.stdout, "git returned an empty detached HEAD")?.to_owned(),
            )
        };

        let status = self.run(
            "git",
            &["status", "--porcelain=v1", "--untracked-files=normal"],
            &repository_root,
        )?;
        let dirty = status.success && !trimmed(&status.stdout).is_empty();
        let (remote_name, remote) = self.resolve_remote(&repository_root, &reference)?;

        let (review, review_error) =
            if let (Some(branch), Some(remote)) = (reference.branch(), remote.as_ref()) {
                if remote.kind == GitHostKind::GitHub {
                    self.resolve_review(&repository_root, branch, remote)
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            };

        Ok(Some(ProjectContext {
            repository_root,
            git_directory,
            reference,
            dirty,
            remote_name,
            remote,
            review,
            review_error,
        }))
    }

    fn resolve_remote(
        &self,
        root: &Path,
        reference: &GitReference,
    ) -> Result<(Option<String>, Option<GitRemote>), ProjectContextError> {
        let mut candidates = Vec::new();
        if let Some(branch) = reference.branch() {
            let key = format!("branch.{branch}.remote");
            let configured = self.run("git", &["config", "--get", &key], root)?;
            if configured.success
                && let Some(name) = optional_line(&configured.stdout)
            {
                candidates.push(name.to_owned());
            }
        }
        candidates.push("origin".to_owned());
        let list = self.run("git", &["remote"], root)?;
        if list.success {
            candidates.extend(trimmed(&list.stdout).lines().map(str::to_owned));
        }
        let mut seen = BTreeSet::new();
        for name in candidates {
            if name.is_empty() || name == "." || !seen.insert(name.clone()) {
                continue;
            }
            let result = self.run("git", &["remote", "get-url", &name], root)?;
            if !result.success {
                continue;
            }
            let Some(value) = optional_line(&result.stdout) else {
                continue;
            };
            if let Some(remote) = parse_git_remote(value) {
                return Ok((Some(name), Some(remote)));
            }
        }
        Ok((None, None))
    }

    fn resolve_review(
        &self,
        root: &Path,
        branch: &str,
        remote: &GitRemote,
    ) -> (Option<ReviewContext>, Option<String>) {
        let repository = remote.repository_specifier();
        let result = match self.run(
            "gh",
            &[
                "pr",
                "view",
                branch,
                "--repo",
                &repository,
                "--json",
                "number,url,isDraft,state,reviewDecision,mergeable,statusCheckRollup",
            ],
            root,
        ) {
            Ok(result) => result,
            Err(error) => return (None, Some(error.to_string())),
        };
        if !result.success {
            let combined = format!("{}\n{}", trimmed(&result.stdout), trimmed(&result.stderr));
            if combined
                .to_ascii_lowercase()
                .contains("no pull requests found")
            {
                return (None, None);
            }
            return (
                None,
                Some(
                    first_line(&combined)
                        .unwrap_or("gh pr view failed")
                        .to_owned(),
                ),
            );
        }
        match parse_review(&result.stdout) {
            Ok(review) => (Some(review), None),
            Err(error) => (None, Some(error)),
        }
    }

    fn run(
        &self,
        executable: &str,
        arguments: &[&str],
        directory: &Path,
    ) -> Result<CommandResult, ProjectContextError> {
        let executable = resolve_executable(executable, &self.search_path).ok_or_else(|| {
            ProjectContextError::InvalidOutput(format!("unable to locate {executable}"))
        })?;
        run_bounded(&executable, arguments, directory)
    }
}

#[derive(Debug)]
struct CommandResult {
    success: bool,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

fn run_bounded(
    executable: &Path,
    arguments: &[&str],
    directory: &Path,
) -> Result<CommandResult, ProjectContextError> {
    let command_label = format!("{} {}", executable.display(), arguments.join(" "));
    let mut child = Command::new(executable)
        .args(arguments)
        .current_dir(directory)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");
    let stdout_reader = thread::spawn(move || read_bounded(stdout));
    let stderr_reader = thread::spawn(move || read_bounded(stderr));
    let deadline = Instant::now() + COMMAND_TIMEOUT;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            return Err(ProjectContextError::TimedOut(command_label));
        }
        thread::sleep(Duration::from_millis(10));
    };
    let stdout = stdout_reader
        .join()
        .map_err(|_| ProjectContextError::InvalidOutput("stdout reader panicked".to_owned()))??;
    let stderr = stderr_reader
        .join()
        .map_err(|_| ProjectContextError::InvalidOutput("stderr reader panicked".to_owned()))??;
    if stdout.1 || stderr.1 {
        return Err(ProjectContextError::OutputTooLarge(command_label));
    }
    Ok(CommandResult {
        success: status.success(),
        stdout: stdout.0,
        stderr: stderr.0,
    })
}

fn read_bounded(mut input: impl Read) -> io::Result<(Vec<u8>, bool)> {
    let mut result = Vec::new();
    let mut buffer = [0_u8; 8192];
    let mut exceeded = false;
    loop {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        let remaining = MAX_COMMAND_OUTPUT.saturating_sub(result.len());
        result.extend_from_slice(&buffer[..count.min(remaining)]);
        exceeded |= count > remaining;
    }
    Ok((result, exceeded))
}

fn resolve_executable(name: &str, search_path: &str) -> Option<PathBuf> {
    if name.contains('/') {
        let path = PathBuf::from(name);
        return path.is_file().then_some(path);
    }
    search_path
        .split(':')
        .filter(|entry| !entry.is_empty())
        .map(|entry| Path::new(entry).join(name))
        .find(|candidate| candidate.is_file())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PullRequestPayload {
    number: u64,
    url: Option<String>,
    is_draft: bool,
    state: String,
    review_decision: Option<String>,
    mergeable: Option<String>,
    status_check_rollup: Option<Vec<CheckPayload>>,
}

#[derive(Deserialize)]
struct CheckPayload {
    state: Option<String>,
    status: Option<String>,
    conclusion: Option<String>,
}

fn parse_review(bytes: &[u8]) -> Result<ReviewContext, String> {
    let payload: PullRequestPayload = serde_json::from_slice(bytes)
        .map_err(|error| format!("could not decode gh pr view output: {error}"))?;
    let state = match payload.state.to_ascii_uppercase().as_str() {
        "MERGED" => PullRequestState::Merged,
        "CLOSED" => PullRequestState::Closed,
        _ if payload.is_draft => PullRequestState::Draft,
        _ => PullRequestState::Open,
    };
    let checks_state = aggregate_checks(payload.status_check_rollup.as_deref());
    let mut chips = match state {
        PullRequestState::Draft => vec![chip("Draft", ReviewChipStyle::Info)],
        PullRequestState::Merged => vec![chip("Merged", ReviewChipStyle::Success)],
        PullRequestState::Closed => vec![chip("Closed", ReviewChipStyle::Neutral)],
        PullRequestState::Open => Vec::new(),
    };
    if state == PullRequestState::Open {
        match payload
            .review_decision
            .as_deref()
            .map(str::to_ascii_uppercase)
            .as_deref()
        {
            Some("APPROVED") => chips.push(chip("Approved", ReviewChipStyle::Success)),
            Some("CHANGES_REQUESTED") => {
                chips.push(chip("Changes requested", ReviewChipStyle::Danger));
            }
            Some("REVIEW_REQUIRED") => {
                chips.push(chip("Review required", ReviewChipStyle::Warning));
            }
            _ => {}
        }
    }
    if matches!(state, PullRequestState::Open | PullRequestState::Draft) {
        match checks_state {
            ChecksState::Failing => {
                let failures = payload
                    .status_check_rollup
                    .as_deref()
                    .unwrap_or_default()
                    .iter()
                    .filter(|check| check_failing(check))
                    .count();
                chips.push(chip(
                    if failures == 1 {
                        "1 failing".to_owned()
                    } else {
                        format!("{failures} failing")
                    },
                    ReviewChipStyle::Danger,
                ));
            }
            ChecksState::Running => chips.push(chip("Running", ReviewChipStyle::Warning)),
            ChecksState::Passed => chips.push(chip("Checks passed", ReviewChipStyle::Success)),
            ChecksState::None if state == PullRequestState::Open => {
                chips.push(chip("Ready", ReviewChipStyle::Success));
            }
            ChecksState::None => {}
        }
        if state == PullRequestState::Open
            && payload
                .mergeable
                .as_deref()
                .is_some_and(|value| value.eq_ignore_ascii_case("CONFLICTING"))
        {
            chips.push(chip("Conflicts", ReviewChipStyle::Danger));
        }
    }
    let url = payload.url.filter(|url| is_safe_web_url(url));
    Ok(ReviewContext {
        pull_request: PullRequestSummary {
            number: payload.number,
            url,
            state,
        },
        checks_state,
        chips,
        fetched_at_unix_seconds: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs()),
    })
}

fn aggregate_checks(checks: Option<&[CheckPayload]>) -> ChecksState {
    let Some(checks) = checks.filter(|checks| !checks.is_empty()) else {
        return ChecksState::None;
    };
    if checks.iter().any(check_failing) {
        return ChecksState::Failing;
    }
    if checks.iter().any(check_pending) {
        return ChecksState::Running;
    }
    ChecksState::Passed
}

fn check_failing(check: &CheckPayload) -> bool {
    check.conclusion.as_deref().is_some_and(|value| {
        matches!(
            value.to_ascii_uppercase().as_str(),
            "FAILURE" | "TIMED_OUT" | "CANCELLED" | "ACTION_REQUIRED" | "STARTUP_FAILURE" | "STALE"
        )
    }) || check
        .state
        .as_deref()
        .is_some_and(|value| matches!(value.to_ascii_uppercase().as_str(), "FAILURE" | "ERROR"))
}

fn check_pending(check: &CheckPayload) -> bool {
    if let Some(status) = &check.status {
        return !status.eq_ignore_ascii_case("COMPLETED");
    }
    check
        .state
        .as_deref()
        .is_some_and(|value| matches!(value.to_ascii_uppercase().as_str(), "PENDING" | "EXPECTED"))
}

fn chip(text: impl Into<String>, style: ReviewChipStyle) -> ReviewChip {
    ReviewChip {
        text: text.into(),
        style,
    }
}

#[must_use]
pub fn parse_git_remote(value: &str) -> Option<GitRemote> {
    if value.is_empty()
        || value
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
        || value.contains(['?', '#'])
    {
        return None;
    }
    if let Some(rest) = value.strip_prefix("git@") {
        let (host, path) = rest.split_once(':')?;
        return build_remote("https", host, path);
    }
    if let Some(rest) = value.strip_prefix("ssh://git@") {
        let (authority, path) = rest.split_once('/')?;
        let host = if let Some((host, port)) = authority.split_once(':') {
            if port.is_empty() || !port.bytes().all(|byte| byte.is_ascii_digit()) {
                return None;
            }
            host
        } else {
            authority
        };
        return build_remote("https", host, path);
    }
    let (scheme, rest) = value.split_once("://")?;
    if !matches!(scheme, "http" | "https") {
        return None;
    }
    let (authority, path) = rest.split_once('/')?;
    if authority.is_empty() || authority.contains('@') {
        return None;
    }
    let host = authority.to_ascii_lowercase();
    if let Some((_, port)) = host.rsplit_once(':')
        && (port.is_empty() || !port.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return None;
    }
    build_remote(scheme, &host, path)
}

fn build_remote(scheme: &str, host: &str, path: &str) -> Option<GitRemote> {
    let host = host.to_ascii_lowercase();
    let host_without_port = host.split_once(':').map_or(host.as_str(), |(host, _)| host);
    if host_without_port.is_empty()
        || !host_without_port
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
    {
        return None;
    }
    let path = path.strip_suffix(".git").unwrap_or(path).trim_matches('/');
    let mut components = path.split('/');
    let owner = components.next()?;
    let repository = components.next()?;
    if components.next().is_some()
        || !valid_repository_component(owner)
        || !valid_repository_component(repository)
    {
        return None;
    }
    let kind = match host_without_port {
        "github.com" | "www.github.com" => GitHostKind::GitHub,
        "gitlab.com" | "www.gitlab.com" => GitHostKind::GitLab,
        "bitbucket.org" | "www.bitbucket.org" => GitHostKind::Bitbucket,
        _ => GitHostKind::Unknown,
    };
    Some(GitRemote {
        scheme: scheme.to_owned(),
        host,
        owner: owner.to_owned(),
        repository: repository.to_owned(),
        kind,
    })
}

fn valid_repository_component(value: &str) -> bool {
    !value.is_empty()
        && !matches!(value, "." | "..")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn encode_branch(branch: &str) -> Option<String> {
    if branch.is_empty()
        || branch.starts_with('/')
        || branch.ends_with('/')
        || branch.contains('\\')
        || branch.bytes().any(|byte| byte.is_ascii_control())
        || branch
            .split('/')
            .any(|part| matches!(part, "" | "." | ".."))
    {
        return None;
    }
    let mut encoded = String::new();
    for byte in branch.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/') {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write as _;
            write!(&mut encoded, "%{byte:02X}").expect("writing to a string cannot fail");
        }
    }
    Some(encoded)
}

fn is_safe_web_url(value: &str) -> bool {
    parse_web_authority(value).is_some()
}

fn parse_web_authority(value: &str) -> Option<&str> {
    if value
        .bytes()
        .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
    {
        return None;
    }
    let rest = value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"))?;
    let authority = rest.split('/').next()?;
    (!authority.is_empty() && !authority.contains('@')).then_some(authority)
}

fn trimmed(bytes: &[u8]) -> &str {
    std::str::from_utf8(bytes).unwrap_or_default().trim()
}

fn required_line<'a>(bytes: &'a [u8], error: &str) -> Result<&'a str, ProjectContextError> {
    optional_line(bytes).ok_or_else(|| ProjectContextError::InvalidOutput(error.to_owned()))
}

fn optional_line(bytes: &[u8]) -> Option<&str> {
    first_line(trimmed(bytes))
        .map(str::trim)
        .filter(|line| !line.is_empty())
}

fn first_line(value: &str) -> Option<&str> {
    value.lines().next()
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::{GitReference, ProjectContextError};

    #[test]
    fn reference_accessors_and_error_messages_preserve_user_visible_context() {
        let branch = GitReference::Branch("feature/review".to_owned());
        assert_eq!(branch.display(), "feature/review");
        assert_eq!(branch.branch(), Some("feature/review"));

        let detached = GitReference::Detached("abc1234".to_owned());
        assert_eq!(detached.display(), "abc1234 (detached)");
        assert_eq!(detached.branch(), None);

        assert_eq!(
            ProjectContextError::TimedOut("git status".to_owned()).to_string(),
            "command timed out: git status"
        );
        assert_eq!(
            ProjectContextError::OutputTooLarge("gh pr view".to_owned()).to_string(),
            "command output exceeded limit: gh pr view"
        );
        assert_eq!(
            ProjectContextError::InvalidOutput("bad reference".to_owned()).to_string(),
            "bad reference"
        );
        assert_eq!(
            ProjectContextError::Io(io::Error::new(io::ErrorKind::NotFound, "missing")).to_string(),
            "missing"
        );
    }

    use super::*;

    #[test]
    fn terminal_pull_request_states_ignore_non_applicable_review_details() {
        let review = parse_review(br#"{"number":7,"url":null,"isDraft":false,"state":"MERGED","reviewDecision":"CHANGES_REQUESTED","mergeable":"CONFLICTING","statusCheckRollup":[{"conclusion":"FAILURE"}]}"#).unwrap();
        assert_eq!(review.pull_request.state, PullRequestState::Merged);
        assert_eq!(review.chips, [chip("Merged", ReviewChipStyle::Success)]);
    }

    #[test]
    fn empty_and_successful_rollups_are_distinct() {
        assert_eq!(aggregate_checks(None), ChecksState::None);
        assert_eq!(aggregate_checks(Some(&[])), ChecksState::None);
        assert_eq!(
            aggregate_checks(Some(&[CheckPayload {
                state: None,
                status: Some("COMPLETED".to_owned()),
                conclusion: Some("SUCCESS".to_owned()),
            }])),
            ChecksState::Passed
        );
    }

    #[test]
    fn review_age_has_stable_human_boundaries_and_never_goes_negative() {
        let review = ReviewContext {
            pull_request: PullRequestSummary {
                number: 1,
                url: None,
                state: PullRequestState::Open,
            },
            checks_state: ChecksState::None,
            chips: Vec::new(),
            fetched_at_unix_seconds: 1_000,
        };
        assert_eq!(review.age_label(900), "just now");
        assert_eq!(review.age_label(1_044), "just now");
        assert_eq!(review.age_label(1_045), "1m ago");
        assert_eq!(review.age_label(1_149), "2m ago");
        assert_eq!(review.age_label(4_600), "1h ago");
        assert_eq!(review.age_label(87_400), "1d ago");
    }
}
