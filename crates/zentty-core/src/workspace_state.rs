/// The source-defined set of user-selectable worklane colors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorklaneColor {
    Red,
    Orange,
    Amber,
    Yellow,
    Lime,
    Green,
    Teal,
    Cyan,
    Blue,
    Indigo,
    Purple,
    Pink,
}

impl WorklaneColor {
    pub const ALL: [Self; 12] = [
        Self::Red,
        Self::Orange,
        Self::Amber,
        Self::Yellow,
        Self::Lime,
        Self::Green,
        Self::Teal,
        Self::Cyan,
        Self::Blue,
        Self::Indigo,
        Self::Purple,
        Self::Pink,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Red => "red",
            Self::Orange => "orange",
            Self::Amber => "amber",
            Self::Yellow => "yellow",
            Self::Lime => "lime",
            Self::Green => "green",
            Self::Teal => "teal",
            Self::Cyan => "cyan",
            Self::Blue => "blue",
            Self::Indigo => "indigo",
            Self::Purple => "purple",
            Self::Pink => "pink",
        }
    }

    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|color| color.as_str() == name)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaneState {
    pub id: String,
    pub title: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SidebarPaneSummary {
    pub pane_id: String,
    pub primary_text: String,
    pub is_focused: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SidebarWorklaneSummary {
    pub worklane_id: String,
    pub top_label: Option<String>,
    pub primary_text: String,
    pub pane_rows: Vec<SidebarPaneSummary>,
    pub is_active: bool,
    pub color: Option<WorklaneColor>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorklaneState {
    pub id: String,
    pub title: Option<String>,
    pub color: Option<WorklaneColor>,
    pub panes: Vec<PaneState>,
    pub focused_pane_id: String,
}

/// Platform-neutral subset of `WorklaneStore` used by the first Linux GTK
/// product slice. Callers supply stable identities, just as the Swift store
/// delegates identity creation to `RuntimeIdentity`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceState {
    worklanes: Vec<WorklaneState>,
    active_worklane_id: String,
}

impl WorkspaceState {
    #[must_use]
    pub fn new(worklane_id: impl Into<String>, pane_id: impl Into<String>) -> Self {
        let worklane_id = worklane_id.into();
        let pane_id = pane_id.into();
        Self {
            worklanes: vec![WorklaneState {
                id: worklane_id.clone(),
                title: None,
                color: None,
                panes: vec![PaneState {
                    id: pane_id.clone(),
                    title: "shell".to_owned(),
                }],
                focused_pane_id: pane_id,
            }],
            active_worklane_id: worklane_id,
        }
    }

    /// Imports the single-window, single-column subset currently rendered by
    /// the Linux shell. Unsupported source layouts fail explicitly instead of
    /// being silently flattened and later persisted incorrectly.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceStateImportError`] for empty, duplicate, invalidly
    /// focused, or multi-column source state.
    pub fn from_window_recipe(window: &WindowRecipe) -> Result<Self, WorkspaceStateImportError> {
        if window.worklanes.is_empty() {
            return Err(WorkspaceStateImportError::EmptyWindow);
        }
        let mut worklane_ids = BTreeSet::new();
        let mut pane_ids = BTreeSet::new();
        let mut worklanes = Vec::with_capacity(window.worklanes.len());
        for recipe in &window.worklanes {
            if !worklane_ids.insert(recipe.id.clone()) {
                return Err(WorkspaceStateImportError::DuplicateWorklane(
                    recipe.id.clone(),
                ));
            }
            let [column] = recipe.columns.as_slice() else {
                return Err(WorkspaceStateImportError::UnsupportedColumnCount {
                    worklane_id: recipe.id.clone(),
                    count: recipe.columns.len(),
                });
            };
            if column.panes.is_empty() {
                return Err(WorkspaceStateImportError::EmptyWorklane(recipe.id.clone()));
            }
            let mut panes = Vec::with_capacity(column.panes.len());
            for pane in &column.panes {
                if !pane_ids.insert(pane.id.clone()) {
                    return Err(WorkspaceStateImportError::DuplicatePane(pane.id.clone()));
                }
                panes.push(PaneState {
                    id: pane.id.clone(),
                    title: pane
                        .custom_title
                        .as_deref()
                        .or(pane.title_seed.as_deref())
                        .or(pane.last_activity_title.as_deref())
                        .unwrap_or("shell")
                        .to_owned(),
                });
            }
            let focused_pane_id = column
                .focused_pane_id
                .as_deref()
                .filter(|id| panes.iter().any(|pane| pane.id == *id))
                .unwrap_or(&panes[0].id)
                .to_owned();
            worklanes.push(WorklaneState {
                id: recipe.id.clone(),
                title: recipe.title.clone(),
                color: recipe.color.as_deref().and_then(WorklaneColor::named),
                panes,
                focused_pane_id,
            });
        }
        let active_worklane_id = window
            .active_worklane_id
            .as_deref()
            .filter(|id| worklanes.iter().any(|worklane| worklane.id == *id))
            .unwrap_or(&worklanes[0].id)
            .to_owned();
        Ok(Self {
            worklanes,
            active_worklane_id,
        })
    }

    /// Projects current Linux state back into a source-compatible window while
    /// preserving recipe metadata for panes and worklanes that still exist.
    #[must_use]
    pub fn to_window_recipe(&self, template: &WindowRecipe) -> WindowRecipe {
        let existing_worklanes: BTreeMap<&str, &WorklaneRecipe> = template
            .worklanes
            .iter()
            .map(|worklane| (worklane.id.as_str(), worklane))
            .collect();
        let worklanes = self
            .worklanes
            .iter()
            .map(|state| {
                let existing = existing_worklanes.get(state.id.as_str()).copied();
                let existing_column = existing.and_then(|worklane| worklane.columns.first());
                let existing_panes: BTreeMap<&str, &PaneRecipe> = existing_column
                    .into_iter()
                    .flat_map(|column| &column.panes)
                    .map(|pane| (pane.id.as_str(), pane))
                    .collect();
                let existing_heights: BTreeMap<&str, f64> = existing_column
                    .into_iter()
                    .flat_map(|column| column.panes.iter().zip(&column.pane_heights))
                    .map(|(pane, height)| (pane.id.as_str(), *height))
                    .collect();
                let panes = state
                    .panes
                    .iter()
                    .map(|pane| {
                        existing_panes.get(pane.id.as_str()).map_or_else(
                            || PaneRecipe {
                                id: pane.id.clone(),
                                custom_title: None,
                                title_seed: Some(pane.title.clone()),
                                working_directory: None,
                                last_activity_title: None,
                                last_run_command: None,
                            },
                            |recipe| (*recipe).clone(),
                        )
                    })
                    .collect();
                let pane_heights = state
                    .panes
                    .iter()
                    .map(|pane| {
                        existing_heights
                            .get(pane.id.as_str())
                            .copied()
                            .unwrap_or(1.0)
                    })
                    .collect();
                let column_id = existing_column.map_or_else(
                    || format!("column-{}", state.id),
                    |column| column.id.clone(),
                );
                let minimum_next_pane = i64::try_from(state.panes.len())
                    .unwrap_or(i64::MAX)
                    .saturating_add(1);
                WorklaneRecipe {
                    id: state.id.clone(),
                    title: state.title.clone(),
                    next_pane_number: existing.map_or(minimum_next_pane, |worklane| {
                        worklane.next_pane_number.max(minimum_next_pane)
                    }),
                    focused_column_id: Some(column_id.clone()),
                    columns: vec![ColumnRecipe {
                        id: column_id,
                        width: existing_column.map_or(1.0, |column| column.width),
                        focused_pane_id: Some(state.focused_pane_id.clone()),
                        last_focused_pane_id: Some(state.focused_pane_id.clone()),
                        pane_heights,
                        panes,
                    }],
                    color: state.color.map(|color| color.as_str().to_owned()),
                    bookmark_origin_id: existing
                        .and_then(|worklane| worklane.bookmark_origin_id.clone()),
                }
            })
            .collect();
        WindowRecipe {
            id: template.id.clone(),
            frame: template.frame.clone(),
            worklanes,
            active_worklane_id: Some(self.active_worklane_id.clone()),
        }
    }

    #[must_use]
    pub fn worklanes(&self) -> &[WorklaneState] {
        &self.worklanes
    }

    #[must_use]
    pub fn worklane_ids(&self) -> Vec<&str> {
        self.worklanes
            .iter()
            .map(|worklane| worklane.id.as_str())
            .collect()
    }

    #[must_use]
    pub fn active_worklane_id(&self) -> &str {
        &self.active_worklane_id
    }

    #[must_use]
    /// Returns the active worklane.
    ///
    /// # Panics
    ///
    /// Panics only if an internal state transition has violated the invariant
    /// that every workspace has an active worklane.
    pub fn active_worklane(&self) -> &WorklaneState {
        self.worklanes
            .iter()
            .find(|worklane| worklane.id == self.active_worklane_id)
            .expect("workspace invariant: active worklane exists")
    }

    #[must_use]
    pub fn active_pane_ids(&self) -> Vec<&str> {
        self.active_worklane()
            .panes
            .iter()
            .map(|pane| pane.id.as_str())
            .collect()
    }

    #[must_use]
    pub fn focused_pane_id(&self) -> Option<&str> {
        Some(self.active_worklane().focused_pane_id.as_str())
    }

    #[must_use]
    pub fn sidebar_summaries(&self) -> Vec<SidebarWorklaneSummary> {
        self.worklanes
            .iter()
            .map(|worklane| {
                let primary_text = worklane
                    .panes
                    .iter()
                    .find(|pane| pane.id == worklane.focused_pane_id)
                    .map_or_else(|| "shell".to_owned(), |pane| pane.title.clone());
                SidebarWorklaneSummary {
                    worklane_id: worklane.id.clone(),
                    top_label: worklane.title.clone(),
                    primary_text,
                    pane_rows: worklane
                        .panes
                        .iter()
                        .map(|pane| SidebarPaneSummary {
                            pane_id: pane.id.clone(),
                            primary_text: pane.title.clone(),
                            is_focused: pane.id == worklane.focused_pane_id,
                        })
                        .collect(),
                    is_active: worklane.id == self.active_worklane_id,
                    color: worklane.color,
                }
            })
            .collect()
    }

    pub fn create_worklane(
        &mut self,
        worklane_id: impl Into<String>,
        pane_id: impl Into<String>,
    ) -> bool {
        let worklane_id = worklane_id.into();
        let pane_id = pane_id.into();
        if self.contains_worklane(&worklane_id) || self.contains_pane(&pane_id) {
            return false;
        }
        let insertion_index = self
            .active_worklane_index()
            .map_or(self.worklanes.len(), |index| index + 1);
        self.worklanes.insert(
            insertion_index,
            WorklaneState {
                id: worklane_id.clone(),
                title: None,
                color: None,
                panes: vec![PaneState {
                    id: pane_id.clone(),
                    title: "shell".to_owned(),
                }],
                focused_pane_id: pane_id,
            },
        );
        self.active_worklane_id = worklane_id;
        true
    }

    pub fn select_worklane(&mut self, id: &str) -> bool {
        if !self.contains_worklane(id) {
            return false;
        }
        id.clone_into(&mut self.active_worklane_id);
        true
    }

    pub fn set_worklane_title(&mut self, id: &str, title: Option<&str>) -> bool {
        let Some(worklane) = self.worklanes.iter_mut().find(|worklane| worklane.id == id) else {
            return false;
        };
        let title = title
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .map(str::to_owned);
        if worklane.title == title {
            return false;
        }
        worklane.title = title;
        true
    }

    pub fn set_worklane_color(&mut self, id: &str, color: Option<WorklaneColor>) -> bool {
        let Some(worklane) = self.worklanes.iter_mut().find(|worklane| worklane.id == id) else {
            return false;
        };
        if worklane.color == color {
            return false;
        }
        worklane.color = color;
        true
    }

    pub fn move_worklane(&mut self, id: &str, to_index: usize) -> bool {
        let Some(from_index) = self.worklanes.iter().position(|worklane| worklane.id == id) else {
            return false;
        };
        if to_index >= self.worklanes.len() || from_index == to_index {
            return false;
        }
        let worklane = self.worklanes.remove(from_index);
        self.worklanes.insert(to_index, worklane);
        true
    }

    /// Removes the active worklane when another worklane can replace it.
    ///
    /// # Panics
    ///
    /// Panics only if an internal state transition has violated active-ID or
    /// non-empty-workspace invariants.
    pub fn close_active_worklane(&mut self) -> bool {
        if self.worklanes.len() == 1 {
            return false;
        }
        let active_index = self
            .active_worklane_index()
            .expect("workspace invariant: active worklane exists");
        self.worklanes.remove(active_index);
        let replacement_index = active_index.saturating_sub(1).min(self.worklanes.len() - 1);
        self.active_worklane_id = self.worklanes[replacement_index].id.clone();
        true
    }

    /// Adds and focuses a pane immediately to the right of the focused pane.
    ///
    /// # Panics
    ///
    /// Panics only if an internal state transition has violated active-lane or
    /// focused-pane invariants.
    pub fn split_focused_pane_right(&mut self, pane_id: impl Into<String>) -> bool {
        let pane_id = pane_id.into();
        if self.contains_pane(&pane_id) {
            return false;
        }
        let worklane = self.active_worklane_mut();
        let focused_index = worklane
            .panes
            .iter()
            .position(|pane| pane.id == worklane.focused_pane_id)
            .expect("workspace invariant: focused pane exists");
        worklane.panes.insert(
            focused_index + 1,
            PaneState {
                id: pane_id.clone(),
                title: "shell".to_owned(),
            },
        );
        worklane.focused_pane_id = pane_id;
        true
    }

    pub fn select_pane(&mut self, pane_id: &str) -> bool {
        let worklane = self.active_worklane_mut();
        if !worklane.panes.iter().any(|pane| pane.id == pane_id) {
            return false;
        }
        pane_id.clone_into(&mut worklane.focused_pane_id);
        true
    }

    pub fn set_pane_title(&mut self, pane_id: &str, title: &str) -> bool {
        let Some(pane) = self
            .worklanes
            .iter_mut()
            .flat_map(|worklane| &mut worklane.panes)
            .find(|pane| pane.id == pane_id)
        else {
            return false;
        };
        let title = match title.trim() {
            "" => "shell",
            title => title,
        };
        if pane.title == title {
            return false;
        }
        title.clone_into(&mut pane.title);
        true
    }

    /// Moves the focused pane one position left in the active worklane.
    ///
    /// # Panics
    ///
    /// Panics only if an internal transition violated the focused-pane
    /// invariant.
    pub fn move_focused_pane_left(&mut self) -> bool {
        let worklane = self.active_worklane_mut();
        let focused_index = worklane
            .panes
            .iter()
            .position(|pane| pane.id == worklane.focused_pane_id)
            .expect("workspace invariant: focused pane exists");
        if focused_index == 0 {
            return false;
        }
        worklane.panes.swap(focused_index, focused_index - 1);
        true
    }

    /// Moves the focused pane one position right in the active worklane.
    ///
    /// # Panics
    ///
    /// Panics only if an internal transition violated the focused-pane
    /// invariant.
    pub fn move_focused_pane_right(&mut self) -> bool {
        let worklane = self.active_worklane_mut();
        let focused_index = worklane
            .panes
            .iter()
            .position(|pane| pane.id == worklane.focused_pane_id)
            .expect("workspace invariant: focused pane exists");
        if focused_index + 1 == worklane.panes.len() {
            return false;
        }
        worklane.panes.swap(focused_index, focused_index + 1);
        true
    }

    /// Closes the focused pane or requests window closure for the last pane.
    ///
    /// # Panics
    ///
    /// Panics only if an internal state transition has violated active-lane,
    /// focused-pane, or non-empty-workspace invariants.
    pub fn close_focused_pane(&mut self) -> ClosePaneOutcome {
        let pane_id = self.active_worklane().focused_pane_id.clone();
        self.close_pane(&pane_id)
    }

    /// Closes a pane in any worklane, including an inactive pane whose shell
    /// exited. The last pane in the last worklane requests window closure and
    /// remains in the model, matching the source confirmation boundary.
    pub fn close_pane(&mut self, pane_id: &str) -> ClosePaneOutcome {
        let Some((worklane_index, pane_index)) =
            self.worklanes
                .iter()
                .enumerate()
                .find_map(|(worklane_index, worklane)| {
                    worklane
                        .panes
                        .iter()
                        .position(|pane| pane.id == pane_id)
                        .map(|pane_index| (worklane_index, pane_index))
                })
        else {
            return ClosePaneOutcome::NotFound;
        };

        if self.worklanes[worklane_index].panes.len() == 1 {
            if self.worklanes.len() == 1 {
                return ClosePaneOutcome::CloseWindow;
            }
            let removed_active = self.worklanes[worklane_index].id == self.active_worklane_id;
            self.worklanes.remove(worklane_index);
            if removed_active {
                let replacement_index = worklane_index
                    .saturating_sub(1)
                    .min(self.worklanes.len() - 1);
                self.active_worklane_id = self.worklanes[replacement_index].id.clone();
            }
            return ClosePaneOutcome::Closed;
        }

        let worklane = &mut self.worklanes[worklane_index];
        worklane.panes.remove(pane_index);
        let replacement_index = pane_index.saturating_sub(1).min(worklane.panes.len() - 1);
        worklane.focused_pane_id = worklane.panes[replacement_index].id.clone();
        ClosePaneOutcome::Closed
    }

    fn active_worklane_mut(&mut self) -> &mut WorklaneState {
        let active_id = self.active_worklane_id.clone();
        self.worklanes
            .iter_mut()
            .find(|worklane| worklane.id == active_id)
            .expect("workspace invariant: active worklane exists")
    }

    fn active_worklane_index(&self) -> Option<usize> {
        self.worklanes
            .iter()
            .position(|worklane| worklane.id == self.active_worklane_id)
    }

    fn contains_worklane(&self, id: &str) -> bool {
        self.worklanes.iter().any(|worklane| worklane.id == id)
    }

    fn contains_pane(&self, id: &str) -> bool {
        self.worklanes
            .iter()
            .flat_map(|worklane| &worklane.panes)
            .any(|pane| pane.id == id)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClosePaneOutcome {
    Closed,
    CloseWindow,
    NotFound,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkspaceStateImportError {
    EmptyWindow,
    EmptyWorklane(String),
    UnsupportedColumnCount { worklane_id: String, count: usize },
    DuplicateWorklane(String),
    DuplicatePane(String),
}

impl fmt::Display for WorkspaceStateImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyWindow => formatter.write_str("workspace window has no worklanes"),
            Self::EmptyWorklane(id) => write!(formatter, "worklane {id} has no panes"),
            Self::UnsupportedColumnCount { worklane_id, count } => write!(
                formatter,
                "worklane {worklane_id} has {count} columns; Linux currently requires exactly one"
            ),
            Self::DuplicateWorklane(id) => write!(formatter, "duplicate worklane ID: {id}"),
            Self::DuplicatePane(id) => write!(formatter, "duplicate pane ID: {id}"),
        }
    }
}

impl Error for WorkspaceStateImportError {}
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::{ColumnRecipe, PaneRecipe, WindowRecipe, WorklaneRecipe};
