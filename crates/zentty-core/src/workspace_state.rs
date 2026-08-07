use crate::pane_layout::PaneLayoutPolicy;

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
    pub custom_title: Option<String>,
    pub live_title: String,
    pub working_directory: Option<String>,
    pub last_run_command: Option<String>,
}

impl PaneState {
    fn new(id: String) -> Self {
        Self {
            id,
            custom_title: None,
            live_title: "shell".to_owned(),
            working_directory: None,
            last_run_command: None,
        }
    }

    #[must_use]
    pub fn display_title(&self) -> &str {
        self.custom_title.as_deref().unwrap_or(&self.live_title)
    }

    fn from_recipe(recipe: &PaneRecipe) -> Self {
        Self {
            id: recipe.id.clone(),
            custom_title: recipe
                .custom_title
                .as_deref()
                .map(str::trim)
                .filter(|title| !title.is_empty())
                .map(str::to_owned),
            live_title: recipe
                .last_activity_title
                .as_deref()
                .or(recipe.title_seed.as_deref())
                .unwrap_or("shell")
                .to_owned(),
            working_directory: recipe.working_directory.clone(),
            last_run_command: recipe.last_run_command.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PaneColumnState {
    pub id: String,
    pub width: f64,
    pub panes: Vec<PaneState>,
    pub pane_heights: Vec<f64>,
    pub focused_pane_id: String,
    pub last_focused_pane_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SidebarPaneSummary {
    pub pane_id: String,
    pub primary_text: String,
    pub custom_title: Option<String>,
    pub is_focused: bool,
    pub agent_status: Option<PaneAgentStatus>,
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

#[derive(Clone, Debug, PartialEq)]
pub struct WorklaneState {
    pub id: String,
    pub title: Option<String>,
    pub color: Option<WorklaneColor>,
    pub columns: Vec<PaneColumnState>,
    pub focused_column_id: String,
}

/// Platform-neutral subset of `WorklaneStore` used by the first Linux GTK
/// product slice. Callers supply stable identities, just as the Swift store
/// delegates identity creation to `RuntimeIdentity`.
#[derive(Clone, Debug, PartialEq)]
pub struct WorkspaceState {
    worklanes: Vec<WorklaneState>,
    active_worklane_id: String,
    focus_history: PaneFocusHistory,
    is_navigating_history: bool,
    closed_panes: Vec<ClosedPaneEntry>,
    agent_statuses: AgentStatusStore,
}

#[derive(Clone, Debug, PartialEq)]
struct ClosedPaneEntry {
    closed_at: u64,
    pane: PaneState,
    worklane_id: String,
    column_id: String,
    column_index: usize,
    pane_index: usize,
    column_width: f64,
    pane_height: Option<f64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RestoredPane {
    pub pane_id: String,
    pub worklane_id: String,
    pub working_directory: Option<String>,
    pub prefill_text: Option<String>,
}

const CLOSED_PANE_CAPACITY: usize = 10;
const CLOSED_PANE_EXPIRY_SECONDS: u64 = 60 * 60;

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
                columns: vec![PaneColumnState {
                    id: format!("column-{pane_id}"),
                    width: 1.0,
                    panes: vec![PaneState::new(pane_id.clone())],
                    pane_heights: vec![1.0],
                    focused_pane_id: pane_id.clone(),
                    last_focused_pane_id: pane_id.clone(),
                }],
                focused_column_id: format!("column-{pane_id}"),
            }],
            active_worklane_id: worklane_id,
            focus_history: PaneFocusHistory::default(),
            is_navigating_history: false,
            closed_panes: Vec::new(),
            agent_statuses: AgentStatusStore::default(),
        }
    }

    /// Imports source worklane columns and vertical pane geometry without
    /// flattening stable identities or persisted sizing metadata.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceStateImportError`] for empty or duplicate source
    /// state. Missing focus references fall back to the first valid item, as
    /// the source model does during restoration.
    pub fn from_window_recipe(window: &WindowRecipe) -> Result<Self, WorkspaceStateImportError> {
        if window.worklanes.is_empty() {
            return Err(WorkspaceStateImportError::EmptyWindow);
        }
        let mut worklane_ids = BTreeSet::new();
        let mut column_ids = BTreeSet::new();
        let mut pane_ids = BTreeSet::new();
        let mut worklanes = Vec::with_capacity(window.worklanes.len());
        for recipe in &window.worklanes {
            if !worklane_ids.insert(recipe.id.clone()) {
                return Err(WorkspaceStateImportError::DuplicateWorklane(
                    recipe.id.clone(),
                ));
            }
            if recipe.columns.is_empty() {
                return Err(WorkspaceStateImportError::EmptyWorklane(recipe.id.clone()));
            }
            let mut columns = Vec::with_capacity(recipe.columns.len());
            for column in &recipe.columns {
                if !column_ids.insert(column.id.clone()) {
                    return Err(WorkspaceStateImportError::DuplicateColumn(
                        column.id.clone(),
                    ));
                }
                if column.panes.is_empty() {
                    return Err(WorkspaceStateImportError::EmptyColumn(column.id.clone()));
                }
                let mut panes = Vec::with_capacity(column.panes.len());
                for pane in &column.panes {
                    if !pane_ids.insert(pane.id.clone()) {
                        return Err(WorkspaceStateImportError::DuplicatePane(pane.id.clone()));
                    }
                    panes.push(PaneState::from_recipe(pane));
                }
                let focused_pane_id = column
                    .focused_pane_id
                    .as_deref()
                    .filter(|id| panes.iter().any(|pane| pane.id == *id))
                    .unwrap_or(&panes[0].id)
                    .to_owned();
                let last_focused_pane_id = column
                    .last_focused_pane_id
                    .as_deref()
                    .filter(|id| panes.iter().any(|pane| pane.id == *id))
                    .unwrap_or(&focused_pane_id)
                    .to_owned();
                let pane_heights = if column.pane_heights.len() == panes.len() {
                    column
                        .pane_heights
                        .iter()
                        .map(|height| sanitize_dimension(*height))
                        .collect()
                } else {
                    vec![1.0; panes.len()]
                };
                columns.push(PaneColumnState {
                    id: column.id.clone(),
                    width: sanitize_dimension(column.width),
                    panes,
                    pane_heights,
                    focused_pane_id,
                    last_focused_pane_id,
                });
            }
            let focused_column_id = recipe
                .focused_column_id
                .as_deref()
                .filter(|id| columns.iter().any(|column| column.id == *id))
                .unwrap_or(&columns[0].id)
                .to_owned();
            worklanes.push(WorklaneState {
                id: recipe.id.clone(),
                title: recipe.title.clone(),
                color: recipe.color.as_deref().and_then(WorklaneColor::named),
                columns,
                focused_column_id,
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
            focus_history: PaneFocusHistory::default(),
            is_navigating_history: false,
            closed_panes: Vec::new(),
            agent_statuses: AgentStatusStore::default(),
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
                let existing_panes: BTreeMap<&str, &PaneRecipe> = existing
                    .into_iter()
                    .flat_map(|worklane| &worklane.columns)
                    .flat_map(|column| &column.panes)
                    .map(|pane| (pane.id.as_str(), pane))
                    .collect();
                let columns = state
                    .columns
                    .iter()
                    .map(|column| {
                        let panes = column
                            .panes
                            .iter()
                            .map(|pane| {
                                let mut recipe = existing_panes.get(pane.id.as_str()).map_or_else(
                                    || PaneRecipe {
                                        id: pane.id.clone(),
                                        custom_title: pane.custom_title.clone(),
                                        title_seed: Some(pane.live_title.clone()),
                                        working_directory: pane.working_directory.clone(),
                                        last_activity_title: None,
                                        last_run_command: pane.last_run_command.clone(),
                                    },
                                    |recipe| (*recipe).clone(),
                                );
                                recipe.custom_title.clone_from(&pane.custom_title);
                                if pane.live_title != "shell" {
                                    recipe.last_activity_title = Some(pane.live_title.clone());
                                }
                                recipe.working_directory.clone_from(&pane.working_directory);
                                recipe.last_run_command.clone_from(&pane.last_run_command);
                                recipe
                            })
                            .collect();
                        ColumnRecipe {
                            id: column.id.clone(),
                            width: column.width,
                            focused_pane_id: Some(column.focused_pane_id.clone()),
                            last_focused_pane_id: Some(column.last_focused_pane_id.clone()),
                            pane_heights: column.pane_heights.clone(),
                            panes,
                        }
                    })
                    .collect();
                let pane_count: usize = state.columns.iter().map(|column| column.panes.len()).sum();
                let minimum_next_pane = i64::try_from(pane_count)
                    .unwrap_or(i64::MAX)
                    .saturating_add(1);
                WorklaneRecipe {
                    id: state.id.clone(),
                    title: state.title.clone(),
                    next_pane_number: existing.map_or(minimum_next_pane, |worklane| {
                        worklane.next_pane_number.max(minimum_next_pane)
                    }),
                    focused_column_id: Some(state.focused_column_id.clone()),
                    columns,
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
            .columns
            .iter()
            .flat_map(|column| &column.panes)
            .map(|pane| pane.id.as_str())
            .collect()
    }

    #[must_use]
    pub fn pane(&self, pane_id: &str) -> Option<&PaneState> {
        self.worklanes
            .iter()
            .flat_map(|worklane| &worklane.columns)
            .flat_map(|column| &column.panes)
            .find(|pane| pane.id == pane_id)
    }

    #[must_use]
    pub fn worklane_id_for_pane(&self, pane_id: &str) -> Option<&str> {
        self.worklanes
            .iter()
            .find(|worklane| {
                worklane
                    .columns
                    .iter()
                    .flat_map(|column| &column.panes)
                    .any(|pane| pane.id == pane_id)
            })
            .map(|worklane| worklane.id.as_str())
    }

    #[must_use]
    pub fn active_columns(&self) -> &[PaneColumnState] {
        &self.active_worklane().columns
    }

    #[must_use]
    pub fn focused_pane_id(&self) -> Option<&str> {
        let worklane = self.active_worklane();
        worklane
            .columns
            .iter()
            .find(|column| column.id == worklane.focused_column_id)
            .map(|column| column.focused_pane_id.as_str())
    }

    #[must_use]
    pub fn can_navigate_back(&self) -> bool {
        self.focus_history.can_go_back()
    }

    #[must_use]
    pub fn can_navigate_forward(&self) -> bool {
        self.focus_history.can_go_forward()
    }

    #[must_use]
    pub fn recent_pane_references(&self) -> Vec<PaneReference> {
        self.focus_history
            .recent_references(&self.live_pane_references())
    }

    pub fn navigate_back(&mut self) -> bool {
        let Some(current) = self.current_pane_reference() else {
            return false;
        };
        let live = self.live_pane_references();
        let Some(target) = self.focus_history.navigate_back(current, &live) else {
            return false;
        };
        self.select_history_target(&target)
    }

    pub fn navigate_forward(&mut self) -> bool {
        let Some(current) = self.current_pane_reference() else {
            return false;
        };
        let live = self.live_pane_references();
        let Some(target) = self.focus_history.navigate_forward(current, &live) else {
            return false;
        };
        self.select_history_target(&target)
    }

    pub fn select_adjacent_pane(&mut self, forward: bool) -> bool {
        let Some(current) = self.current_pane_reference() else {
            return false;
        };
        let references = self
            .worklanes
            .iter()
            .flat_map(|worklane| {
                worklane.columns.iter().flat_map(|column| {
                    column
                        .panes
                        .iter()
                        .map(|pane| PaneReference::new(&worklane.id, &pane.id))
                })
            })
            .collect::<Vec<_>>();
        if references.len() < 2 {
            return false;
        }
        let Some(current_index) = references
            .iter()
            .position(|reference| reference == &current)
        else {
            return false;
        };
        let target_index = if forward {
            (current_index + 1) % references.len()
        } else {
            current_index.checked_sub(1).unwrap_or(references.len() - 1)
        };
        let target = &references[target_index];
        self.select_worklane_and_pane(&target.worklane_id, &target.pane_id)
    }

    #[must_use]
    pub fn sidebar_summaries(&self) -> Vec<SidebarWorklaneSummary> {
        self.worklanes
            .iter()
            .map(|worklane| {
                let focused_pane_id = worklane
                    .columns
                    .iter()
                    .find(|column| column.id == worklane.focused_column_id)
                    .map(|column| column.focused_pane_id.as_str());
                let primary_text = worklane
                    .columns
                    .iter()
                    .flat_map(|column| &column.panes)
                    .find(|pane| Some(pane.id.as_str()) == focused_pane_id)
                    .map_or_else(
                        || "shell".to_owned(),
                        |pane| pane.display_title().to_owned(),
                    );
                SidebarWorklaneSummary {
                    worklane_id: worklane.id.clone(),
                    top_label: worklane.title.clone(),
                    primary_text,
                    pane_rows: worklane
                        .columns
                        .iter()
                        .flat_map(|column| &column.panes)
                        .map(|pane| SidebarPaneSummary {
                            pane_id: pane.id.clone(),
                            primary_text: pane.display_title().to_owned(),
                            custom_title: pane.custom_title.clone(),
                            is_focused: Some(pane.id.as_str()) == focused_pane_id,
                            agent_status: self.agent_statuses.status_for_pane(&pane.id).cloned(),
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
        let previous = self.current_pane_reference();
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
                columns: vec![PaneColumnState {
                    id: format!("column-{pane_id}"),
                    width: 1.0,
                    panes: vec![PaneState::new(pane_id.clone())],
                    pane_heights: vec![1.0],
                    focused_pane_id: pane_id.clone(),
                    last_focused_pane_id: pane_id.clone(),
                }],
                focused_column_id: format!("column-{pane_id}"),
            },
        );
        self.active_worklane_id = worklane_id;
        self.record_focus_transition(previous);
        true
    }

    pub fn select_worklane(&mut self, id: &str) -> bool {
        if !self.contains_worklane(id) {
            return false;
        }
        let previous = self.current_pane_reference();
        id.clone_into(&mut self.active_worklane_id);
        self.record_focus_transition(previous);
        true
    }

    pub fn select_adjacent_worklane(&mut self, forward: bool) -> bool {
        if self.worklanes.len() < 2 {
            return false;
        }
        let Some(index) = self
            .worklanes
            .iter()
            .position(|worklane| worklane.id == self.active_worklane_id)
        else {
            return false;
        };
        let target = if forward {
            (index + 1) % self.worklanes.len()
        } else {
            index.checked_sub(1).unwrap_or(self.worklanes.len() - 1)
        };
        let id = self.worklanes[target].id.clone();
        self.select_worklane(&id)
    }

    pub fn select_worklane_and_pane(&mut self, worklane_id: &str, pane_id: &str) -> bool {
        let Some(worklane) = self
            .worklanes
            .iter()
            .find(|worklane| worklane.id == worklane_id)
        else {
            return false;
        };
        if !worklane
            .columns
            .iter()
            .any(|column| column.panes.iter().any(|pane| pane.id == pane_id))
        {
            return false;
        }

        let previous = self.current_pane_reference();
        worklane_id.clone_into(&mut self.active_worklane_id);
        let selected = self.select_pane_without_history(pane_id);
        self.record_focus_transition(previous);
        selected
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

    /// Moves a worklane into an insertion slot computed after excluding the
    /// dragged worklane, matching the sidebar drag-preview model.
    pub fn reorder_worklane(&mut self, id: &str, insertion_index: usize) -> bool {
        let Some(from_index) = self.worklanes.iter().position(|worklane| worklane.id == id) else {
            return false;
        };
        if insertion_index >= self.worklanes.len() {
            return false;
        }
        let final_index = insertion_index;
        if from_index == final_index {
            return false;
        }
        let worklane = self.worklanes.remove(from_index);
        self.worklanes.insert(final_index, worklane);
        true
    }

    /// Removes the active worklane when another worklane can replace it.
    ///
    /// # Panics
    ///
    /// Panics only if an internal state transition has violated active-ID or
    /// non-empty-workspace invariants.
    pub fn close_active_worklane(&mut self) -> bool {
        let active_id = self.active_worklane_id.clone();
        self.close_worklane(&active_id)
    }

    /// Removes a worklane by stable identity when another worklane can replace
    /// it. Closing an inactive worklane does not change the current selection.
    pub fn close_worklane(&mut self, id: &str) -> bool {
        if self.worklanes.len() == 1 {
            return false;
        }
        let Some(index) = self.worklanes.iter().position(|worklane| worklane.id == id) else {
            return false;
        };
        let was_active = self.active_worklane_id == id;
        self.worklanes.remove(index);
        if was_active {
            let replacement_index = index.saturating_sub(1).min(self.worklanes.len() - 1);
            self.active_worklane_id = self.worklanes[replacement_index].id.clone();
        }
        true
    }

    /// Adds and focuses a new single-pane column immediately to the right of
    /// the focused column.
    ///
    /// # Panics
    ///
    /// Panics only if an internal state transition has violated active-lane or
    /// focused-pane invariants.
    pub fn split_focused_pane_right(&mut self, pane_id: impl Into<String>) -> bool {
        let pane_id = pane_id.into();
        self.insert_focused_pane_right(&pane_id, None)
    }

    pub fn split_focused_pane_right_visibly(
        &mut self,
        pane_id: impl Into<String>,
        column_width: f64,
    ) -> bool {
        let pane_id = pane_id.into();
        self.insert_focused_pane_right(&pane_id, Some(sanitize_dimension(column_width)))
    }

    /// Inserts and focuses a new single-pane column immediately to the left of
    /// the focused column.
    ///
    /// # Panics
    ///
    /// Panics only if an internal state transition has violated the focused-
    /// column invariant.
    pub fn insert_focused_pane_left(
        &mut self,
        pane_id: impl Into<String>,
        column_width: f64,
    ) -> bool {
        let previous = self.current_pane_reference();
        let pane_id = pane_id.into();
        if self.contains_pane(&pane_id) {
            return false;
        }
        let worklane = self.active_worklane_mut();
        let focused_index = worklane
            .columns
            .iter()
            .position(|column| column.id == worklane.focused_column_id)
            .expect("workspace invariant: focused column exists");
        let width = sanitize_dimension(column_width);
        let column_id = format!("column-{pane_id}");
        worklane.columns.insert(
            focused_index,
            PaneColumnState {
                id: column_id.clone(),
                width,
                panes: vec![PaneState::new(pane_id.clone())],
                pane_heights: vec![1.0],
                focused_pane_id: pane_id.clone(),
                last_focused_pane_id: pane_id,
            },
        );
        worklane.focused_column_id = column_id;
        self.record_focus_transition(previous);
        true
    }

    pub fn add_pane_right_without_resizing(
        &mut self,
        pane_id: impl Into<String>,
        column_width: f64,
    ) -> bool {
        let pane_id = pane_id.into();
        self.insert_focused_pane_right(&pane_id, Some(sanitize_dimension(column_width)))
    }

    fn insert_focused_pane_right(&mut self, pane_id: &str, width: Option<f64>) -> bool {
        let previous = self.current_pane_reference();
        if self.contains_pane(pane_id) {
            return false;
        }
        let worklane = self.active_worklane_mut();
        let focused_index = worklane
            .columns
            .iter()
            .position(|column| column.id == worklane.focused_column_id)
            .expect("workspace invariant: focused column exists");
        let width = width.unwrap_or(worklane.columns[focused_index].width);
        if width.is_finite() && width > 0.0 {
            worklane.columns[focused_index].width = width;
        }
        worklane.columns.insert(
            focused_index + 1,
            PaneColumnState {
                id: format!("column-{pane_id}"),
                width,
                panes: vec![PaneState::new(pane_id.to_owned())],
                pane_heights: vec![1.0],
                focused_pane_id: pane_id.to_owned(),
                last_focused_pane_id: pane_id.to_owned(),
            },
        );
        worklane.focused_column_id = format!("column-{pane_id}");
        self.record_focus_transition(previous);
        true
    }

    /// Adds and focuses a pane immediately below the focused pane in the same
    /// column.
    ///
    /// # Panics
    ///
    /// Panics only if an internal state transition has violated focused-column
    /// or focused-pane invariants.
    pub fn split_focused_pane_below(&mut self, pane_id: impl Into<String>) -> bool {
        let previous = self.current_pane_reference();
        let pane_id = pane_id.into();
        if self.contains_pane(&pane_id) {
            return false;
        }
        let worklane = self.active_worklane_mut();
        let column = worklane
            .columns
            .iter_mut()
            .find(|column| column.id == worklane.focused_column_id)
            .expect("workspace invariant: focused column exists");
        let focused_index = column
            .panes
            .iter()
            .position(|pane| pane.id == column.focused_pane_id)
            .expect("workspace invariant: focused pane exists");
        let height = column.pane_heights[focused_index];
        column
            .panes
            .insert(focused_index + 1, PaneState::new(pane_id.clone()));
        column.pane_heights[focused_index] = height / 2.0;
        column.pane_heights.insert(focused_index + 1, height / 2.0);
        column.focused_pane_id.clone_from(&pane_id);
        column.last_focused_pane_id = pane_id;
        self.record_focus_transition(previous);
        true
    }

    /// Inserts and focuses a pane immediately above the focused pane.
    ///
    /// # Panics
    ///
    /// Panics only if an internal state transition has violated focused-column
    /// or focused-pane invariants.
    pub fn insert_focused_pane_above(&mut self, pane_id: impl Into<String>) -> bool {
        let previous = self.current_pane_reference();
        let pane_id = pane_id.into();
        if self.contains_pane(&pane_id) {
            return false;
        }
        let worklane = self.active_worklane_mut();
        let column = worklane
            .columns
            .iter_mut()
            .find(|column| column.id == worklane.focused_column_id)
            .expect("workspace invariant: focused column exists");
        let focused_index = column
            .panes
            .iter()
            .position(|pane| pane.id == column.focused_pane_id)
            .expect("workspace invariant: focused pane exists");
        let height = column.pane_heights[focused_index];
        column
            .panes
            .insert(focused_index, PaneState::new(pane_id.clone()));
        column.pane_heights[focused_index] = height / 2.0;
        column.pane_heights.insert(focused_index, height / 2.0);
        column.focused_pane_id.clone_from(&pane_id);
        column.last_focused_pane_id = pane_id;
        self.record_focus_transition(previous);
        true
    }

    pub fn focus_pane_left(&mut self) -> bool {
        self.focus_adjacent_column(-1)
    }

    pub fn focus_pane_right(&mut self) -> bool {
        self.focus_adjacent_column(1)
    }

    fn focus_adjacent_column(&mut self, offset: isize) -> bool {
        let previous = self.current_pane_reference();
        let worklane = self.active_worklane_mut();
        let focused_index = worklane
            .columns
            .iter()
            .position(|column| column.id == worklane.focused_column_id)
            .expect("workspace invariant: focused column exists");
        let Some(target_index) = focused_index.checked_add_signed(offset) else {
            return false;
        };
        let Some(target) = worklane.columns.get_mut(target_index) else {
            return false;
        };
        worklane.focused_column_id.clone_from(&target.id);
        target
            .focused_pane_id
            .clone_from(&target.last_focused_pane_id);
        self.record_focus_transition(previous);
        true
    }

    pub fn focus_pane_up(&mut self) -> bool {
        self.focus_pane_vertically(-1)
    }

    pub fn focus_pane_down(&mut self) -> bool {
        self.focus_pane_vertically(1)
    }

    fn focus_pane_vertically(&mut self, offset: isize) -> bool {
        let previous = self.current_pane_reference();
        let worklane = self.active_worklane_mut();
        let column = worklane
            .columns
            .iter_mut()
            .find(|column| column.id == worklane.focused_column_id)
            .expect("workspace invariant: focused column exists");
        let focused_index = column
            .panes
            .iter()
            .position(|pane| pane.id == column.focused_pane_id)
            .expect("workspace invariant: focused pane exists");
        let Some(target_index) = focused_index.checked_add_signed(offset) else {
            return false;
        };
        let Some(target) = column.panes.get(target_index) else {
            return false;
        };
        column.focused_pane_id.clone_from(&target.id);
        column.last_focused_pane_id.clone_from(&target.id);
        self.record_focus_transition(previous);
        true
    }

    /// Sets every column to the source preset width: the readable viewport
    /// divided by the requested number of simultaneously visible columns.
    pub fn arrange_columns(&mut self, visible_column_count: usize, available_width: f64) -> bool {
        if !(1..=4).contains(&visible_column_count) || !available_width.is_finite() {
            return false;
        }
        let visible_count = small_count_as_f64(visible_column_count);
        let total_spacing = f64::from(PaneLayoutPolicy::INTER_PANE_SPACING)
            * small_count_as_f64(visible_column_count.saturating_sub(1));
        let target = sanitize_dimension((available_width - total_spacing).max(1.0) / visible_count);
        let mut changed = false;
        for column in &mut self.active_worklane_mut().columns {
            changed |= (column.width - target).abs() > f64::EPSILON;
            column.width = target;
        }
        changed
    }

    /// Reflows panes in sidebar/reading order into columns containing the
    /// requested number of panes while preserving stable pane identities and
    /// the focused pane.
    ///
    /// # Panics
    ///
    /// Panics only if an internal state transition has violated active-lane,
    /// focused-pane, or nonempty-column invariants.
    pub fn arrange_panes_per_column(&mut self, panes_per_column: usize) -> bool {
        if !(1..=4).contains(&panes_per_column) {
            return false;
        }
        let worklane = self.active_worklane_mut();
        let before = worklane.columns.clone();
        let focused_pane_id = before
            .iter()
            .find(|column| column.id == worklane.focused_column_id)
            .map(|column| column.focused_pane_id.clone())
            .expect("workspace invariant: focused pane exists");
        let widths = before.iter().map(|column| column.width).collect::<Vec<_>>();
        let panes = before
            .iter()
            .flat_map(|column| column.panes.iter().cloned())
            .collect::<Vec<_>>();
        let mut rebuilt = Vec::new();
        for (index, chunk) in panes.chunks(panes_per_column).enumerate() {
            let chunk = chunk.to_vec();
            let first_id = chunk[0].id.clone();
            let focused = if chunk.iter().any(|pane| pane.id == focused_pane_id) {
                focused_pane_id.clone()
            } else {
                first_id.clone()
            };
            rebuilt.push(PaneColumnState {
                id: before
                    .get(index)
                    .map_or_else(|| format!("column-{first_id}"), |column| column.id.clone()),
                width: widths[index.min(widths.len() - 1)],
                pane_heights: vec![1.0 / small_count_as_f64(chunk.len()); chunk.len()],
                panes: chunk,
                focused_pane_id: focused.clone(),
                last_focused_pane_id: focused,
            });
        }
        let focused_column_id = rebuilt
            .iter()
            .find(|column| column.panes.iter().any(|pane| pane.id == focused_pane_id))
            .map(|column| column.id.clone())
            .expect("workspace invariant: focused pane survives reflow");
        if rebuilt == before {
            return false;
        }
        worklane.columns = rebuilt;
        worklane.focused_column_id = focused_column_id;
        true
    }

    /// # Panics
    ///
    /// Panics only if an internal state transition has violated the focused-
    /// column invariant.
    pub fn arrange_golden_width(&mut self, focus_wide: bool, available_width: f64) -> bool {
        let worklane = self.active_worklane_mut();
        let focused_column_id = worklane.focused_column_id.clone();
        arrange_golden_column_width(worklane, &focused_column_id, focus_wide, available_width)
    }

    /// Applies the source golden-width layout to the column containing a
    /// specific pane without changing the active worklane or focused pane.
    pub fn arrange_golden_width_for_pane(
        &mut self,
        pane_id: &str,
        focus_wide: bool,
        available_width: f64,
    ) -> bool {
        let Some((worklane, column_id)) = self.worklanes.iter_mut().find_map(|worklane| {
            let column_id = worklane
                .columns
                .iter()
                .find(|column| column.panes.iter().any(|pane| pane.id == pane_id))?
                .id
                .clone();
            Some((worklane, column_id))
        }) else {
            return false;
        };
        arrange_golden_column_width(worklane, &column_id, focus_wide, available_width)
    }

    /// # Panics
    ///
    /// Panics only if an internal state transition has violated focused-column
    /// or focused-pane invariants.
    pub fn arrange_golden_height(&mut self, focus_tall: bool) -> bool {
        let worklane = self.active_worklane_mut();
        let column = worklane
            .columns
            .iter_mut()
            .find(|column| column.id == worklane.focused_column_id)
            .expect("workspace invariant: focused column exists");
        if column.panes.len() < 2 {
            return false;
        }
        let focused_index = column
            .panes
            .iter()
            .position(|pane| pane.id == column.focused_pane_id)
            .expect("workspace invariant: focused pane exists");
        let neighbor_index = if focused_index + 1 < column.panes.len() {
            focused_index + 1
        } else {
            focused_index - 1
        };
        let major = (1.0 + 5.0_f64.sqrt()) / (3.0 + 5.0_f64.sqrt());
        let focused_ratio = if focus_tall { major } else { 1.0 - major };
        let pair_total = column.pane_heights[focused_index] + column.pane_heights[neighbor_index];
        let focused_height = pair_total * focused_ratio;
        let neighbor_height = pair_total - focused_height;
        let changed = (column.pane_heights[focused_index] - focused_height).abs() > f64::EPSILON;
        column.pane_heights[focused_index] = focused_height;
        column.pane_heights[neighbor_index] = neighbor_height;
        changed
    }

    pub fn equalize_pane_heights_in_column(&mut self, pane_id: &str) -> bool {
        let Some(column) = self.worklanes.iter_mut().find_map(|worklane| {
            worklane
                .columns
                .iter_mut()
                .find(|column| column.panes.iter().any(|pane| pane.id == pane_id))
        }) else {
            return false;
        };
        let height = 1.0 / small_count_as_f64(column.panes.len());
        let mut changed = false;
        for pane_height in &mut column.pane_heights {
            changed |= pane_height.to_bits() != height.to_bits();
            *pane_height = height;
        }
        changed
    }

    /// Restores an absolute source column width without changing selection or
    /// resizing neighboring columns.
    pub fn restore_column_width(&mut self, pane_id: &str, width: f64) -> bool {
        if !width.is_finite() || width <= 0.0 {
            return false;
        }
        let Some(column) = self.worklanes.iter_mut().find_map(|worklane| {
            worklane
                .columns
                .iter_mut()
                .find(|column| column.panes.iter().any(|pane| pane.id == pane_id))
        }) else {
            return false;
        };
        if column.width.to_bits() == width.to_bits() {
            return false;
        }
        column.width = width;
        true
    }

    pub fn reset_active_layout(&mut self, default_column_width: f64) -> bool {
        let width = sanitize_dimension(default_column_width);
        let mut changed = false;
        for column in &mut self.active_worklane_mut().columns {
            changed |= (column.width - width).abs() > f64::EPSILON;
            column.width = width;
            let height = 1.0 / small_count_as_f64(column.panes.len());
            for pane_height in &mut column.pane_heights {
                changed |= (*pane_height - height).abs() > f64::EPSILON;
                *pane_height = height;
            }
        }
        changed
    }

    pub fn select_pane(&mut self, pane_id: &str) -> bool {
        let previous = self.current_pane_reference();
        let selected = self.select_pane_without_history(pane_id);
        if selected {
            self.record_focus_transition(previous);
        }
        selected
    }

    fn select_pane_without_history(&mut self, pane_id: &str) -> bool {
        let worklane = self.active_worklane_mut();
        for column in &mut worklane.columns {
            if column.panes.iter().any(|pane| pane.id == pane_id) {
                pane_id.clone_into(&mut column.focused_pane_id);
                pane_id.clone_into(&mut column.last_focused_pane_id);
                worklane.focused_column_id.clone_from(&column.id);
                return true;
            }
        }
        false
    }

    pub fn set_pane_title(&mut self, pane_id: &str, title: &str) -> bool {
        let Some(pane) = self
            .worklanes
            .iter_mut()
            .flat_map(|worklane| &mut worklane.columns)
            .flat_map(|column| &mut column.panes)
            .find(|pane| pane.id == pane_id)
        else {
            return false;
        };
        let title = match title.trim() {
            "" => "shell",
            title => title,
        };
        if pane.live_title == title {
            return false;
        }
        title.clone_into(&mut pane.live_title);
        true
    }

    pub fn set_pane_custom_title(&mut self, pane_id: &str, title: Option<&str>) -> bool {
        let Some(pane) = self
            .worklanes
            .iter_mut()
            .flat_map(|worklane| &mut worklane.columns)
            .flat_map(|column| &mut column.panes)
            .find(|pane| pane.id == pane_id)
        else {
            return false;
        };
        let title = title
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .map(str::to_owned);
        if pane.custom_title == title {
            return false;
        }
        pane.custom_title = title;
        true
    }

    /// Moves the focused pane into the preceding column, or extracts it into a
    /// new leading column when the first column contains multiple panes.
    ///
    /// # Panics
    ///
    /// Panics only if an internal transition violated the focused-pane
    /// invariant.
    pub fn move_focused_pane_left(&mut self) -> bool {
        let worklane = self.active_worklane_mut();
        let column_index = worklane
            .columns
            .iter()
            .position(|column| column.id == worklane.focused_column_id)
            .expect("workspace invariant: focused column exists");
        if column_index == 0 && worklane.columns[0].panes.len() == 1 {
            return false;
        }
        let pane_id = worklane.columns[column_index].focused_pane_id.clone();
        let (pane, height) = remove_pane(&mut worklane.columns[column_index], &pane_id);
        if column_index == 0 {
            let column_id = format!("column-{}", pane.id);
            worklane.columns.insert(
                0,
                PaneColumnState {
                    id: column_id.clone(),
                    width: worklane.columns[0].width,
                    panes: vec![pane],
                    pane_heights: vec![height],
                    focused_pane_id: pane_id.clone(),
                    last_focused_pane_id: pane_id,
                },
            );
            worklane.focused_column_id = column_id;
        } else {
            if worklane.columns[column_index].panes.is_empty() {
                worklane.columns.remove(column_index);
            }
            let destination = &mut worklane.columns[column_index - 1];
            destination.panes.insert(0, pane);
            destination.pane_heights = vec![1.0; destination.panes.len()];
            destination.focused_pane_id.clone_from(&pane_id);
            destination.last_focused_pane_id = pane_id;
            worklane.focused_column_id.clone_from(&destination.id);
        }
        true
    }

    /// Moves the focused pane into the following column, or extracts it into a
    /// new trailing column when the last column contains multiple panes.
    ///
    /// # Panics
    ///
    /// Panics only if an internal transition violated the focused-pane
    /// invariant.
    pub fn move_focused_pane_right(&mut self) -> bool {
        let worklane = self.active_worklane_mut();
        let column_index = worklane
            .columns
            .iter()
            .position(|column| column.id == worklane.focused_column_id)
            .expect("workspace invariant: focused column exists");
        if column_index + 1 == worklane.columns.len()
            && worklane.columns[column_index].panes.len() == 1
        {
            return false;
        }
        let pane_id = worklane.columns[column_index].focused_pane_id.clone();
        let (pane, height) = remove_pane(&mut worklane.columns[column_index], &pane_id);
        if column_index + 1 == worklane.columns.len() {
            let column_id = format!("column-{}", pane.id);
            worklane.columns.push(PaneColumnState {
                id: column_id.clone(),
                width: worklane.columns[column_index].width,
                panes: vec![pane],
                pane_heights: vec![height],
                focused_pane_id: pane_id.clone(),
                last_focused_pane_id: pane_id,
            });
            worklane.focused_column_id = column_id;
        } else {
            let source_removed = worklane.columns[column_index].panes.is_empty();
            if source_removed {
                worklane.columns.remove(column_index);
            }
            let destination_index = if source_removed {
                column_index
            } else {
                column_index + 1
            };
            let destination = &mut worklane.columns[destination_index];
            destination.panes.insert(0, pane);
            destination.pane_heights = vec![1.0; destination.panes.len()];
            destination.focused_pane_id.clone_from(&pane_id);
            destination.last_focused_pane_id = pane_id;
            worklane.focused_column_id.clone_from(&destination.id);
        }
        true
    }

    pub fn move_focused_pane_up(&mut self) -> bool {
        self.move_focused_pane_vertically(-1)
    }

    pub fn move_focused_pane_down(&mut self) -> bool {
        self.move_focused_pane_vertically(1)
    }

    /// Moves the focused pane from the active worklane into a new rightmost
    /// column in an existing worklane, matching the default source transfer.
    /// The destination becomes active and focused; an emptied source worklane
    /// is removed.
    ///
    /// # Panics
    ///
    /// Panics only if an internal state transition has violated active
    /// worklane, focused-column, or focused-pane invariants.
    pub fn transfer_focused_pane_to_worklane(&mut self, target_worklane_id: &str) -> bool {
        let Some(source_index) = self
            .worklanes
            .iter()
            .position(|worklane| worklane.id == self.active_worklane_id)
        else {
            return false;
        };
        let Some(target_index) = self
            .worklanes
            .iter()
            .position(|worklane| worklane.id == target_worklane_id)
        else {
            return false;
        };
        if source_index == target_index {
            return false;
        }

        let source_column_index = self.worklanes[source_index]
            .columns
            .iter()
            .position(|column| column.id == self.worklanes[source_index].focused_column_id)
            .expect("workspace invariant: focused column exists");
        let pane_id = self.worklanes[source_index].columns[source_column_index]
            .focused_pane_id
            .clone();
        let width = self.worklanes[source_index].columns[source_column_index].width;
        let (pane, _) = remove_pane(
            &mut self.worklanes[source_index].columns[source_column_index],
            &pane_id,
        );

        if self.worklanes[source_index].columns[source_column_index]
            .panes
            .is_empty()
        {
            self.worklanes[source_index]
                .columns
                .remove(source_column_index);
            if let Some(replacement_id) = self.worklanes[source_index]
                .columns
                .get(source_column_index)
                .or_else(|| self.worklanes[source_index].columns.last())
                .map(|column| column.id.clone())
            {
                self.worklanes[source_index].focused_column_id = replacement_id;
            }
        }

        let source_removed = self.worklanes[source_index].columns.is_empty();
        if source_removed {
            self.worklanes.remove(source_index);
        }
        let target_index = if source_removed && source_index < target_index {
            target_index - 1
        } else {
            target_index
        };
        let column_id = self.unique_column_id(&pane_id);
        self.worklanes[target_index].columns.push(PaneColumnState {
            id: column_id.clone(),
            width,
            panes: vec![pane],
            pane_heights: vec![1.0],
            focused_pane_id: pane_id.clone(),
            last_focused_pane_id: pane_id,
        });
        self.worklanes[target_index].focused_column_id = column_id;
        target_worklane_id.clone_into(&mut self.active_worklane_id);
        true
    }

    /// Closes the focused pane or requests window closure for the last pane.
    ///
    /// # Panics
    ///
    /// Panics only if an internal state transition has violated active-lane,
    /// focused-pane, or non-empty-workspace invariants.
    pub fn close_focused_pane(&mut self) -> ClosePaneOutcome {
        let pane_id = self
            .focused_pane_id()
            .expect("workspace invariant: focused pane exists")
            .to_owned();
        self.close_pane(&pane_id)
    }

    /// Closes a pane in any worklane, including an inactive pane whose shell
    /// exited. The last pane in the last worklane requests window closure and
    /// remains in the model, matching the source confirmation boundary.
    pub fn close_pane(&mut self, pane_id: &str) -> ClosePaneOutcome {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs());
        self.close_pane_at(pane_id, now)
    }

    /// Closes a pane using an explicit clock value for deterministic expiry
    /// tests. Product callers should use [`Self::close_pane`].
    pub fn close_pane_at(&mut self, pane_id: &str, now: u64) -> ClosePaneOutcome {
        self.close_pane_at_with_capture(pane_id, now, true)
    }

    /// Removes a pane because its child exited naturally. Source behavior does
    /// not put non-user closures on the Undo Close Pane stack.
    pub fn close_pane_after_child_exit(&mut self, pane_id: &str) -> ClosePaneOutcome {
        self.close_pane_at_with_capture(pane_id, 0, false)
    }

    pub fn apply_agent_event(&mut self, event: AuthenticatedAgentEvent, now: u64) {
        self.agent_statuses.apply(event, now);
    }

    /// Reconciles a real terminal-title callback into the canonical per-pane
    /// agent store. Unknown panes, non-Codex sessions, and unrelated titles
    /// are no-ops.
    pub fn reconcile_terminal_title(&mut self, pane_id: &str, title: &str, now: u64) -> bool {
        self.agent_statuses
            .clear_codex_after_shell_return(pane_id, title)
            || self.agent_statuses.apply_codex_title(pane_id, title, now)
    }

    /// Records a physical terminal input submission after the product has
    /// allowed the key event to reach the embedded Ghostty surface.
    pub fn record_terminal_input_submitted(&mut self, pane_id: &str, now: u64) -> bool {
        self.agent_statuses.apply_codex_user_submitted(pane_id, now)
    }

    /// Records an exact Ctrl-C gesture after it has been forwarded to the
    /// embedded terminal, clearing Codex state while suppressing late idle.
    pub fn record_terminal_interrupt(&mut self, pane_id: &str, now: u64) -> bool {
        self.agent_statuses
            .apply_codex_user_interrupted(pane_id, now)
    }

    /// Captures source-compatible restore drafts for active supported agents.
    ///
    /// Invalid or unsupported sessions are excluded rather than persisted as
    /// shell commands. Pane traversal order makes the snapshot deterministic.
    #[must_use]
    pub fn agent_restore_drafts(&self) -> Vec<PaneRestoreDraft> {
        self.worklanes
            .iter()
            .flat_map(|worklane| &worklane.columns)
            .flat_map(|column| &column.panes)
            .filter_map(|pane| {
                let status = self.agent_statuses.status_for_pane(&pane.id)?;
                let arguments = if status.agent_name.eq_ignore_ascii_case("codex") {
                    vec![
                        "codex".to_owned(),
                        "resume".to_owned(),
                        status.session_id.clone(),
                    ]
                } else if status.agent_name.eq_ignore_ascii_case("claude")
                    || status.agent_name.eq_ignore_ascii_case("claude code")
                {
                    vec![
                        "claude".to_owned(),
                        "--resume".to_owned(),
                        status.session_id.clone(),
                    ]
                } else if status.agent_name.eq_ignore_ascii_case("gemini")
                    || status.agent_name.eq_ignore_ascii_case("gemini cli")
                {
                    vec!["gemini".to_owned(), "--resume".to_owned()]
                } else {
                    return None;
                };
                let draft = PaneRestoreDraft {
                    pane_id: pane.id.clone(),
                    kind: RestoreDraftKind::AgentResume,
                    tool_name: status.agent_name.clone(),
                    session_id: status.session_id.clone(),
                    working_directory: pane.working_directory.clone(),
                    tracked_pid: status.tracked_pid.unwrap_or_default(),
                    agent_launch_snapshot: Some(AgentLaunchSnapshot {
                        arguments,
                        environment: None,
                    }),
                };
                draft.resume_command().is_some().then_some(draft)
            })
            .collect()
    }

    fn close_pane_at_with_capture(
        &mut self,
        pane_id: &str,
        now: u64,
        capture: bool,
    ) -> ClosePaneOutcome {
        let Some((worklane_index, column_index, pane_index)) = self
            .worklanes
            .iter()
            .enumerate()
            .find_map(|(worklane_index, worklane)| {
                worklane
                    .columns
                    .iter()
                    .enumerate()
                    .find_map(|(column_index, column)| {
                        column
                            .panes
                            .iter()
                            .position(|pane| pane.id == pane_id)
                            .map(|pane_index| (worklane_index, column_index, pane_index))
                    })
            })
        else {
            return ClosePaneOutcome::NotFound;
        };
        let pane_count: usize = self.worklanes[worklane_index]
            .columns
            .iter()
            .map(|column| column.panes.len())
            .sum();
        if pane_count == 1 {
            if self.worklanes.len() == 1 {
                return ClosePaneOutcome::CloseWindow;
            }
            if capture {
                self.capture_closed_pane(worklane_index, column_index, pane_index, now);
            }
            self.agent_statuses.remove_pane(pane_id);
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

        if capture {
            self.capture_closed_pane(worklane_index, column_index, pane_index, now);
        }
        self.agent_statuses.remove_pane(pane_id);
        let worklane = &mut self.worklanes[worklane_index];
        if worklane.columns[column_index].panes.len() == 1 {
            let removed_focused_column =
                worklane.columns[column_index].id == worklane.focused_column_id;
            worklane.columns.remove(column_index);
            if removed_focused_column {
                let replacement_index = column_index
                    .saturating_sub(1)
                    .min(worklane.columns.len() - 1);
                let replacement = &worklane.columns[replacement_index];
                worklane.focused_column_id.clone_from(&replacement.id);
            }
        } else {
            let column = &mut worklane.columns[column_index];
            let removed_focused_pane = column.focused_pane_id == pane_id;
            column.panes.remove(pane_index);
            let removed_height = column.pane_heights.remove(pane_index);
            let replacement_index = pane_index.saturating_sub(1).min(column.panes.len() - 1);
            column.pane_heights[replacement_index] += removed_height;
            if removed_focused_pane {
                column.focused_pane_id = column.panes[replacement_index].id.clone();
                column
                    .last_focused_pane_id
                    .clone_from(&column.focused_pane_id);
                worklane.focused_column_id.clone_from(&column.id);
            } else if column.last_focused_pane_id == pane_id {
                column
                    .last_focused_pane_id
                    .clone_from(&column.focused_pane_id);
            }
        }
        ClosePaneOutcome::Closed
    }

    /// Restores the most recently user-closed local pane using source LIFO and
    /// one-hour expiry semantics. A restored terminal receives a new identity.
    pub fn restore_closed_pane(&mut self, new_pane_id: impl Into<String>) -> Option<RestoredPane> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs());
        self.restore_closed_pane_at(new_pane_id, now)
    }

    /// Deterministic-clock form of [`Self::restore_closed_pane`].
    pub fn restore_closed_pane_at(
        &mut self,
        new_pane_id: impl Into<String>,
        now: u64,
    ) -> Option<RestoredPane> {
        let new_pane_id = new_pane_id.into();
        if self.pane(&new_pane_id).is_some() {
            return None;
        }
        self.prune_closed_panes(now);
        let entry = self.closed_panes.pop()?;
        let target_worklane_index = self
            .worklanes
            .iter()
            .position(|worklane| worklane.id == entry.worklane_id)
            .or_else(|| self.active_worklane_index())?;
        let target_worklane_id = self.worklanes[target_worklane_index].id.clone();
        let mut pane = entry.pane;
        pane.id.clone_from(&new_pane_id);

        let existing_column_index = self.worklanes[target_worklane_index]
            .columns
            .iter()
            .position(|column| column.id == entry.column_id);
        if let Some(column_index) = existing_column_index {
            let column = &mut self.worklanes[target_worklane_index].columns[column_index];
            let pane_index = entry.pane_index.min(column.panes.len());
            column.panes.insert(pane_index, pane);
            column.pane_heights.fill(1.0);
            column.pane_heights.insert(
                pane_index,
                entry
                    .pane_height
                    .filter(|height| *height > 0.0)
                    .unwrap_or(1.0),
            );
            column.focused_pane_id.clone_from(&new_pane_id);
            column.last_focused_pane_id.clone_from(&new_pane_id);
            self.worklanes[target_worklane_index]
                .focused_column_id
                .clone_from(&entry.column_id);
        } else {
            let column_id = self.unique_column_id(&new_pane_id);
            let column_index = entry
                .column_index
                .min(self.worklanes[target_worklane_index].columns.len());
            self.worklanes[target_worklane_index].columns.insert(
                column_index,
                PaneColumnState {
                    id: column_id.clone(),
                    width: entry.column_width,
                    panes: vec![pane],
                    pane_heights: vec![1.0],
                    focused_pane_id: new_pane_id.clone(),
                    last_focused_pane_id: new_pane_id.clone(),
                },
            );
            self.worklanes[target_worklane_index].focused_column_id = column_id;
        }
        self.active_worklane_id.clone_from(&target_worklane_id);
        let pane = self.pane(&new_pane_id)?;
        let restored = RestoredPane {
            pane_id: new_pane_id,
            worklane_id: target_worklane_id,
            working_directory: pane.working_directory.clone(),
            prefill_text: trimmed_owned(pane.last_run_command.as_deref()),
        };
        self.record_focus_transition(None);
        Some(restored)
    }

    fn capture_closed_pane(
        &mut self,
        worklane_index: usize,
        column_index: usize,
        pane_index: usize,
        now: u64,
    ) {
        self.prune_closed_panes(now);
        let worklane = &self.worklanes[worklane_index];
        let column = &worklane.columns[column_index];
        self.closed_panes.push(ClosedPaneEntry {
            closed_at: now,
            pane: column.panes[pane_index].clone(),
            worklane_id: worklane.id.clone(),
            column_id: column.id.clone(),
            column_index,
            pane_index,
            column_width: column.width,
            pane_height: (column.panes.len() > 1).then(|| column.pane_heights[pane_index]),
        });
        if self.closed_panes.len() > CLOSED_PANE_CAPACITY {
            self.closed_panes.remove(0);
        }
    }

    fn prune_closed_panes(&mut self, now: u64) {
        self.closed_panes
            .retain(|entry| now.saturating_sub(entry.closed_at) <= CLOSED_PANE_EXPIRY_SECONDS);
    }

    fn move_focused_pane_vertically(&mut self, direction: isize) -> bool {
        let worklane = self.active_worklane_mut();
        let column = worklane
            .columns
            .iter_mut()
            .find(|column| column.id == worklane.focused_column_id)
            .expect("workspace invariant: focused column exists");
        let focused_index = column
            .panes
            .iter()
            .position(|pane| pane.id == column.focused_pane_id)
            .expect("workspace invariant: focused pane exists");
        let Some(destination) = focused_index.checked_add_signed(direction) else {
            return false;
        };
        if destination >= column.panes.len() {
            return false;
        }
        column.panes.swap(focused_index, destination);
        column.pane_heights.swap(focused_index, destination);
        true
    }

    fn current_pane_reference(&self) -> Option<PaneReference> {
        self.focused_pane_id()
            .map(|pane_id| PaneReference::new(self.active_worklane_id.clone(), pane_id.to_owned()))
    }

    fn live_pane_references(&self) -> BTreeSet<PaneReference> {
        self.worklanes
            .iter()
            .flat_map(|worklane| {
                worklane
                    .columns
                    .iter()
                    .flat_map(|column| &column.panes)
                    .map(|pane| PaneReference::new(worklane.id.clone(), pane.id.clone()))
            })
            .collect()
    }

    fn record_focus_transition(&mut self, previous: Option<PaneReference>) {
        if self.is_navigating_history {
            return;
        }
        if let Some(previous) = previous
            && self.current_pane_reference().as_ref() != Some(&previous)
        {
            self.focus_history.record(previous);
        }
    }

    fn select_history_target(&mut self, target: &PaneReference) -> bool {
        self.is_navigating_history = true;
        let selected = self.select_worklane_and_pane(&target.worklane_id, &target.pane_id);
        self.is_navigating_history = false;
        selected
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
            .flat_map(|worklane| &worklane.columns)
            .flat_map(|column| &column.panes)
            .any(|pane| pane.id == id)
    }

    fn unique_column_id(&self, pane_id: &str) -> String {
        let base = format!("column-{pane_id}");
        if !self
            .worklanes
            .iter()
            .flat_map(|worklane| &worklane.columns)
            .any(|column| column.id == base)
        {
            return base;
        }
        let column_count: usize = self
            .worklanes
            .iter()
            .map(|worklane| worklane.columns.len())
            .sum();
        (2..=column_count + 2)
            .map(|suffix| format!("{base}-{suffix}"))
            .find(|candidate| {
                !self
                    .worklanes
                    .iter()
                    .flat_map(|worklane| &worklane.columns)
                    .any(|column| column.id == *candidate)
            })
            .expect("workspace invariant: available generated column identity")
    }
}

fn arrange_golden_column_width(
    worklane: &mut WorklaneState,
    column_id: &str,
    focus_wide: bool,
    available_width: f64,
) -> bool {
    if worklane.columns.len() < 2 || !available_width.is_finite() {
        return false;
    }
    let Some(focused_index) = worklane
        .columns
        .iter()
        .position(|column| column.id == column_id)
    else {
        return false;
    };
    let neighbor_index = if focused_index + 1 < worklane.columns.len() {
        focused_index + 1
    } else {
        focused_index - 1
    };
    let major = (1.0 + 5.0_f64.sqrt()) / (3.0 + 5.0_f64.sqrt());
    let focused_ratio = if focus_wide { major } else { 1.0 - major };
    let pair_width = (available_width - f64::from(PaneLayoutPolicy::INTER_PANE_SPACING)).max(1.0);
    let focused_width = pair_width * focused_ratio;
    let neighbor_width = pair_width - focused_width;
    let changed = (worklane.columns[focused_index].width - focused_width).abs() > f64::EPSILON
        || (worklane.columns[neighbor_index].width - neighbor_width).abs() > f64::EPSILON;
    worklane.columns[focused_index].width = focused_width;
    worklane.columns[neighbor_index].width = neighbor_width;
    changed
}

fn remove_pane(column: &mut PaneColumnState, pane_id: &str) -> (PaneState, f64) {
    let pane_index = column
        .panes
        .iter()
        .position(|pane| pane.id == pane_id)
        .expect("workspace invariant: focused pane exists");
    let pane = column.panes.remove(pane_index);
    let height = column.pane_heights.remove(pane_index);
    let replacement_index = pane_index
        .saturating_sub(1)
        .min(column.panes.len().saturating_sub(1));
    if let Some(replacement) = column.panes.get(replacement_index) {
        column.pane_heights[replacement_index] += height;
        column.focused_pane_id.clone_from(&replacement.id);
        column.last_focused_pane_id.clone_from(&replacement.id);
    }
    (pane, height)
}

fn sanitize_dimension(value: f64) -> f64 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        1.0
    }
}

fn trimmed_owned(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn small_count_as_f64(count: usize) -> f64 {
    f64::from(u32::try_from(count).unwrap_or(u32::MAX))
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
    EmptyColumn(String),
    DuplicateWorklane(String),
    DuplicateColumn(String),
    DuplicatePane(String),
}

impl fmt::Display for WorkspaceStateImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyWindow => formatter.write_str("workspace window has no worklanes"),
            Self::EmptyWorklane(id) => write!(formatter, "worklane {id} has no columns"),
            Self::EmptyColumn(id) => write!(formatter, "column {id} has no panes"),
            Self::DuplicateWorklane(id) => write!(formatter, "duplicate worklane ID: {id}"),
            Self::DuplicateColumn(id) => write!(formatter, "duplicate column ID: {id}"),
            Self::DuplicatePane(id) => write!(formatter, "duplicate pane ID: {id}"),
        }
    }
}

impl Error for WorkspaceStateImportError {}
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::{
    AgentLaunchSnapshot, AgentStatusStore, AuthenticatedAgentEvent, ColumnRecipe, PaneAgentStatus,
    PaneRecipe, PaneReference, PaneRestoreDraft, RestoreDraftKind, WindowRecipe, WorklaneRecipe,
    pane_focus_history::PaneFocusHistory,
};
