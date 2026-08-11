use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk::{gio, glib};
use zentty_core::{
    ChecksState, ProjectContext, ProjectIconCache, ProjectIconLookup, PullRequestState,
    SystemProjectContextResolver,
};

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
    icon_cache: ProjectIconCache,
    icon_invalidations: BTreeSet<PathBuf>,
    pub(super) icons: BTreeMap<String, PathBuf>,
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
        let mut shell_ref = shell.borrow_mut();
        let icon_root = shell_ref
            .project_context_runtime
            .contexts
            .get(&pane_id)
            .map(|context| context.repository_root.clone())
            .or_else(|| {
                shell_ref
                    .state
                    .pane(&pane_id)
                    .and_then(|pane| pane.working_directory.as_deref())
                    .map(PathBuf::from)
            });
        if let Some(icon_root) = icon_root {
            if shell_ref.project_context_runtime.probe_in_flight {
                shell_ref
                    .project_context_runtime
                    .icon_invalidations
                    .insert(icon_root.clone());
            }
            shell_ref
                .project_context_runtime
                .icon_cache
                .invalidate(&icon_root);
        }
        shell_ref
            .project_context_runtime
            .force_panes
            .insert(pane_id);
        drop(shell_ref);
        request_probe(shell);
    }
}

pub(super) fn mark_pane_for_refresh(shell: &mut ApplicationShell, pane_id: &str) {
    shell
        .project_context_runtime
        .force_panes
        .insert(pane_id.to_owned());
}

pub(super) fn forget_pane(shell: &mut ApplicationShell, pane_id: &str) {
    let runtime = &mut shell.project_context_runtime;
    runtime.force_panes.remove(pane_id);
    runtime.last_refresh.remove(pane_id);
    runtime.contexts.remove(pane_id);
    runtime.icons.remove(pane_id);
    if runtime.last_active_pane.as_deref() == Some(pane_id) {
        runtime.last_active_pane = None;
    }
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

    let icon_cache = shell.borrow().project_context_runtime.icon_cache.clone();
    let weak = Rc::downgrade(shell);
    glib::spawn_future_local(async move {
        let result = gio::spawn_blocking(move || probe(sources, icon_cache)).await;
        let Some(shell) = weak.upgrade() else {
            return;
        };
        let mut shell = shell.borrow_mut();
        shell.project_context_runtime.probe_in_flight = false;
        if shell.shutting_down {
            return;
        }
        match result {
            Ok((icon_cache, results)) => {
                let icon_cache = apply_icon_invalidations(
                    icon_cache,
                    std::mem::take(&mut shell.project_context_runtime.icon_invalidations),
                );
                shell.project_context_runtime.icon_cache = icon_cache;
                apply_results(&mut shell, results);
            }
            Err(_) => eprintln!("zentty-linux: project-context error=worker-panic"),
        }
    });
}

