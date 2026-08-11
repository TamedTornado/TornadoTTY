use std::{
    collections::{BTreeMap, HashMap},
    path::{Component, Path},
};

use serde::{Deserialize, Serialize};

use crate::{ColumnRecipe, PaneRecipe, WorklaneRecipe};

const GENERATED_ENVIRONMENT_KEYS: &[&str] = &[
    "PATH",
    "ZDOTDIR",
    "PROMPT_COMMAND",
    "GHOSTTY_LOG",
    "COLORTERM",
    "XDG_DATA_DIRS",
];

const SHELL_PROCESS_NAMES: &[&str] = &[
    "zsh", "bash", "sh", "fish", "dash", "ksh", "tcsh", "csh", "-zsh", "-bash", "-sh", "-fish",
    "-dash", "-ksh", "-tcsh", "-csh", "login",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TemplateKind {
    Bookmark,
    Preset,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceTemplate {
    pub schema_version: i64,
    pub id: String,
    pub name: String,
    pub kind: TemplateKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub captured_readable_width: Option<f64>,
    pub next_pane_number: i64,
    #[serde(
        default,
        rename = "focusedColumnID",
        skip_serializing_if = "Option::is_none"
    )]
    pub focused_column_id: Option<String>,
    pub columns: Vec<WorkspaceTemplateColumn>,
    pub pinned: bool,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<String>,
}

impl WorkspaceTemplate {
    pub const CURRENT_SCHEMA_VERSION: i64 = 1;

    #[must_use]
    pub fn capture(
        worklane: &WorklaneRecipe,
        kind: TemplateKind,
        name: &str,
        context: WorkspaceTemplateCaptureContext<'_>,
    ) -> Self {
        let columns = worklane
            .columns
            .iter()
            .map(|column| WorkspaceTemplateColumn {
                id: column.id.clone(),
                width: column.width,
                focused_pane_id: column.focused_pane_id.clone(),
                last_focused_pane_id: column.last_focused_pane_id.clone(),
                pane_heights: column.pane_heights.clone(),
                panes: column
                    .panes
                    .iter()
                    .map(|pane| {
                        let command = context
                            .commands
                            .get(&pane.id)
                            .map(String::as_str)
                            .and_then(trimmed)
                            .filter(|command| !is_shell_process_name(command))
                            .map(str::to_owned);
                        WorkspaceTemplatePane {
                            id: pane.id.clone(),
                            custom_title: pane
                                .custom_title
                                .as_deref()
                                .and_then(trimmed)
                                .map(str::to_owned),
                            title_seed: pane
                                .last_activity_title
                                .as_deref()
                                .and_then(trimmed)
                                .or_else(|| pane.title_seed.as_deref().and_then(trimmed))
                                .map(str::to_owned),
                            working_directory: (kind == TemplateKind::Bookmark)
                                .then(|| pane.working_directory.as_deref().and_then(trimmed))
                                .flatten()
                                .map(str::to_owned),
                            command,
                            environment: safe_environment(
                                context
                                    .environments
                                    .get(&pane.id)
                                    .cloned()
                                    .unwrap_or_default(),
                            ),
                            was_user_edited: false,
                        }
                    })
                    .collect(),
            })
            .collect::<Vec<_>>();
        let project_root = (kind == TemplateKind::Bookmark)
            .then(|| {
                longest_common_ancestor(
                    columns
                        .iter()
                        .flat_map(|column| &column.panes)
                        .filter_map(|pane| pane.working_directory.as_deref()),
                )
            })
            .flatten();

        Self {
            schema_version: Self::CURRENT_SCHEMA_VERSION,
            id: context.id.to_owned(),
            name: name.trim().to_owned(),
            kind,
            title: worklane
                .title
                .as_deref()
                .and_then(trimmed)
                .map(str::to_owned),
            color: worklane.color.clone(),
            project_root,
            captured_readable_width: context.captured_readable_width,
            next_pane_number: worklane.next_pane_number.max(1),
            focused_column_id: worklane.focused_column_id.clone(),
            columns,
            pinned: false,
            created_at: context.now.to_owned(),
            updated_at: context.now.to_owned(),
            last_used_at: None,
        }
    }

    pub fn all_panes(&self) -> impl Iterator<Item = &WorkspaceTemplatePane> {
        self.columns.iter().flat_map(|column| &column.panes)
    }

    pub fn all_panes_mut(&mut self) -> impl Iterator<Item = &mut WorkspaceTemplatePane> {
        self.columns.iter_mut().flat_map(|column| &mut column.panes)
    }

    #[must_use]
    pub fn into_portable_preset(mut self, now: &str) -> Self {
        self.kind = TemplateKind::Preset;
        self.project_root = None;
        now.clone_into(&mut self.updated_at);
        for pane in self.columns.iter_mut().flat_map(|column| &mut column.panes) {
            pane.working_directory = None;
            pane.environment = safe_environment(std::mem::take(&mut pane.environment));
        }
        self
    }

