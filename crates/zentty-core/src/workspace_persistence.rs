use std::collections::HashSet;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::workspace::{
    AgentResume, ColumnLayout, Pane, PaneLayout, StableId, Window, Worklane, Workspace,
    WorkspaceError,
};

const CURRENT_SCHEMA_VERSION: u64 = 1;

impl Workspace {
    /// Decodes strict versioned workspace JSON, including the supported v0
    /// migration, without retaining the input bytes or any live platform data.
    ///
    /// # Errors
    ///
    /// Returns an actionable structural error for malformed/unknown data or
    /// [`WorkspaceError::UnsupportedSchemaVersion`] for a newer version.
    pub fn from_json(bytes: &[u8]) -> Result<Self, WorkspaceError> {
        let value: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|error| json_error(&error))?;
        let version = value
            .get("schema_version")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| invalid("schema_version must be a non-negative integer"))?;
        let wire = match version {
            0 => V0Workspace::decode(value)?.migrate(),
            CURRENT_SCHEMA_VERSION => {
                serde_json::from_value(value).map_err(|error| json_error(&error))?
            }
            other => return Err(WorkspaceError::UnsupportedSchemaVersion(other)),
        };
        wire.try_into()
    }

    /// Encodes canonical version-1 JSON with a trailing newline.
    ///
    /// # Errors
    ///
    /// Returns an error if an in-memory path cannot be represented by the
    /// UTF-8 durable schema or JSON encoding fails.
    pub fn to_json(&self) -> Result<Vec<u8>, WorkspaceError> {
        let wire = V1Workspace::try_from(self)?;
        let mut bytes = serde_json::to_vec_pretty(&wire).map_err(|error| json_error(&error))?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct V1Workspace {
    schema_version: u64,
    workspace_id: String,
    revision: u64,
    active_window_id: String,
    windows: Vec<V1Window>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct V1Window {
    id: String,
    order: usize,
    active_worklane_id: String,
    worklanes: Vec<V1Worklane>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct V1Worklane {
    id: String,
    order: usize,
    title: Option<String>,
    layout: V1WorklaneLayout,
    active_pane_id: String,
    panes: Vec<V1Pane>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct V1WorklaneLayout {
    columns: Vec<V1Column>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct V1Column {
    index: usize,
    weight: f64,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct V1Pane {
    id: String,
    order: usize,
    title: Option<String>,
    layout: V1PaneLayout,
    cwd: String,
    command: V1Command,
    agent: Option<V1Agent>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct V1PaneLayout {
    column: usize,
    row: usize,
    row_weight: f64,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct V1Command {
    launch_profile_id: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct V1Agent {
    adapter: String,
    resume_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct V0Workspace {
    schema_version: u64,
    workspace_id: String,
    active_window: String,
    windows: Vec<V0Window>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct V0Window {
    id: String,
    active_worklane: String,
    worklanes: Vec<V0Worklane>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct V0Worklane {
    id: String,
    active_pane: String,
    panes: Vec<V0Pane>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct V0Pane {
    id: String,
    cwd: String,
    command_profile: String,
}

impl V0Workspace {
    fn decode(value: serde_json::Value) -> Result<Self, WorkspaceError> {
        let wire: Self = serde_json::from_value(value).map_err(|error| json_error(&error))?;
        if wire.schema_version != 0 {
            return Err(invalid("v0 decoder received a non-v0 document"));
        }
        Ok(wire)
    }

    fn migrate(self) -> V1Workspace {
        V1Workspace {
            schema_version: CURRENT_SCHEMA_VERSION,
            workspace_id: self.workspace_id,
            revision: 0,
            active_window_id: self.active_window,
            windows: self
                .windows
                .into_iter()
                .enumerate()
                .map(|(window_order, window)| V1Window {
                    id: window.id,
                    order: window_order,
                    active_worklane_id: window.active_worklane,
                    worklanes: window
                        .worklanes
                        .into_iter()
                        .enumerate()
                        .map(|(worklane_order, worklane)| V1Worklane {
                            id: worklane.id,
                            order: worklane_order,
                            title: None,
                            layout: V1WorklaneLayout {
                                columns: vec![V1Column {
                                    index: 0,
                                    weight: 1.0,
                                }],
                            },
                            active_pane_id: worklane.active_pane,
                            panes: worklane
                                .panes
                                .into_iter()
                                .enumerate()
                                .map(|(pane_order, pane)| V1Pane {
                                    id: pane.id,
                                    order: pane_order,
                                    title: None,
                                    layout: V1PaneLayout {
                                        column: 0,
                                        row: pane_order,
                                        row_weight: 1.0,
                                    },
                                    cwd: pane.cwd,
                                    command: V1Command {
                                        launch_profile_id: pane.command_profile,
                                    },
                                    agent: None,
                                })
                                .collect(),
                        })
                        .collect(),
                })
                .collect(),
        }
    }
}

impl TryFrom<V1Workspace> for Workspace {
    type Error = WorkspaceError;

    fn try_from(mut wire: V1Workspace) -> Result<Self, Self::Error> {
        if wire.schema_version != CURRENT_SCHEMA_VERSION {
            return Err(WorkspaceError::UnsupportedSchemaVersion(
                wire.schema_version,
            ));
        }
        require_nonempty(&wire.windows, "windows")?;
        sort_contiguous(&mut wire.windows, |window| window.order, "window order")?;

        let id = StableId::parse(wire.workspace_id)?;
        let active_window_id = StableId::parse(wire.active_window_id)?;
        let mut seen = HashSet::new();
        let mut windows = Vec::with_capacity(wire.windows.len());
        for window in wire.windows {
            windows.push(window_from_wire(window, &mut seen)?);
        }
        if !windows.iter().any(|window| window.id == active_window_id) {
            return Err(invalid("active_window_id does not resolve"));
        }
        Ok(Self {
            id,
            revision: wire.revision,
            windows,
            active_window_id,
        })
    }
}

fn window_from_wire(
    mut wire: V1Window,
    seen: &mut HashSet<String>,
) -> Result<Window, WorkspaceError> {
    require_nonempty(&wire.worklanes, "window worklanes")?;
    sort_contiguous(
        &mut wire.worklanes,
        |worklane| worklane.order,
        "worklane order",
    )?;
    let id = unique_id(wire.id, seen)?;
    let active_worklane_id = StableId::parse(wire.active_worklane_id)?;
    let worklanes = wire
        .worklanes
        .into_iter()
        .map(|worklane| worklane_from_wire(worklane, seen))
        .collect::<Result<Vec<_>, _>>()?;
    if !worklanes
        .iter()
        .any(|worklane| worklane.id == active_worklane_id)
    {
        return Err(invalid("active_worklane_id does not resolve"));
    }
    Ok(Window {
        id,
        worklanes,
        active_worklane_id,
    })
}

fn worklane_from_wire(
    mut wire: V1Worklane,
    seen: &mut HashSet<String>,
) -> Result<Worklane, WorkspaceError> {
    validate_title(wire.title.as_deref())?;
    require_nonempty(&wire.layout.columns, "worklane columns")?;
    sort_contiguous(
        &mut wire.layout.columns,
        |column| column.index,
        "column index",
    )?;
    if wire
        .layout
        .columns
        .iter()
        .any(|column| !positive_finite(column.weight))
    {
        return Err(invalid("column weight must be positive and finite"));
    }
    require_nonempty(&wire.panes, "worklane panes")?;
    sort_contiguous(&mut wire.panes, |pane| pane.order, "pane order")?;
    validate_pane_rows(&wire.panes, wire.layout.columns.len())?;
    let id = unique_id(wire.id, seen)?;
    let active_pane_id = StableId::parse(wire.active_pane_id)?;
    let panes = wire
        .panes
        .into_iter()
        .map(|pane| pane_from_wire(pane, seen))
        .collect::<Result<Vec<_>, _>>()?;
    if !panes.iter().any(|pane| pane.id == active_pane_id) {
        return Err(invalid("active_pane_id does not resolve"));
    }
    Ok(Worklane {
        id,
        title: wire.title,
        columns: wire
            .layout
            .columns
            .into_iter()
            .map(|column| ColumnLayout {
                weight: column.weight,
            })
            .collect(),
        panes,
        active_pane_id,
    })
}

fn pane_from_wire(wire: V1Pane, seen: &mut HashSet<String>) -> Result<Pane, WorkspaceError> {
    validate_title(wire.title.as_deref())?;
    if !positive_finite(wire.layout.row_weight) {
        return Err(invalid("pane row weight must be positive and finite"));
    }
    let id = unique_id(wire.id, seen)?;
    let mut pane = Pane::new(id, PathBuf::from(wire.cwd), wire.command.launch_profile_id)?;
    pane.title = wire.title;
    pane.layout = PaneLayout {
        column: wire.layout.column,
        row: wire.layout.row,
        row_weight: wire.layout.row_weight,
    };
    pane.agent = wire.agent.map(validate_agent).transpose()?;
    Ok(pane)
}

impl TryFrom<&Workspace> for V1Workspace {
    type Error = WorkspaceError;

    fn try_from(workspace: &Workspace) -> Result<Self, Self::Error> {
        Ok(Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            workspace_id: workspace.id.0.clone(),
            revision: workspace.revision,
            active_window_id: workspace.active_window_id.0.clone(),
            windows: workspace
                .windows
                .iter()
                .enumerate()
                .map(|(window_order, window)| {
                    Ok(V1Window {
                        id: window.id.0.clone(),
                        order: window_order,
                        active_worklane_id: window.active_worklane_id.0.clone(),
                        worklanes: window
                            .worklanes
                            .iter()
                            .enumerate()
                            .map(|(worklane_order, worklane)| {
                                Ok(V1Worklane {
                                    id: worklane.id.0.clone(),
                                    order: worklane_order,
                                    title: worklane.title.clone(),
                                    layout: V1WorklaneLayout {
                                        columns: worklane
                                            .columns
                                            .iter()
                                            .enumerate()
                                            .map(|(index, column)| V1Column {
                                                index,
                                                weight: column.weight,
                                            })
                                            .collect(),
                                    },
                                    active_pane_id: worklane.active_pane_id.0.clone(),
                                    panes: worklane
                                        .panes
                                        .iter()
                                        .enumerate()
                                        .map(|(pane_order, pane)| {
                                            Ok(V1Pane {
                                                id: pane.id.0.clone(),
                                                order: pane_order,
                                                title: pane.title.clone(),
                                                layout: V1PaneLayout {
                                                    column: pane.layout.column,
                                                    row: pane.layout.row,
                                                    row_weight: pane.layout.row_weight,
                                                },
                                                cwd: pane
                                                    .cwd
                                                    .to_str()
                                                    .ok_or_else(|| {
                                                        invalid("pane CWD is not valid UTF-8")
                                                    })?
                                                    .to_owned(),
                                                command: V1Command {
                                                    launch_profile_id: pane
                                                        .launch_profile_id
                                                        .clone(),
                                                },
                                                agent: pane.agent.as_ref().map(|agent| V1Agent {
                                                    adapter: agent.adapter.clone(),
                                                    resume_id: agent.resume_id.clone(),
                                                }),
                                            })
                                        })
                                        .collect::<Result<Vec<_>, WorkspaceError>>()?,
                                })
                            })
                            .collect::<Result<Vec<_>, WorkspaceError>>()?,
                    })
                })
                .collect::<Result<Vec<_>, WorkspaceError>>()?,
        })
    }
}

fn validate_pane_rows(panes: &[V1Pane], column_count: usize) -> Result<(), WorkspaceError> {
    if panes.iter().any(|pane| pane.layout.column >= column_count) {
        return Err(invalid("pane layout references an unknown column"));
    }
    for column in 0..column_count {
        let mut rows: Vec<_> = panes
            .iter()
            .filter(|pane| pane.layout.column == column)
            .map(|pane| pane.layout.row)
            .collect();
        if rows.is_empty() {
            return Err(invalid("every layout column must contain a pane"));
        }
        rows.sort_unstable();
        if rows.iter().copied().ne(0..rows.len()) {
            return Err(invalid("pane rows must be contiguous within each column"));
        }
    }
    Ok(())
}

fn validate_agent(agent: V1Agent) -> Result<AgentResume, WorkspaceError> {
    let valid_adapter = !agent.adapter.is_empty()
        && agent
            .adapter
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase())
        && agent
            .adapter
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'-');
    if !valid_adapter || agent.resume_id.is_empty() || agent.resume_id.contains('\0') {
        return Err(invalid("invalid non-secret agent resume metadata"));
    }
    Ok(AgentResume {
        adapter: agent.adapter,
        resume_id: agent.resume_id,
    })
}

fn validate_title(title: Option<&str>) -> Result<(), WorkspaceError> {
    if title.is_some_and(|value| value.is_empty() || value.contains('\0')) {
        Err(invalid("title must be non-empty and contain no NUL"))
    } else {
        Ok(())
    }
}

fn unique_id(value: String, seen: &mut HashSet<String>) -> Result<StableId, WorkspaceError> {
    let id = StableId::parse(value)?;
    if !seen.insert(id.0.clone()) {
        return Err(WorkspaceError::DuplicateId(id));
    }
    Ok(id)
}

fn sort_contiguous<T>(
    items: &mut [T],
    order: impl Fn(&T) -> usize,
    label: &str,
) -> Result<(), WorkspaceError> {
    items.sort_by_key(&order);
    if items
        .iter()
        .enumerate()
        .any(|(expected, item)| order(item) != expected)
    {
        return Err(invalid(&format!("{label} must be contiguous")));
    }
    Ok(())
}

fn require_nonempty<T>(items: &[T], label: &str) -> Result<(), WorkspaceError> {
    if items.is_empty() {
        Err(invalid(&format!("{label} must not be empty")))
    } else {
        Ok(())
    }
}

fn positive_finite(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

fn json_error(error: &serde_json::Error) -> WorkspaceError {
    invalid(&format!(
        "JSON at line {}, column {}: {error}",
        error.line(),
        error.column()
    ))
}

fn invalid(detail: &str) -> WorkspaceError {
    WorkspaceError::InvalidPersistedState(detail.to_owned())
}
