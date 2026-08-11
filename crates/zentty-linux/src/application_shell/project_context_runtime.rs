use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk::{gio, glib};
use zentty_core::{ChecksState, ProjectContext, PullRequestState, SystemProjectContextResolver};

use super::ApplicationShell;

const PROBE_INTERVAL: Duration = Duration::from_secs(2);
const BACKGROUND_REFRESH_INTERVAL: Duration = Duration::from_secs(90);
const MAX_PANES_PER_PROBE: usize = 24;

#[derive(Default)]
pub(super) struct ProjectContextRuntime {
    pub(super) probe_source: Option<glib::SourceId>,
    probe_in_flight: bool,
    force_panes: BTreeSet<String>,
    last_active_pane: Option<String>,
    last_refresh: BTreeMap<String, Instant>,
    pub(super) contexts: BTreeMap<String, ProjectContext>,
}

#[derive(Clone, Debug)]
struct ProbeSource {
    pane_id: String,
    working_directory: PathBuf,
    active: bool,
    forced: bool,
}

pub(super) fn install(shell: &Rc<RefCell<ApplicationShell>>) -> glib::SourceId {
    let weak = Rc::downgrade(shell);
    glib::timeout_add_local(PROBE_INTERVAL, move || {
        let Some(shell) = weak.upgrade() else {
            return glib::ControlFlow::Break;
        };
        if shell.borrow().shutting_down {
            return glib::ControlFlow::Break;
        }
        request_probe(&shell);
        glib::ControlFlow::Continue
    })
}

pub(super) fn refresh_focused(shell: &Rc<RefCell<ApplicationShell>>) {
    let pane_id = shell.borrow().state.focused_pane_id().map(str::to_owned);
    if let Some(pane_id) = pane_id {
        shell
            .borrow_mut()
            .project_context_runtime
            .force_panes
            .insert(pane_id);
        request_probe(shell);
    }
}

pub(super) fn mark_pane_for_refresh(shell: &mut ApplicationShell, pane_id: &str) {
    shell
        .project_context_runtime
        .force_panes
        .insert(pane_id.to_owned());
}

pub(super) fn open_focused_branch(shell: &Rc<RefCell<ApplicationShell>>) {
    let url = focused_context(shell).and_then(|context| {
        context
            .reference
            .branch()
            .and_then(|branch| context.remote.as_ref()?.branch_url(branch))
    });
    launch_validated_url(url.as_deref(), "branch");
}

pub(super) fn open_focused_pull_request(shell: &Rc<RefCell<ApplicationShell>>) {
    let url = focused_context(shell).and_then(|context| {
        context
            .review
            .as_ref()?
            .pull_request
            .url
            .as_deref()
            .map(str::to_owned)
    });
    launch_validated_url(url.as_deref(), "pull-request");
}

fn focused_context(shell: &Rc<RefCell<ApplicationShell>>) -> Option<ProjectContext> {
    let shell = shell.borrow();
    let pane_id = shell.state.focused_pane_id()?;
    shell.project_context_runtime.contexts.get(pane_id).cloned()
}

fn launch_validated_url(url: Option<&str>, kind: &'static str) {
    let Some(url) = url.filter(|url| safe_http_url(url)) else {
        eprintln!("zentty-linux: action=open-{kind} result=unavailable");
        return;
    };
    let url = url.to_owned();
    glib::spawn_future_local(async move {
        let launched = gio::spawn_blocking(move || {
            std::process::Command::new("xdg-open")
                .arg(&url)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .map(|_| url)
                .map_err(|error| error.to_string())
        })
        .await;
        match launched {
            Ok(Ok(url)) => eprintln!("zentty-linux: action=open-{kind} url={url} result=launched"),
            Ok(Err(error)) => eprintln!("zentty-linux: action=open-{kind} error={error}"),
            Err(_) => eprintln!("zentty-linux: action=open-{kind} error=worker-panic"),
        }
    });
}

fn safe_http_url(url: &str) -> bool {
    if url
        .bytes()
        .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
    {
        return false;
    }
    let Some(rest) = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
    else {
        return false;
    };
    let authority = rest.split('/').next().unwrap_or_default();
    !authority.is_empty() && !authority.contains('@')
}

