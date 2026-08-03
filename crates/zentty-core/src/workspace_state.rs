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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaneState {
    pub id: String,
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
                }],
                focused_pane_id: pane_id,
            }],
            active_worklane_id: worklane_id,
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

    /// Closes the focused pane or requests window closure for the last pane.
    ///
    /// # Panics
    ///
    /// Panics only if an internal state transition has violated active-lane,
    /// focused-pane, or non-empty-workspace invariants.
    pub fn close_focused_pane(&mut self) -> ClosePaneOutcome {
        if self.active_worklane().panes.len() == 1 {
            if self.worklanes.len() == 1 {
                return ClosePaneOutcome::CloseWindow;
            }
            self.close_active_worklane();
            return ClosePaneOutcome::Closed;
        }
        let worklane = self.active_worklane_mut();
        let focused_index = worklane
            .panes
            .iter()
            .position(|pane| pane.id == worklane.focused_pane_id)
            .expect("workspace invariant: focused pane exists");
        worklane.panes.remove(focused_index);
        let replacement_index = focused_index
            .saturating_sub(1)
            .min(worklane.panes.len() - 1);
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
}