fn apply_icon_invalidations(
    mut cache: ProjectIconCache,
    invalidations: BTreeSet<PathBuf>,
) -> ProjectIconCache {
    for root in invalidations {
        cache.invalidate(&root);
    }
    cache
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

struct ProbeResult {
    source: ProbeSource,
    context: Result<Option<ProjectContext>, String>,
    icon: Result<ProjectIconLookup, String>,
}

fn probe(
    sources: Vec<ProbeSource>,
    mut icon_cache: ProjectIconCache,
) -> (ProjectIconCache, Vec<ProbeResult>) {
    let resolver = SystemProjectContextResolver::default();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    let results = sources
        .into_iter()
        .map(|source| {
            let context = resolver
                .resolve(&source.working_directory)
                .map_err(|error| error.to_string());
            let icon_root = context
                .as_ref()
                .ok()
                .and_then(|context| context.as_ref())
                .map_or(source.working_directory.as_path(), |context| {
                    context.repository_root.as_path()
                });
            let icon = icon_cache.resolve_at(icon_root, now);
            ProbeResult {
                source,
                context,
                icon,
            }
        })
        .collect();
    (icon_cache, results)
}

fn apply_results(shell: &mut ApplicationShell, results: Vec<ProbeResult>) {
    let now = Instant::now();
    let mut changed = false;
    for ProbeResult {
        source,
        context,
        icon,
    } in results
    {
        if !observation_is_current(shell, &source) {
            shell
                .project_context_runtime
                .force_panes
                .insert(source.pane_id);
            continue;
        }
        shell
            .project_context_runtime
            .last_refresh
            .insert(source.pane_id.clone(), now);
        changed |= apply_icon_result(shell, &source.pane_id, icon);
        changed |= apply_context_result(shell, &source, context);
    }
    if changed {
        shell.refresh_project_context_presentation();
    }
}

fn apply_icon_result(
    shell: &mut ApplicationShell,
    pane_id: &str,
    icon: Result<ProjectIconLookup, String>,
) -> bool {
    match icon {
        Ok(ProjectIconLookup::Hit(path)) => {
            let changed = shell.project_context_runtime.icons.get(pane_id) != Some(&path);
            eprintln!(
                "zentty-linux: project-icon pane={pane_id} path={} result=resolved",
                path.display()
            );
            shell
                .project_context_runtime
                .icons
                .insert(pane_id.to_owned(), path);
            changed
        }
        Ok(ProjectIconLookup::Miss) => {
            let changed = shell
                .project_context_runtime
                .icons
                .remove(pane_id)
                .is_some();
            eprintln!("zentty-linux: project-icon pane={pane_id} result=missing");
            changed
        }
        Err(error) => {
            eprintln!("zentty-linux: project-icon pane={pane_id} error={error}");
            false
        }
    }
}

fn apply_context_result(
    shell: &mut ApplicationShell,
    source: &ProbeSource,
    context: Result<Option<ProjectContext>, String>,
) -> bool {
    match context {
        Ok(Some(mut context)) => {
            if let Some(previous) = shell.project_context_runtime.contexts.get(&source.pane_id) {
                preserve_review_on_refresh_failure(previous, &mut context);
            }
            let changed =
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
                .insert(source.pane_id.clone(), context);
            changed
        }
        Ok(None) => {
            let changed = shell
                .project_context_runtime
                .contexts
                .remove(&source.pane_id)
                .is_some();
            eprintln!(
                "zentty-linux: project-context pane={} repository=none",
                source.pane_id
            );
            changed
        }
        Err(error) => {
            eprintln!(
                "zentty-linux: project-context pane={} error={error}",
                source.pane_id
            );
            false
        }
    }
}

fn observation_is_current(shell: &ApplicationShell, source: &ProbeSource) -> bool {
    let Some(pane) = shell.state.pane(&source.pane_id) else {
        return false;
    };
    let current = shell
        .pane_runtime
        .surface(&pane.id)
        .and_then(zentty_ghostty::GhosttySurface::foreground_process_id)
        .and_then(|pid| process_working_directory(pid).ok())
        .or_else(|| pane.working_directory.as_deref().map(PathBuf::from));
    current.is_some_and(|current| canonical_directories_match(&current, &source.working_directory))
}

fn canonical_directories_match(current: &Path, observed: &Path) -> bool {
    matches!(
        (
            std::fs::canonicalize(current),
            std::fs::canonicalize(observed)
        ),
        (Ok(current), Ok(observed)) if current == observed
    )
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
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    use zentty_core::{
        ChecksState, GitReference, ProjectContext, ProjectIconCache, ProjectIconLookup,
        PullRequestState, PullRequestSummary, ReviewContext,
    };

    use super::{
        adaptive_refresh_interval, apply_icon_invalidations, canonical_directories_match,
        preserve_review_on_refresh_failure, safe_http_url,
    };

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(1);

    #[test]
    fn invalidation_during_a_probe_is_applied_to_the_returned_cache() {
        let root = std::env::temp_dir().join(format!(
            "zentty-project-cache-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).expect("project directory");
        let mut worker_cache = ProjectIconCache::default();
        assert_eq!(
            worker_cache.resolve_at(&root, 0).expect("initial miss"),
            ProjectIconLookup::Miss
        );
        fs::write(root.join("favicon.svg"), b"<svg></svg>").expect("new icon");
        let mut cache = apply_icon_invalidations(worker_cache, [root.clone()].into());
        assert!(matches!(
            cache.resolve_at(&root, 1),
            Ok(ProjectIconLookup::Hit(_))
        ));
        fs::remove_dir_all(&root).expect("remove fixture");
    }

    #[test]
    fn stale_directory_observations_require_two_live_canonical_paths() {
        let root = std::env::temp_dir().join(format!(
            "zentty-project-observation-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        let current = root.join("current");
        let other = root.join("other");
        fs::create_dir_all(&current).expect("current directory");
        fs::create_dir_all(&other).expect("other directory");
        assert!(canonical_directories_match(&current, &current));
        assert!(!canonical_directories_match(&current, &other));
        fs::remove_dir_all(&root).expect("remove fixture");
        assert!(!canonical_directories_match(&current, &current));
    }

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