fn request_probe(shell: &Rc<RefCell<ApplicationShell>>) {
    let sources = {
        let mut shell = shell.borrow_mut();
        if shell.shutting_down || shell.project_context_runtime.probe_in_flight {
            return;
        }
        let active_worklane = shell.state.active_worklane_id().to_owned();
        let focused_pane = shell.state.focused_pane_id().map(str::to_owned);
        if shell.project_context_runtime.last_active_pane != focused_pane {
            if let Some(pane_id) = &focused_pane {
                shell
                    .project_context_runtime
                    .force_panes
                    .insert(pane_id.clone());
            }
            shell
                .project_context_runtime
                .last_active_pane
                .clone_from(&focused_pane);
        }
        let now = Instant::now();
        let mut sources = Vec::new();
        for worklane in shell.state.worklanes() {
            for pane in worklane.columns.iter().flat_map(|column| &column.panes) {
                if sources.len() >= MAX_PANES_PER_PROBE || pane.ssh_connection_label.is_some() {
                    continue;
                }
                let forced = shell.project_context_runtime.force_panes.contains(&pane.id);
                let active =
                    worklane.id == active_worklane && Some(&pane.id) == focused_pane.as_ref();
                let interval = if active {
                    shell
                        .project_context_runtime
                        .contexts
                        .get(&pane.id)
                        .map_or(Duration::ZERO, adaptive_refresh_interval)
                } else {
                    BACKGROUND_REFRESH_INTERVAL
                };
                if !forced
                    && shell
                        .project_context_runtime
                        .last_refresh
                        .get(&pane.id)
                        .is_some_and(|last| now.duration_since(*last) < interval)
                {
                    continue;
                }
                let working_directory = shell
                    .pane_runtime
                    .surface(&pane.id)
                    .and_then(zentty_ghostty::GhosttySurface::foreground_process_id)
                    .and_then(|pid| process_working_directory(pid).ok())
                    .or_else(|| pane.working_directory.as_deref().map(PathBuf::from))
                    .or_else(|| std::env::current_dir().ok());
                if let Some(working_directory) = working_directory {
                    sources.push(ProbeSource {
                        pane_id: pane.id.clone(),
                        working_directory,
                        active,
                        forced,
                    });
                }
            }
        }
        if sources.is_empty() {
            return;
        }
        for source in &sources {
            shell
                .project_context_runtime
                .force_panes
                .remove(&source.pane_id);
        }
        shell.project_context_runtime.probe_in_flight = true;
        sources
    };

    let weak = Rc::downgrade(shell);
    glib::spawn_future_local(async move {
        let result = gio::spawn_blocking(move || probe(sources)).await;
        let Some(shell) = weak.upgrade() else {
            return;
        };
        let mut shell = shell.borrow_mut();
        shell.project_context_runtime.probe_in_flight = false;
        if shell.shutting_down {
            return;
        }
        match result {
            Ok(results) => apply_results(&mut shell, results),
            Err(_) => eprintln!("zentty-linux: project-context error=worker-panic"),
        }
    });
}

fn adaptive_refresh_interval(context: &ProjectContext) -> Duration {
    let Some(review) = &context.review else {
        return Duration::from_secs(90);
    };
    match review.pull_request.state {
        PullRequestState::Merged | PullRequestState::Closed => Duration::from_mins(5),
        PullRequestState::Open | PullRequestState::Draft
            if review.checks_state == ChecksState::Running =>
        {
            Duration::from_secs(15)
        }
        PullRequestState::Open | PullRequestState::Draft => Duration::from_mins(1),
    }
}

fn process_working_directory(pid: u64) -> std::io::Result<PathBuf> {
    std::fs::read_link(Path::new("/proc").join(pid.to_string()).join("cwd"))
}

fn probe(sources: Vec<ProbeSource>) -> Vec<(ProbeSource, Result<Option<ProjectContext>, String>)> {
    let resolver = SystemProjectContextResolver::default();
    sources
        .into_iter()
        .map(|source| {
            let result = resolver
                .resolve(&source.working_directory)
                .map_err(|error| error.to_string());
            (source, result)
        })
        .collect()
}