    /// Creates a source-shaped worklane plus pane launch policy. The caller
    /// supplies fresh stable identities from the one platform runtime owner.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty template, exhausted identity source, or
    /// a missing fallback directory.
    pub fn restore(
        &self,
        worklane_id: &str,
        identities: &mut impl Iterator<Item = String>,
        fallback_working_directory: &str,
        readable_width: f64,
        single_column_width: f64,
        command_available: impl Fn(&str) -> bool,
    ) -> Result<WorkspaceTemplateRestore, TemplateRestoreError> {
        if self.columns.is_empty() {
            return Err(TemplateRestoreError::EmptyTemplate);
        }
        let fallback = canonical_directory(fallback_working_directory)
            .ok_or(TemplateRestoreError::MissingFallbackDirectory)?;
        let scale = self
            .captured_readable_width
            .filter(|width| width.is_finite() && *width > 0.0)
            .map_or(1.0, |width| readable_width / width);
        let mut column_id_map = HashMap::new();
        let mut restored_columns = Vec::with_capacity(self.columns.len());
        let mut launches = BTreeMap::new();
        let mut fallbacks = Vec::new();

        let command_available = &command_available as &dyn Fn(&str) -> bool;
        for column in &self.columns {
            let width = if self.columns.len() == 1 {
                single_column_width.max(1.0)
            } else {
                (column.width * scale).max(1.0)
            };
            let (restored, column_id) = restore_column(
                column,
                RestoreColumnContext {
                    identities,
                    fallback_requested: fallback_working_directory,
                    fallback_canonical: &fallback,
                    width,
                    command_available,
                    launches: &mut launches,
                    fallbacks: &mut fallbacks,
                },
            )?;
            column_id_map.entry(column.id.as_str()).or_insert(column_id);
            restored_columns.push(restored);
        }

        let focused_column_id = self
            .focused_column_id
            .as_deref()
            .and_then(|id| column_id_map.get(id))
            .cloned()
            .or_else(|| restored_columns.first().map(|column| column.id.clone()));
        Ok(WorkspaceTemplateRestore {
            recipe: WorklaneRecipe {
                id: worklane_id.to_owned(),
                title: self.title.clone(),
                next_pane_number: self.next_pane_number.max(1),
                focused_column_id,
                columns: restored_columns,
                color: self.color.clone(),
                bookmark_origin_id: Some(self.id.clone()),
            },
            launches,
            fallbacks,
        })
    }
}

struct RestoreColumnContext<'a> {
    identities: &'a mut dyn Iterator<Item = String>,
    fallback_requested: &'a str,
    fallback_canonical: &'a str,
    width: f64,
    command_available: &'a dyn Fn(&str) -> bool,
    launches: &'a mut BTreeMap<String, WorkspaceTemplatePaneLaunch>,
    fallbacks: &'a mut Vec<TemplateRestoreFallback>,
}

fn restore_column(
    column: &WorkspaceTemplateColumn,
    mut context: RestoreColumnContext<'_>,
) -> Result<(ColumnRecipe, String), TemplateRestoreError> {
    if column.panes.is_empty() {
        return Err(TemplateRestoreError::EmptyColumn);
    }
    let column_id = context
        .identities
        .next()
        .ok_or(TemplateRestoreError::IdentityExhausted)?;
    let mut pane_id_map = HashMap::new();
    let mut panes = Vec::with_capacity(column.panes.len());
    for pane in &column.panes {
        panes.push(restore_pane(pane, &mut context, &mut pane_id_map)?);
    }
    let focused_pane_id = column
        .focused_pane_id
        .as_deref()
        .and_then(|id| pane_id_map.get(id))
        .cloned()
        .or_else(|| panes.first().map(|pane| pane.id.clone()));
    let last_focused_pane_id = column
        .last_focused_pane_id
        .as_deref()
        .and_then(|id| pane_id_map.get(id))
        .cloned()
        .or_else(|| focused_pane_id.clone());
    let pane_heights = if column.pane_heights.len() == panes.len()
        && column
            .pane_heights
            .iter()
            .all(|height| height.is_finite() && *height > 0.0)
    {
        column.pane_heights.clone()
    } else {
        vec![1.0; panes.len()]
    };
    Ok((
        ColumnRecipe {
            id: column_id.clone(),
            width: context.width,
            focused_pane_id,
            last_focused_pane_id,
            pane_heights,
            panes,
        },
        column_id,
    ))
}

