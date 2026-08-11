use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use zentty_core::{
    TaskRunnerDisabledReason, TaskRunnerSourceKind, discover_task_runners, revalidate_task_runner,
};

fn write(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

struct TestRoot(std::path::PathBuf);

impl TestRoot {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "zentty-task-runner-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn discovers_nearest_package_scripts_with_declared_pnpm() {
    let root = TestRoot::new();
    let repo = root.path().join("repo");
    let focused = repo.join("apps/web/src");
    fs::create_dir_all(&focused).unwrap();
    fs::create_dir(repo.join(".git")).unwrap();
    write(
        &repo.join("package.json"),
        r#"{"packageManager":"pnpm@10.0.0","scripts":{"test":"vitest","dev":"vite"}}"#,
    );

    let actions = discover_task_runners(&focused).unwrap();

    assert_eq!(
        actions
            .iter()
            .map(|action| action.title.as_str())
            .collect::<Vec<_>>(),
        ["dev", "test"]
    );
    assert!(
        actions
            .iter()
            .all(|action| action.source_kind == TaskRunnerSourceKind::PackageScript)
    );
    assert_eq!(actions[0].execution_command, "pnpm run dev");
    assert_eq!(
        actions[0].id,
        format!("package-script|{}/package.json|dev", repo.display())
    );
    assert_eq!(actions[0].working_directory, repo);
    assert_eq!(actions[0].subtitle(), "package.json · pnpm run dev");
    assert!(actions[0].is_enabled());
}

#[test]
fn package_manager_lockfile_precedence_and_shell_quoting_are_deterministic() {
    let root = TestRoot::new();
    write(
        &root.path().join("package.json"),
        r#"{"scripts":{"dev server":"vite"}}"#,
    );
    write(
        &root.path().join("pnpm-lock.yaml"),
        "lockfileVersion: '9'\n",
    );
    write(&root.path().join("package-lock.json"), "{}\n");

    let actions = discover_task_runners(root.path()).unwrap();

    assert_eq!(actions[0].execution_command, "pnpm run 'dev server'");
}

#[test]
fn honors_every_declared_javascript_package_manager() {
    for (index, manager) in ["pnpm", "yarn", "bun", "npm"].into_iter().enumerate() {
        let root = TestRoot::new();
        write(
            &root.path().join("package.json"),
            &format!(
                "{{\"packageManager\":\"{manager}@1.2.3\",\"scripts\":{{\"run\":\"ignored\"}}}}"
            ),
        );
        let actions = discover_task_runners(root.path()).unwrap();
        assert_eq!(actions[0].execution_command, format!("{manager} run run"));
        assert_eq!(actions.len(), 1, "package-manager fixture {index}");
    }
}

#[test]
fn discovers_taskfile_includes_and_disables_required_variables() {
    let root = TestRoot::new();
    write(
        &root.path().join("Taskfile.yml"),
        "version: '3'\nincludes:\n  api: ./tasks/api.yml\ntasks:\n  build:\n    desc: Build all\n  prompt:\n    requires:\n      vars: [NAME, TARGET]\n",
    );
    write(
        &root.path().join("tasks/api.yml"),
        "version: '3'\ntasks:\n  test:\n    desc: Run API tests\n",
    );

    let actions = discover_task_runners(root.path()).unwrap();

    assert_eq!(
        actions
            .iter()
            .map(|action| action.title.as_str())
            .collect::<Vec<_>>(),
        ["build", "prompt", "api:test"]
    );
    assert_eq!(actions[0].description.as_deref(), Some("Build all"));
    assert_eq!(
        actions[1].disabled_reason,
        Some(TaskRunnerDisabledReason::Unsupported(
            "Task requires variables: NAME, TARGET".to_owned()
        ))
    );
    assert_eq!(actions[2].execution_command, "task api:test");
    assert!(!actions[1].is_enabled());
    assert_eq!(
        actions[1].disabled_reason.as_ref().unwrap().display_text(),
        "Task requires variables: NAME, TARGET"
    );
}

#[test]
fn parses_taskfile_inline_map_directory_and_multiline_requirements_exactly() {
    let root = TestRoot::new();
    write(
        &root.path().join("Taskfile.yaml"),
        "# heading\nincludes:\n  api: {other: './wrong.yaml', taskfile: './tasks/api.yaml'}\n  web:\n    dir: './tasks/web'\n  ignored:\n    unknown: './wrong.yaml'\n      taskfile: './wrong.yaml'\ntasks:\n  _private:\n  summarized:\n    summary: 'Summary text'\n  required:\n    requires:\n      vars:\n        - FIRST\n        - 'SECOND'\n        not-a-list-item\n    cmds:\n      - echo no\n  malformed:\n    requires:\n      vars: [BROKEN\n  ordinary:\n    cmds:\n      - echo enabled\n",
    );
    write(&root.path().join("tasks/api.yaml"), "tasks:\n  test:\n");
    write(
        &root.path().join("wrong.yaml"),
        "tasks:\n  must-not-appear:\n",
    );
    write(
        &root.path().join("tasks/web/Taskfile.yml"),
        "tasks:\n  check:\n",
    );

    let actions = discover_task_runners(root.path()).unwrap();

    assert_eq!(
        actions
            .iter()
            .map(|action| action.title.as_str())
            .collect::<Vec<_>>(),
        [
            "summarized",
            "required",
            "malformed",
            "ordinary",
            "api:test",
            "web:check"
        ]
    );
    assert_eq!(actions[0].description.as_deref(), Some("Summary text"));
    assert_eq!(
        actions[1].disabled_reason.as_ref().unwrap().display_text(),
        "Task requires variables: FIRST, SECOND"
    );
    assert_eq!(
        actions[2].disabled_reason.as_ref().unwrap().display_text(),
        "Task requires variables: [BROKEN"
    );
    assert!(actions[3].is_enabled());
}

#[test]
fn discovers_linux_vscode_jsonc_tasks_environment_and_variables() {
    let root = TestRoot::new();
    write(
        &root.path().join(".vscode/tasks.json"),
        r#"// leading JSONC comment
        /* block JSONC comment containing "quotes" and // markers */
        {
          // JSONC comment
          "tasks": [
            {"label":"lint", "command":"npm", "args":["run", "lint:strict mode"],
             "options":{"env":{"NODE_ENV":"test"}},
             "linux":{"command":"pnpm", "args":["lint:strict mode", "${workspaceFolder}"]}},
            {"label":"open-file", "command":"cat ${file}"},
          ],
        }"#,
    );

    let actions = discover_task_runners(root.path()).unwrap();

    assert_eq!(
        actions[0].execution_command,
        format!("pnpm 'lint:strict mode' '{}'", root.path().display())
    );
    assert_eq!(
        actions[0].environment.get("NODE_ENV").map(String::as_str),
        Some("test")
    );
    assert_eq!(
        actions[1].disabled_reason,
        Some(TaskRunnerDisabledReason::Unsupported(
            "Unsupported VS Code variable: ${file}".to_owned()
        ))
    );
}

#[test]
fn jsonc_preserves_comment_markers_and_escaped_quotes_inside_strings() {
    let root = TestRoot::new();
    write(
        &root.path().join(".vscode/tasks.json"),
        r#"{
          "tasks": [
            {"label":"URL // not a comment", "command":"printf \"quoted // value\"",},
            // A comment after an escaped quote must remain outside the string.
            {"label":"single slash", "command":"printf https://example.test/path / one"},
            {"label":"backslash", "command":"printf \\\\ path"},
            {"label":"two-vars", "command":"printf ${workspaceFolder}${cwd}${input:name}"},
            {"label":"duplicate", "command":"one"},
            {"label":"duplicate", "command":"two"},
          ],
        }"#,
    );

    let actions = discover_task_runners(root.path()).unwrap();

    assert_eq!(actions[0].title, "URL // not a comment");
    assert_eq!(actions[0].execution_command, "printf \"quoted // value\"");
    assert_eq!(
        actions[1].execution_command,
        "printf https://example.test/path / one"
    );
    assert_eq!(actions[2].execution_command, "printf \\\\ path");
    assert_eq!(
        actions[3].disabled_reason.as_ref().unwrap().display_text(),
        "Unsupported VS Code variable: ${input:name}"
    );
    assert_eq!(
        actions[4].id,
        format!(
            "vscode-task|{}/.vscode/tasks.json|duplicate",
            root.path().display()
        )
    );
    assert_eq!(
        actions[5].id,
        format!(
            "vscode-task|{}/.vscode/tasks.json|duplicate#2",
            root.path().display()
        )
    );
}

