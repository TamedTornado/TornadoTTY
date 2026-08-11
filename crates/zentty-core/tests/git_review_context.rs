use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use zentty_core::{
    ChecksState, GitReference, PullRequestState, ReviewChipStyle, SystemProjectContextResolver,
    parse_git_remote,
};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

struct RepositoryFixture {
    root: PathBuf,
}

impl RepositoryFixture {
    fn new() -> Self {
        let serial = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "zentty-git-context-{}-{serial}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        git(&root, &["init", "-b", "main"]);
        git(&root, &["config", "user.email", "test@zentty.invalid"]);
        git(&root, &["config", "user.name", "Zentty Test"]);
        fs::write(root.join("README.md"), "fixture\n").unwrap();
        git(&root, &["add", "README.md"]);
        git(&root, &["commit", "-m", "fixture"]);
        Self { root }
    }
}

impl Drop for RepositoryFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn git(root: &Path, arguments: &[&str]) -> String {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {} failed: {}",
        arguments.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

#[test]
fn real_git_resolves_nested_root_branch_dirty_state_and_remote() {
    let fixture = RepositoryFixture::new();
    let nested = fixture.root.join("src/deep");
    fs::create_dir_all(&nested).unwrap();
    git(
        &fixture.root,
        &["remote", "add", "origin", "git@github.com:acme/rocket.git"],
    );
    fs::write(fixture.root.join("untracked.txt"), "dirty\n").unwrap();

    let context = SystemProjectContextResolver::default()
        .resolve(&nested)
        .unwrap()
        .unwrap();

    assert_eq!(
        context.repository_root,
        fixture.root.canonicalize().unwrap()
    );
    assert_eq!(context.reference, GitReference::Branch("main".to_owned()));
    assert!(context.dirty);
    assert_eq!(
        context.remote.as_ref().unwrap().branch_url("feature/a b"),
        Some("https://github.com/acme/rocket/tree/feature/a%20b".to_owned())
    );
}

#[test]
fn real_git_distinguishes_detached_head_and_worktree_root() {
    let fixture = RepositoryFixture::new();
    git(&fixture.root, &["checkout", "--detach"]);

    let context = SystemProjectContextResolver::default()
        .resolve(&fixture.root)
        .unwrap()
        .unwrap();
    let short_head = git(&fixture.root, &["rev-parse", "--short=7", "HEAD"]);
    assert_eq!(context.reference, GitReference::Detached(short_head));
}

#[test]
fn real_git_returns_none_for_non_repository() {
    let fixture = RepositoryFixture::new();
    let outside = fixture.root.parent().unwrap().join(format!(
        "zentty-not-repo-{}-{}",
        std::process::id(),
        NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&outside).unwrap();
    assert_eq!(
        SystemProjectContextResolver::default()
            .resolve(&outside)
            .unwrap(),
        None
    );
    fs::remove_dir_all(outside).unwrap();
}

#[test]
fn remote_parsing_accepts_common_hosts_and_rejects_hostile_urls() {
    let github = parse_git_remote("ssh://git@github.com:2222/acme/rocket.git").unwrap();
    assert_eq!(github.repository_specifier(), "acme/rocket");
    assert_eq!(
        github.branch_url("release/v1"),
        Some("https://github.com/acme/rocket/tree/release/v1".to_owned())
    );
    for hostile_branch in ["../main", "feature/../../main", "/main", "main/", "a\\b"] {
        assert_eq!(github.branch_url(hostile_branch), None);
    }

    let gitlab = parse_git_remote("https://gitlab.com/acme/rocket.git").unwrap();
    assert_eq!(
        gitlab.branch_url("main"),
        Some("https://gitlab.com/acme/rocket/-/tree/main".to_owned())
    );
    let bitbucket = parse_git_remote("git@bitbucket.org:acme/rocket.git").unwrap();
    assert_eq!(
        bitbucket.branch_url("main"),
        Some("https://bitbucket.org/acme/rocket/src/main".to_owned())
    );

    for hostile in [
        "file:///etc/passwd",
        "javascript:alert(1)",
        "https://user:password@github.com/acme/rocket.git",
        "https://github.com/acme/../rocket.git",
        "https://github.com/acme/rocket.git?x=1",
        "ssh://git@github.com:not-a-port/acme/rocket.git",
        "git@github.com:acme/rocket.git\nhttps://evil.invalid/pwn",
    ] {
        assert!(parse_git_remote(hostile).is_none(), "accepted {hostile:?}");
    }
}

#[test]
fn real_git_resolves_a_linked_worktree_as_its_own_canonical_root() {
    let fixture = RepositoryFixture::new();
    let worktree = fixture.root.parent().unwrap().join(format!(
        "zentty-worktree-{}-{}",
        std::process::id(),
        NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
    ));
    git(
        &fixture.root,
        &[
            "worktree",
            "add",
            "-b",
            "feature/worktree",
            worktree.to_str().unwrap(),
        ],
    );
    let nested = worktree.join("nested");
    fs::create_dir_all(&nested).unwrap();
    let context = SystemProjectContextResolver::default()
        .resolve(&nested)
        .unwrap()
        .unwrap();
    assert_eq!(context.repository_root, worktree.canonicalize().unwrap());
    assert_eq!(
        context.reference,
        GitReference::Branch("feature/worktree".to_owned())
    );
    git(
        &fixture.root,
        &["worktree", "remove", "--force", worktree.to_str().unwrap()],
    );
}

#[test]
fn real_gh_boundary_maps_pr_precedence_without_shell_interpolation() {
    let fixture = RepositoryFixture::new();
    git(
        &fixture.root,
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/acme/rocket.git",
        ],
    );
    let bin = fixture.root.join("fixture-bin");
    fs::create_dir_all(&bin).unwrap();
    let gh = bin.join("gh");
    fs::write(
        &gh,
        "#!/bin/sh\nset -eu\nprintf '%s\\n' '{\"number\":42,\"url\":\"https://github.com/acme/rocket/pull/42\",\"isDraft\":false,\"state\":\"OPEN\",\"reviewDecision\":\"APPROVED\",\"mergeable\":\"CONFLICTING\",\"statusCheckRollup\":[{\"status\":\"IN_PROGRESS\"},{\"status\":\"COMPLETED\",\"conclusion\":\"FAILURE\"}]}'\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&gh, fs::Permissions::from_mode(0o755)).unwrap();
    }

    let context =
        SystemProjectContextResolver::with_search_path(format!("{}:/usr/bin:/bin", bin.display()))
            .resolve(&fixture.root)
            .unwrap()
            .unwrap();
    let review = context.review.unwrap();
    assert_eq!(review.pull_request.state, PullRequestState::Open);
    assert_eq!(review.checks_state, ChecksState::Failing);
    assert_eq!(
        review
            .chips
            .iter()
            .map(|chip| (chip.text.as_str(), chip.style))
            .collect::<Vec<_>>(),
        [
            ("Approved", ReviewChipStyle::Success),
            ("1 failing", ReviewChipStyle::Danger),
            ("Conflicts", ReviewChipStyle::Danger),
        ]
    );
}