fn restore_pane<'a>(
    pane: &'a WorkspaceTemplatePane,
    context: &mut RestoreColumnContext<'_>,
    pane_id_map: &mut HashMap<&'a str, String>,
) -> Result<PaneRecipe, TemplateRestoreError> {
    let pane_id = context
        .identities
        .next()
        .ok_or(TemplateRestoreError::IdentityExhausted)?;
    pane_id_map
        .entry(pane.id.as_str())
        .or_insert_with(|| pane_id.clone());
    let requested = pane
        .working_directory
        .as_deref()
        .and_then(trimmed)
        .unwrap_or(context.fallback_requested);
    let working_directory = canonical_directory(requested).unwrap_or_else(|| {
        context
            .fallbacks
            .push(TemplateRestoreFallback::MissingDirectory {
                pane_id: pane_id.clone(),
                requested: requested.to_owned(),
                fell_back_to: context.fallback_canonical.to_owned(),
            });
        context.fallback_canonical.to_owned()
    });
    let saved_command = pane.command.as_deref().and_then(trimmed).map(str::to_owned);
    let (command, prefill) = match saved_command {
        Some(command) if (context.command_available)(&command) => (Some(command), None),
        Some(command) => {
            context
                .fallbacks
                .push(TemplateRestoreFallback::MissingCommand {
                    pane_id: pane_id.clone(),
                    command: command.clone(),
                });
            (None, Some(command))
        }
        None => (None, None),
    };
    context.launches.insert(
        pane_id.clone(),
        WorkspaceTemplatePaneLaunch {
            command,
            prefill,
            environment: safe_environment(pane.environment.clone()),
        },
    );
    Ok(PaneRecipe {
        id: pane_id,
        custom_title: pane.custom_title.clone(),
        title_seed: pane.title_seed.clone(),
        working_directory: Some(working_directory),
        last_activity_title: None,
        last_run_command: pane.command.clone(),
    })
}

#[derive(Clone, Copy)]
pub struct WorkspaceTemplateCaptureContext<'a> {
    pub id: &'a str,
    pub now: &'a str,
    pub captured_readable_width: Option<f64>,
    pub commands: &'a BTreeMap<String, String>,
    pub environments: &'a BTreeMap<String, BTreeMap<String, String>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceTemplateColumn {
    pub id: String,
    pub width: f64,
    #[serde(
        default,
        rename = "focusedPaneID",
        skip_serializing_if = "Option::is_none"
    )]
    pub focused_pane_id: Option<String>,
    #[serde(
        default,
        rename = "lastFocusedPaneID",
        skip_serializing_if = "Option::is_none"
    )]
    pub last_focused_pane_id: Option<String>,
    pub pane_heights: Vec<f64>,
    pub panes: Vec<WorkspaceTemplatePane>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceTemplatePane {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_seed: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub environment: BTreeMap<String, String>,
    #[serde(default)]
    pub was_user_edited: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceTemplateBundle {
    pub schema_version: i64,
    pub saved_at: String,
    pub templates: Vec<WorkspaceTemplate>,
}

impl WorkspaceTemplateBundle {
    pub const CURRENT_SCHEMA_VERSION: i64 = 1;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceTemplatePaneLaunch {
    pub command: Option<String>,
    pub prefill: Option<String>,
    pub environment: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorkspaceTemplateRestore {
    pub recipe: WorklaneRecipe,
    pub launches: BTreeMap<String, WorkspaceTemplatePaneLaunch>,
    pub fallbacks: Vec<TemplateRestoreFallback>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TemplateRestoreFallback {
    MissingDirectory {
        pane_id: String,
        requested: String,
        fell_back_to: String,
    },
    MissingCommand {
        pane_id: String,
        command: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TemplateRestoreError {
    EmptyTemplate,
    EmptyColumn,
    IdentityExhausted,
    MissingFallbackDirectory,
}

fn safe_environment(environment: BTreeMap<String, String>) -> BTreeMap<String, String> {
    environment
        .into_iter()
        .filter_map(|(key, value)| {
            let key = key.trim();
            (!key.is_empty()
                && !key.starts_with("ZENTTY_")
                && !GENERATED_ENVIRONMENT_KEYS.contains(&key))
            .then(|| (key.to_owned(), value))
        })
        .collect()
}

fn trimmed(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

fn is_shell_process_name(command: &str) -> bool {
    let Some(program) = command.split_whitespace().next() else {
        return false;
    };
    let program = Path::new(program)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(program)
        .trim_start_matches('-')
        .to_ascii_lowercase();
    SHELL_PROCESS_NAMES.contains(&program.as_str())
}

fn canonical_directory(path: &str) -> Option<String> {
    std::fs::canonicalize(path)
        .ok()
        .filter(|path| path.is_dir())
        .map(|path| path.to_string_lossy().into_owned())
}

fn longest_common_ancestor<'a>(paths: impl Iterator<Item = &'a str>) -> Option<String> {
    let normalized = paths
        .filter_map(normalized_absolute_components)
        .collect::<Vec<_>>();
    let first = normalized.first()?;
    let mut common = first.len();
    for path in normalized.iter().skip(1) {
        common = common.min(path.len());
        common = (0..common)
            .take_while(|index| first[*index] == path[*index])
            .count();
        if common == 0 {
            return None;
        }
    }
    Some(format!("/{}", first[..common].join("/")))
}

fn normalized_absolute_components(path: &str) -> Option<Vec<String>> {
    let path = Path::new(path);
    if !path.is_absolute() {
        return None;
    }
    let mut result = Vec::new();
    for component in path.components() {
        match component {
            Component::RootDir | Component::CurDir => {}
            Component::Normal(value) => result.push(value.to_string_lossy().into_owned()),
            Component::ParentDir => {
                result.pop();
            }
            Component::Prefix(_) => return None,
        }
    }
    (!result.is_empty()).then_some(result)
}