#[test]
fn discovers_just_make_and_mise_without_exposing_private_recipes() {
    let root = TestRoot::new();
    write(
        &root.path().join("justfile"),
        "test:\n  cargo test\n_hidden:\n  echo no\ndeploy target:\n  ./deploy {{target}}\n",
    );
    write(
        &root.path().join("Makefile"),
        ".PHONY: build clean\nbuild: ## Build app\n\tcargo build\ninternal.o: internal.c\n",
    );
    write(
        &root.path().join("mise.toml"),
        "[tasks.lint]\ndescription = 'Lint sources'\nrun = 'cargo clippy'\n\n[tasks]\nfmt = 'cargo fmt'\n",
    );
    write(&root.path().join("mise-tasks/dev"), "#!/bin/sh\n");
    write(&root.path().join(".mise/tasks/ship"), "#!/bin/sh\n");

    let actions = discover_task_runners(root.path()).unwrap();
    let observed = actions
        .iter()
        .map(|action| {
            format!(
                "{:?}:{}:{}",
                action.source_kind, action.title, action.execution_command
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        observed,
        [
            "Justfile:test:just test",
            "Justfile:deploy:just deploy",
            "Makefile:build:make build",
            "Makefile:clean:make clean",
            "Mise:lint:mise run lint",
            "Mise:fmt:mise run fmt",
            "Mise:dev:mise run dev",
            "Mise:ship:mise run ship",
        ]
    );
    assert!(
        actions
            .iter()
            .find(|action| action.title == "deploy")
            .unwrap()
            .disabled_reason
            .is_some()
    );
    assert_eq!(
        actions
            .iter()
            .find(|action| action.title == "lint")
            .unwrap()
            .description
            .as_deref(),
        Some("Lint sources")
    );
}

#[test]
fn malformed_nearest_source_does_not_hide_valid_parent_and_duplicates_keep_identity() {
    let root = TestRoot::new();
    let repo = root.path().join("repo");
    let app = repo.join("app");
    fs::create_dir_all(&app).unwrap();
    write(&repo.join("package.json"), r#"{"scripts":{"test":"root"}}"#);
    write(&app.join("package.json"), "{ broken");
    write(&app.join("Makefile"), ".PHONY: test\n");

    let actions = discover_task_runners(&app).unwrap();

    assert_eq!(
        actions
            .iter()
            .map(|action| action.title.as_str())
            .collect::<Vec<_>>(),
        ["test", "test"]
    );
    assert_ne!(actions[0].id, actions[1].id);
    assert_eq!(actions[0].source_path, app.join("Makefile"));
    assert_eq!(actions[1].source_path, repo.join("package.json"));
}

#[test]
fn refuses_symlink_sources_include_escape_and_oversized_files() {
    let root = TestRoot::new();
    let outside = root.path().join("outside.json");
    write(&outside, r#"{"scripts":{"owned":"no"}}"#);
    let project = root.path().join("project");
    fs::create_dir(&project).unwrap();
    std::os::unix::fs::symlink(&outside, project.join("package.json")).unwrap();
    write(
        &project.join("Taskfile.yml"),
        "includes:\n  bad: ../outside.yml\n  linked: linked-tasks\ntasks:\n  safe:\n",
    );
    write(&root.path().join("outside.yml"), "tasks:\n  escaped:\n");
    write(
        &root.path().join("outside-tasks/Taskfile.yml"),
        "tasks:\n  escaped-through-directory-symlink:\n",
    );
    std::os::unix::fs::symlink(
        root.path().join("outside-tasks"),
        project.join("linked-tasks"),
    )
    .unwrap();
    write(
        &project.join("Makefile"),
        &format!(".PHONY: ok\n{}", "x".repeat(1_048_577)),
    );

    let actions = discover_task_runners(&project).unwrap();

    assert_eq!(
        actions
            .iter()
            .map(|action| action.title.as_str())
            .collect::<Vec<_>>(),
        ["safe"]
    );
}

#[test]
fn refuses_directory_sources_but_accepts_the_exact_source_size_limit() {
    let root = TestRoot::new();
    fs::create_dir(root.path().join("package.json")).unwrap();
    let prefix = ".PHONY: exact\n";
    let contents = format!("{prefix}{}", "#".repeat(1_048_576 - prefix.len()));
    assert_eq!(contents.len(), 1_048_576);
    write(&root.path().join("Makefile"), &contents);

    let actions = discover_task_runners(root.path()).unwrap();

    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].title, "exact");
}

#[test]
fn ignores_indented_just_recipes_and_spaced_or_indented_make_targets() {
    let root = TestRoot::new();
    write(
        &root.path().join("justfile"),
        "# fake: comment\n  indented:\nvisible:\n  echo yes\n",
    );
    write(
        &root.path().join("Makefile"),
        "bad target: ## not runnable\n good: ## indented\nvalid: ## Valid target\n",
    );

    let actions = discover_task_runners(root.path()).unwrap();

    assert_eq!(
        actions
            .iter()
            .map(|action| action.title.as_str())
            .collect::<Vec<_>>(),
        ["visible", "valid"]
    );
}

#[test]
fn activation_revalidation_rejects_changed_deleted_and_forged_snapshots() {
    let root = TestRoot::new();
    let package = root.path().join("package.json");
    write(&package, r#"{"scripts":{"dev":"vite"}}"#);
    let action = discover_task_runners(root.path()).unwrap().remove(0);
    assert_eq!(revalidate_task_runner(&action).unwrap().id, action.id);

    write(&package, r#"{"scripts":{"dev":"vite --host"}}"#);
    assert!(revalidate_task_runner(&action).is_err());

    let fresh = discover_task_runners(root.path()).unwrap().remove(0);
    fs::remove_file(&package).unwrap();
    assert!(revalidate_task_runner(&fresh).is_err());

    let mut forged = fresh;
    forged.id.push_str("-forged");
    assert!(revalidate_task_runner(&forged).is_err());
}
