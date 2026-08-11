use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value as JsonValue;
use sha2::{Digest, Sha256};

const MAX_SOURCE_BYTES: u64 = 1_048_576;
const MAX_ANCESTRY: usize = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskRunnerSourceKind {
    PackageScript,
    Taskfile,
    VsCodeTask,
    Justfile,
    Makefile,
    Mise,
}

impl TaskRunnerSourceKind {
    fn id(self) -> &'static str {
        match self {
            Self::PackageScript => "package-script",
            Self::Taskfile => "taskfile",
            Self::VsCodeTask => "vscode-task",
            Self::Justfile => "justfile",
            Self::Makefile => "makefile",
            Self::Mise => "mise",
        }
    }

    fn display_name(self) -> &'static str {
        match self {
            Self::PackageScript => "package.json",
            Self::Taskfile => "Taskfile",
            Self::VsCodeTask => "VS Code",
            Self::Justfile => "just",
            Self::Makefile => "make",
            Self::Mise => "mise",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TaskRunnerDisabledReason {
    Unsupported(String),
}

impl TaskRunnerDisabledReason {
    #[must_use]
    pub fn display_text(&self) -> &str {
        match self {
            Self::Unsupported(reason) => reason,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskRunnerAction {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub source_kind: TaskRunnerSourceKind,
    pub source_path: PathBuf,
    pub source_root: PathBuf,
    pub focused_working_directory: PathBuf,
    pub working_directory: PathBuf,
    pub execution_command: String,
    pub command_preview: String,
    pub environment: BTreeMap<String, String>,
    pub disabled_reason: Option<TaskRunnerDisabledReason>,
    source_fingerprint: String,
}

impl TaskRunnerAction {
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.disabled_reason.is_none()
    }

    #[must_use]
    pub fn subtitle(&self) -> String {
        let source_name = self
            .source_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_else(|| self.source_kind.display_name());
        let source = if source_name == self.source_kind.display_name() {
            source_name.to_owned()
        } else {
            format!("{} · {source_name}", self.source_kind.display_name())
        };
        let base = format!("{source} · {}", self.command_preview);
        self.disabled_reason
            .as_ref()
            .map_or(base.clone(), |reason| {
                format!("{base} · {}", reason.display_text())
            })
    }
}

/// Discovers a bounded, read-only task snapshot rooted at the focused pane's
/// canonical working-directory ancestry. Malformed sources are isolated so a
/// bad child file cannot suppress valid parent tasks.
///
/// # Errors
///
/// Returns an error when the focused path cannot be canonicalized or is not a
/// directory. Individual malformed task sources are isolated and omitted.
pub fn discover_task_runners(
    focused_working_directory: &Path,
) -> Result<Vec<TaskRunnerAction>, String> {
    let focused = focused_working_directory
        .canonicalize()
        .map_err(|error| format!("canonicalize focused task directory: {error}"))?;
    if !focused.is_dir() {
        return Err("focused task path is not a directory".to_owned());
    }
    let mut actions = Vec::new();
    for source_root in ancestry(&focused) {
        scan_source_root(&source_root, &focused, &mut actions);
    }
    uniquify_ids(&mut actions);
    Ok(actions)
}

/// Revalidates the exact source snapshot immediately before GTK activation.
/// Changed, deleted, replaced, or forged task entries are not executable.
///
/// # Errors
///
/// Returns an error when the source is unreadable or changed, when the task no
/// longer exists, or when its freshly discovered representation differs.
pub fn revalidate_task_runner(action: &TaskRunnerAction) -> Result<TaskRunnerAction, String> {
    let current_fingerprint = source_fingerprint(&action.source_path)?;
    if current_fingerprint != action.source_fingerprint {
        return Err("task source changed after discovery".to_owned());
    }
    let current = discover_task_runners(&action.focused_working_directory)?
        .into_iter()
        .find(|candidate| candidate.id == action.id)
        .ok_or_else(|| "task no longer exists in the focused project".to_owned())?;
    if &current != action {
        return Err("task snapshot no longer matches its source".to_owned());
    }
    Ok(current)
}

fn ancestry(focused: &Path) -> Vec<PathBuf> {
    let mut result = Vec::new();
    let mut current = focused.to_path_buf();
    for _ in 0..MAX_ANCESTRY {
        result.push(current.clone());
        if current.join(".git").is_dir() || !current.pop() {
            break;
        }
    }
    result
}

fn scan_source_root(source_root: &Path, focused: &Path, actions: &mut Vec<TaskRunnerAction>) {
    scan_package(source_root, focused, actions);
    scan_taskfile(source_root, focused, actions);
    scan_vscode(source_root, focused, actions);
    scan_just(source_root, focused, actions);
    scan_make(source_root, focused, actions);
    scan_mise(source_root, focused, actions);
}

fn read_source(path: &Path) -> Result<(String, String), String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if !metadata.is_file() {
        return Err("task source is not a regular non-symlink file".to_owned());
    }
    if metadata.len() > MAX_SOURCE_BYTES {
        return Err("task source exceeds the one MiB limit".to_owned());
    }
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    let text = String::from_utf8(bytes.clone()).map_err(|error| error.to_string())?;
    let fingerprint = hex_digest(&bytes);
    Ok((text, fingerprint))
}

fn source_fingerprint(path: &Path) -> Result<String, String> {
    read_source(path).map(|(_, fingerprint)| fingerprint)
}

fn hex_digest(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        write!(&mut output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

struct ActionInput<'a> {
    title: &'a str,
    description: Option<String>,
    source_kind: TaskRunnerSourceKind,
    source_path: &'a Path,
    source_root: &'a Path,
    focused: &'a Path,
    execution_command: String,
    environment: BTreeMap<String, String>,
    disabled_reason: Option<TaskRunnerDisabledReason>,
    source_fingerprint: &'a str,
}

fn make_action(input: ActionInput<'_>) -> TaskRunnerAction {
    let id = format!(
        "{}|{}|{}",
        input.source_kind.id(),
        input.source_path.display(),
        input.title
    );
    TaskRunnerAction {
        id,
        title: input.title.to_owned(),
        description: input.description,
        source_kind: input.source_kind,
        source_path: input.source_path.to_path_buf(),
        source_root: input.source_root.to_path_buf(),
        focused_working_directory: input.focused.to_path_buf(),
        working_directory: input.source_root.to_path_buf(),
        command_preview: input.execution_command.clone(),
        execution_command: input.execution_command,
        environment: input.environment,
        disabled_reason: input.disabled_reason,
        source_fingerprint: input.source_fingerprint.to_owned(),
    }
}

fn uniquify_ids(actions: &mut [TaskRunnerAction]) {
    let mut counts = BTreeMap::<String, usize>::new();
    for action in actions {
        let count = counts.entry(action.id.clone()).or_default();
        *count += 1;
        if *count > 1 {
            action.id.push('#');
            action.id.push_str(&count.to_string());
        }
    }
}

fn scan_package(source_root: &Path, focused: &Path, actions: &mut Vec<TaskRunnerAction>) {
    let path = source_root.join("package.json");
    let Ok((text, fingerprint)) = read_source(&path) else {
        return;
    };
    let Ok(root) = serde_json::from_str::<JsonValue>(&text) else {
        return;
    };
    let Some(scripts) = root.get("scripts").and_then(JsonValue::as_object) else {
        return;
    };
    let runner = package_runner(&root, source_root);
    let mut names = scripts
        .iter()
        .filter_map(|(name, value)| value.is_string().then_some(name.as_str()))
        .collect::<Vec<_>>();
    names.sort_unstable();
    for name in names {
        let command = format!("{runner} run {}", shell_quote(name));
        actions.push(make_action(ActionInput {
            title: name,
            description: None,
            source_kind: TaskRunnerSourceKind::PackageScript,
            source_path: &path,
            source_root,
            focused,
            execution_command: command,
            environment: BTreeMap::new(),
            disabled_reason: None,
            source_fingerprint: &fingerprint,
        }));
    }
}

fn package_runner(root: &JsonValue, source_root: &Path) -> &'static str {
    if let Some(name) = root
        .get("packageManager")
        .and_then(JsonValue::as_str)
        .and_then(|value| value.split('@').next())
    {
        match name {
            "pnpm" => return "pnpm",
            "yarn" => return "yarn",
            "bun" => return "bun",
            _ => {}
        }
    }
    [
        ("pnpm-lock.yaml", "pnpm"),
        ("yarn.lock", "yarn"),
        ("bun.lockb", "bun"),
        ("bun.lock", "bun"),
        ("package-lock.json", "npm"),
    ]
    .into_iter()
    .find_map(|(file, runner)| source_root.join(file).is_file().then_some(runner))
    .unwrap_or("npm")
}

#[derive(Default)]
struct ParsedTaskfile {
    tasks: Vec<ParsedTask>,
    includes: Vec<(String, String)>,
}

#[derive(Default)]
struct ParsedTask {
    name: String,
    description: Option<String>,
    required_variables: Vec<String>,
}

fn scan_taskfile(source_root: &Path, focused: &Path, actions: &mut Vec<TaskRunnerAction>) {
    let Some(path) = ["Taskfile.yml", "Taskfile.yaml"]
        .into_iter()
        .map(|name| source_root.join(name))
        .find(|path| path.exists())
    else {
        return;
    };
    let Ok((text, fingerprint)) = read_source(&path) else {
        return;
    };
    let parsed = parse_taskfile(&text);
    add_taskfile_actions(
        &parsed.tasks,
        None,
        &path,
        source_root,
        focused,
        &fingerprint,
        actions,
    );
    for (alias, include) in parsed.includes {
        let include_path = source_root.join(include);
        let Ok(canonical_parent) = include_path.parent().unwrap_or(source_root).canonicalize()
        else {
            continue;
        };
        if !canonical_parent.starts_with(source_root) {
            continue;
        }
        let resolved = if include_path.is_dir() {
            ["Taskfile.yml", "Taskfile.yaml"]
                .into_iter()
                .map(|name| include_path.join(name))
                .find(|candidate| candidate.exists())
        } else {
            Some(include_path)
        };
        let Some(resolved) = resolved else { continue };
        let Ok(canonical_resolved) = resolved.canonicalize() else {
            continue;
        };
        if !canonical_resolved.starts_with(source_root) {
            continue;
        }
        let Ok((included, included_fingerprint)) = read_source(&resolved) else {
            continue;
        };
        add_taskfile_actions(
            &parse_taskfile(&included).tasks,
            Some(&alias),
            &resolved,
            source_root,
            focused,
            &included_fingerprint,
            actions,
        );
    }
}

fn add_taskfile_actions(
    tasks: &[ParsedTask],
    prefix: Option<&str>,
    source_path: &Path,
    source_root: &Path,
    focused: &Path,
    fingerprint: &str,
    actions: &mut Vec<TaskRunnerAction>,
) {
    for task in tasks.iter().filter(|task| !task.name.starts_with('_')) {
        let title = prefix.map_or_else(
            || task.name.clone(),
            |prefix| format!("{prefix}:{}", task.name),
        );
        let disabled_reason = (!task.required_variables.is_empty()).then(|| {
            TaskRunnerDisabledReason::Unsupported(format!(
                "Task requires variables: {}",
                task.required_variables.join(", ")
            ))
        });
        actions.push(make_action(ActionInput {
            title: &title,
            description: task.description.clone(),
            source_kind: TaskRunnerSourceKind::Taskfile,
            source_path,
            source_root,
            focused,
            execution_command: format!("task {}", shell_quote(&title)),
            environment: BTreeMap::new(),
            disabled_reason,
            source_fingerprint: fingerprint,
        }));
    }
}

fn parse_taskfile(text: &str) -> ParsedTaskfile {
    let mut parsed = ParsedTaskfile::default();
    let mut section = "";
    let mut current_task = None;
    let mut include_alias: Option<String> = None;
    let mut in_requires = false;
    let mut reading_vars = false;
    for line in text.lines() {
        let trimmed = line.trim();
        let indent = line.len() - line.trim_start_matches(' ').len();
        if indent == 0 && trimmed.ends_with(':') {
            section = trimmed.trim_end_matches(':');
            current_task = None;
            include_alias = None;
            in_requires = false;
            reading_vars = false;
            continue;
        }
        if section == "includes" {
            if indent == 2 {
                if let Some((alias, value)) = yaml_pair(trimmed) {
                    include_alias = Some(alias.to_owned());
                    if !value.is_empty() {
                        let path = inline_map_value(value, "taskfile")
                            .or_else(|| inline_map_value(value, "dir"))
                            .unwrap_or_else(|| strip_scalar(value));
                        parsed.includes.push((alias.to_owned(), path));
                    }
                }
            } else if indent == 4
                && let (Some(alias), Some(("taskfile" | "dir", value))) =
                    (&include_alias, yaml_pair(trimmed))
            {
                parsed.includes.push((alias.clone(), strip_scalar(value)));
            }
        } else if section == "tasks" {
            if indent == 2 && trimmed.ends_with(':') {
                parsed.tasks.push(ParsedTask {
                    name: trimmed.trim_end_matches(':').to_owned(),
                    ..ParsedTask::default()
                });
                current_task = parsed.tasks.len().checked_sub(1);
                in_requires = false;
                reading_vars = false;
            } else if let Some(index) = current_task {
                if indent == 4 {
                    if let Some((key, value)) = yaml_pair(trimmed) {
                        match key {
                            "desc" | "summary" => {
                                parsed.tasks[index].description = Some(strip_scalar(value));
                            }
                            "requires" => in_requires = true,
                            _ => {
                                in_requires = false;
                                reading_vars = false;
                            }
                        }
                    }
                } else if indent == 6 && in_requires {
                    if let Some(("vars", value)) = yaml_pair(trimmed) {
                        parsed.tasks[index].required_variables = yaml_array(value);
                        reading_vars = value.is_empty();
                    }
                } else if indent == 8 && reading_vars && trimmed.starts_with("- ") {
                    parsed.tasks[index]
                        .required_variables
                        .push(strip_scalar(&trimmed[2..]));
                }
            }
        }
    }
    parsed
}

fn yaml_pair(value: &str) -> Option<(&str, &str)> {
    let (key, value) = value.split_once(':')?;
    (!key.trim().is_empty()).then_some((key.trim(), value.trim()))
}

fn strip_scalar(value: &str) -> String {
    value.trim().trim_matches(['\'', '"']).to_owned()
}

fn yaml_array(value: &str) -> Vec<String> {
    let value = value.trim();
    if value.starts_with('[') && value.ends_with(']') {
        value[1..value.len() - 1]
            .split(',')
            .map(strip_scalar)
            .filter(|value| !value.is_empty())
            .collect()
    } else if value.is_empty() {
        Vec::new()
    } else {
        vec![strip_scalar(value)]
    }
}

fn inline_map_value(value: &str, wanted: &str) -> Option<String> {
    let inner = value.trim().strip_prefix('{')?.strip_suffix('}')?;
    inner.split(',').find_map(|entry| {
        let (key, value) = entry.split_once(':')?;
        (key.trim() == wanted).then(|| strip_scalar(value))
    })
}

fn scan_vscode(source_root: &Path, focused: &Path, actions: &mut Vec<TaskRunnerAction>) {
    let path = source_root.join(".vscode/tasks.json");
    let Ok((text, fingerprint)) = read_source(&path) else {
        return;
    };
    let relaxed = strip_jsonc(&text);
    let Ok(root) = serde_json::from_str::<JsonValue>(&relaxed) else {
        return;
    };
    let Some(tasks) = root.get("tasks").and_then(JsonValue::as_array) else {
        return;
    };
    for raw in tasks {
        let Some(base) = raw.as_object() else {
            continue;
        };
        let mut task = base.clone();
        if let Some(linux) = base.get("linux").and_then(JsonValue::as_object) {
            task.extend(linux.clone());
        }
        let Some(title) = task
            .get("label")
            .and_then(JsonValue::as_str)
            .filter(|title| !title.is_empty())
        else {
            continue;
        };
        let environment = task
            .get("options")
            .and_then(|options| options.get("env"))
            .and_then(JsonValue::as_object)
            .map(|values| {
                values
                    .iter()
                    .filter_map(|(key, value)| {
                        value.as_str().map(|value| (key.clone(), value.to_owned()))
                    })
                    .collect::<BTreeMap<_, _>>()
            })
            .unwrap_or_default();
        let Some(command) = task
            .get("command")
            .and_then(JsonValue::as_str)
            .filter(|command| !command.is_empty())
        else {
            actions.push(make_action(ActionInput {
                title,
                description: None,
                source_kind: TaskRunnerSourceKind::VsCodeTask,
                source_path: &path,
                source_root,
                focused,
                execution_command: String::new(),
                environment,
                disabled_reason: Some(TaskRunnerDisabledReason::Unsupported(
                    "VS Code task has no runnable command".to_owned(),
                )),
                source_fingerprint: &fingerprint,
            }));
            continue;
        };
        let args = task
            .get("args")
            .and_then(JsonValue::as_array)
            .map(|args| {
                args.iter()
                    .filter_map(JsonValue::as_str)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let variable = std::iter::once(command)
            .chain(args.iter().copied())
            .chain(environment.values().map(String::as_str))
            .flat_map(variable_matches)
            .find(|variable| variable != &"${workspaceFolder}" && variable != &"${cwd}")
            .map(str::to_owned);
        let mut execution_command = std::iter::once(command.to_owned())
            .chain(args.iter().map(|arg| shell_quote(arg)))
            .collect::<Vec<_>>()
            .join(" ");
        execution_command = execution_command
            .replace("${workspaceFolder}", &source_root.to_string_lossy())
            .replace("${cwd}", &focused.to_string_lossy());
        actions.push(make_action(ActionInput {
            title,
            description: None,
            source_kind: TaskRunnerSourceKind::VsCodeTask,
            source_path: &path,
            source_root,
            focused,
            execution_command,
            environment,
            disabled_reason: variable.map(|variable| {
                TaskRunnerDisabledReason::Unsupported(format!(
                    "Unsupported VS Code variable: {variable}"
                ))
            }),
            source_fingerprint: &fingerprint,
        }));
    }
}

fn strip_jsonc(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut quoted = false;
    let mut escaped = false;
    while let Some(character) = chars.next() {
        if quoted {
            output.push(character);
            if escaped {
                escaped = false;
            } else {
                match character {
                    '\\' => escaped = true,
                    '"' => quoted = false,
                    _ => {}
                }
            }
        } else {
            match character {
                '"' => {
                    quoted = true;
                    output.push(character);
                }
                '/' => match chars.peek() {
                    Some('/') => {
                        chars.next();
                        for next in chars.by_ref() {
                            if next == '\n' {
                                output.push('\n');
                                break;
                            }
                        }
                    }
                    Some('*') => {
                        chars.next();
                        output.push(' ');
                        let mut saw_star = false;
                        for next in chars.by_ref() {
                            if saw_star && next == '/' {
                                break;
                            }
                            saw_star = next == '*';
                        }
                    }
                    _ => output.push(character),
                },
                _ => output.push(character),
            }
        }
    }
    let characters = output.chars().collect::<Vec<_>>();
    let mut relaxed = String::with_capacity(output.len());
    let mut quoted = false;
    let mut escaped = false;
    for (index, character) in characters.iter().copied().enumerate() {
        if quoted {
            relaxed.push(character);
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                quoted = false;
            }
            continue;
        }
        if character == '"' {
            quoted = true;
            relaxed.push(character);
            continue;
        }
        if character == ','
            && characters[index + 1..]
                .iter()
                .copied()
                .find(|next| !next.is_whitespace())
                .is_some_and(|next| next == ']' || next == '}')
        {
            continue;
        }
        relaxed.push(character);
    }
    relaxed
}

fn variable_matches(value: &str) -> Vec<&str> {
    value
        .split_inclusive('}')
        .filter_map(|segment| {
            let start = segment.rfind("${")?;
            let candidate = &segment[start..];
            candidate.ends_with('}').then_some(candidate)
        })
        .collect()
}

fn scan_just(source_root: &Path, focused: &Path, actions: &mut Vec<TaskRunnerAction>) {
    let Some(path) = ["justfile", ".justfile", "Justfile"]
        .into_iter()
        .map(|name| source_root.join(name))
        .find(|path| path.exists())
    else {
        return;
    };
    let Ok((text, fingerprint)) = read_source(&path) else {
        return;
    };
    for line in text.lines() {
        if line.starts_with(char::is_whitespace) || line.trim_start().starts_with('#') {
            continue;
        }
        let Some((header, _)) = line.split_once(':') else {
            continue;
        };
        let mut parts = header.split_whitespace();
        let Some(name) = parts.next().filter(|name| !name.starts_with('_')) else {
            continue;
        };
        let parameters = parts.collect::<Vec<_>>();
        let disabled_reason = (!parameters.is_empty()).then(|| {
            TaskRunnerDisabledReason::Unsupported(format!(
                "Task requires parameters: {}",
                parameters.join(", ")
            ))
        });
        actions.push(make_action(ActionInput {
            title: name,
            description: None,
            source_kind: TaskRunnerSourceKind::Justfile,
            source_path: &path,
            source_root,
            focused,
            execution_command: format!("just {}", shell_quote(name)),
            environment: BTreeMap::new(),
            disabled_reason,
            source_fingerprint: &fingerprint,
        }));
    }
}

fn scan_make(source_root: &Path, focused: &Path, actions: &mut Vec<TaskRunnerAction>) {
    let Some(path) = ["Makefile", "makefile"]
        .into_iter()
        .map(|name| source_root.join(name))
        .find(|path| path.exists())
    else {
        return;
    };
    let Ok((text, fingerprint)) = read_source(&path) else {
        return;
    };
    let mut names = Vec::new();
    let mut descriptions = BTreeMap::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix(".PHONY:") {
            names.extend(rest.split_whitespace().map(str::to_owned));
            continue;
        }
        if line.starts_with(char::is_whitespace) {
            continue;
        }
        let Some((name, rest)) = line.split_once(':') else {
            continue;
        };
        let name = name.trim();
        if name.is_empty() || name.contains(char::is_whitespace) {
            continue;
        }
        if let Some((_, help)) = rest.split_once("##") {
            descriptions.insert(name.to_owned(), help.trim().to_owned());
            names.push(name.to_owned());
        }
    }
    let mut seen = BTreeSet::new();
    for name in names.into_iter().filter(|name| seen.insert(name.clone())) {
        actions.push(make_action(ActionInput {
            title: &name,
            description: descriptions.get(&name).cloned(),
            source_kind: TaskRunnerSourceKind::Makefile,
            source_path: &path,
            source_root,
            focused,
            execution_command: format!("make {}", shell_quote(&name)),
            environment: BTreeMap::new(),
            disabled_reason: None,
            source_fingerprint: &fingerprint,
        }));
    }
}

fn scan_mise(source_root: &Path, focused: &Path, actions: &mut Vec<TaskRunnerAction>) {
    let path = source_root.join("mise.toml");
    if let Ok((text, fingerprint)) = read_source(&path) {
        for (name, description) in parse_mise(&text) {
            actions.push(make_action(ActionInput {
                title: &name,
                description,
                source_kind: TaskRunnerSourceKind::Mise,
                source_path: &path,
                source_root,
                focused,
                execution_command: format!("mise run {}", shell_quote(&name)),
                environment: BTreeMap::new(),
                disabled_reason: None,
                source_fingerprint: &fingerprint,
            }));
        }
    }
    for directory in [
        source_root.join("mise-tasks"),
        source_root.join(".mise/tasks"),
    ] {
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        let mut paths = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .collect::<Vec<_>>();
        paths.sort();
        for path in paths {
            let Ok((_, fingerprint)) = read_source(&path) else {
                continue;
            };
            let Some(name) = path
                .file_stem()
                .and_then(|name| name.to_str())
                .filter(|name| !name.is_empty())
            else {
                continue;
            };
            actions.push(make_action(ActionInput {
                title: name,
                description: None,
                source_kind: TaskRunnerSourceKind::Mise,
                source_path: &path,
                source_root,
                focused,
                execution_command: format!("mise run {}", shell_quote(name)),
                environment: BTreeMap::new(),
                disabled_reason: None,
                source_fingerprint: &fingerprint,
            }));
        }
    }
}

fn parse_mise(text: &str) -> Vec<(String, Option<String>)> {
    let mut result = Vec::new();
    let mut current: Option<(String, Option<String>)> = None;
    let mut section = "";
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(name) = trimmed
            .strip_prefix("[tasks.")
            .and_then(|value| value.strip_suffix(']'))
        {
            if let Some(task) = current.take() {
                result.push(task);
            }
            current = Some((strip_scalar(name), None));
            section = "task";
        } else if trimmed == "[tasks]" {
            if let Some(task) = current.take() {
                result.push(task);
            }
            section = "tasks";
        } else if trimmed.starts_with('[') {
            if let Some(task) = current.take() {
                result.push(task);
            }
            section = "";
        } else if section == "task" && trimmed.starts_with("description") {
            if let (Some((_, description)), Some((_, value))) =
                (&mut current, trimmed.split_once('='))
            {
                *description = Some(strip_scalar(value));
            }
        } else if section == "tasks"
            && let Some(name) = trimmed
                .split_once('=')
                .map(|(name, _)| strip_scalar(name))
                .filter(|name| !name.is_empty())
        {
            result.push((name, None));
        }
    }
    if let Some(task) = current {
        result.push(task);
    }
    result
}

fn shell_quote(value: &str) -> String {
    if !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_@%+=:,./-".contains(&byte))
    {
        value.to_owned()
    } else if value.is_empty() {
        "''".to_owned()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}