fn apply_results(
    shell: &mut ApplicationShell,
    results: Vec<(ProbeSource, Result<Option<ProjectContext>, String>)>,
) {
    let now = Instant::now();
    let mut changed = false;
    for (source, result) in results {
        shell
            .project_context_runtime
            .last_refresh
            .insert(source.pane_id.clone(), now);
        match result {
            Ok(Some(mut context)) => {
                if let Some(previous) = shell.project_context_runtime.contexts.get(&source.pane_id)
                {
                    preserve_review_on_refresh_failure(previous, &mut context);
                }
                changed |=
                    shell.project_context_runtime.contexts.get(&source.pane_id) != Some(&context);
                eprintln!(
                    "zentty-linux: project-context pane={} root={} reference={} dirty={} pr={} refresh={} active={} forced={}",
                    source.pane_id,
                    context.repository_root.display(),
                    context.reference.display(),
                    context.dirty,
                    context.review.as_ref().map_or_else(
                        || "none".to_owned(),
                        |review| review.pull_request.number.to_string()
                    ),
                    if context.review_error.is_some() {
                        "stale-error"
                    } else {
                        "fresh"
                    },
                    source.active,
                    source.forced
                );
                shell
                    .project_context_runtime
                    .contexts
                    .insert(source.pane_id, context);
            }
            Ok(None) => {
                changed |= shell
                    .project_context_runtime
                    .contexts
                    .remove(&source.pane_id)
                    .is_some();
                eprintln!(
                    "zentty-linux: project-context pane={} repository=none",
                    source.pane_id
                );
            }
            Err(error) => {
                eprintln!(
                    "zentty-linux: project-context pane={} error={error}",
                    source.pane_id
                );
            }
        }
    }
    if changed {
        shell.refresh_project_context_presentation();
    }
}

fn preserve_review_on_refresh_failure(previous: &ProjectContext, next: &mut ProjectContext) {
    if next.review.is_none()
        && next.review_error.is_some()
        && previous.repository_root == next.repository_root
        && previous.reference == next.reference
    {
        next.review.clone_from(&previous.review);
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use zentty_core::{
        ChecksState, GitReference, ProjectContext, PullRequestState, PullRequestSummary,
        ReviewContext,
    };

    use super::{adaptive_refresh_interval, preserve_review_on_refresh_failure, safe_http_url};

    #[test]
    fn launcher_boundary_rejects_non_web_and_credential_urls() {
        assert!(safe_http_url("https://github.com/acme/rocket/tree/main"));
        assert!(safe_http_url("http://127.0.0.1:8080/acme/rocket"));
        assert!(!safe_http_url("file:///tmp/pwn"));
        assert!(!safe_http_url("https://token@github.com/acme/rocket"));
        assert!(!safe_http_url("https://github.com/a\nfile:///tmp/pwn"));
        assert!(!safe_http_url("https://github.com/a b"));
        assert!(!safe_http_url("https://github.com/a\u{0001}b"));
        assert!(!safe_http_url("https:///missing-authority"));
    }

    #[test]
    fn adaptive_refresh_matches_source_review_cadence() {
        let context = |state: Option<PullRequestState>, checks_state: ChecksState| ProjectContext {
            repository_root: PathBuf::from("/tmp/project"),
            reference: GitReference::Branch("main".to_owned()),
            dirty: false,
            remote_name: None,
            remote: None,
            review: state.map(|state| ReviewContext {
                pull_request: PullRequestSummary {
                    number: 1,
                    url: None,
                    state,
                },
                checks_state,
                chips: Vec::new(),
                fetched_at_unix_seconds: 0,
            }),
            review_error: None,
        };
        assert_eq!(
            adaptive_refresh_interval(&context(Some(PullRequestState::Open), ChecksState::Running)),
            Duration::from_secs(15)
        );
        assert_eq!(
            adaptive_refresh_interval(&context(Some(PullRequestState::Draft), ChecksState::Passed)),
            Duration::from_mins(1)
        );
        assert_eq!(
            adaptive_refresh_interval(&context(None, ChecksState::None)),
            Duration::from_secs(90)
        );
        assert_eq!(
            adaptive_refresh_interval(&context(Some(PullRequestState::Merged), ChecksState::None)),
            Duration::from_mins(5)
        );
    }

    #[test]
    fn failed_refresh_preserves_only_the_same_repository_and_reference() {
        let review = ReviewContext {
            pull_request: PullRequestSummary {
                number: 42,
                url: None,
                state: PullRequestState::Open,
            },
            checks_state: ChecksState::Passed,
            chips: Vec::new(),
            fetched_at_unix_seconds: 10,
        };
        let previous = ProjectContext {
            repository_root: PathBuf::from("/tmp/a"),
            reference: GitReference::Branch("main".to_owned()),
            dirty: false,
            remote_name: None,
            remote: None,
            review: Some(review.clone()),
            review_error: None,
        };
        let mut failed = previous.clone();
        failed.review = None;
        failed.review_error = Some("offline".to_owned());
        preserve_review_on_refresh_failure(&previous, &mut failed);
        assert_eq!(failed.review, Some(review));

        failed.review = None;
        failed.reference = GitReference::Branch("other".to_owned());
        preserve_review_on_refresh_failure(&previous, &mut failed);
        assert_eq!(failed.review, None);
    }
}
