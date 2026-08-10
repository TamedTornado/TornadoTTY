use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct GlobalSearchTarget {
    pub worklane_id: String,
    pub pane_id: String,
}

impl GlobalSearchTarget {
    #[must_use]
    pub fn new(worklane_id: impl Into<String>, pane_id: impl Into<String>) -> Self {
        Self {
            worklane_id: worklane_id.into(),
            pane_id: pane_id.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GlobalSearchDirection {
    Next,
    Previous,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GlobalSearchEffect {
    Start {
        target: GlobalSearchTarget,
        needle: String,
    },
    End {
        pane_id: String,
    },
    ResetSelection {
        pane_id: String,
    },
    Navigate {
        target: GlobalSearchTarget,
        direction: GlobalSearchDirection,
        selected_index: usize,
    },
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GlobalSearchState {
    pub needle: String,
    pub selected: Option<usize>,
    pub total: usize,
    pub has_remembered_search: bool,
    pub visible: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct PaneResultState {
    total: usize,
    selected: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Selection {
    pane_id: String,
    index: usize,
}

#[derive(Debug, Default)]
pub struct GlobalSearchCoordinator {
    state: GlobalSearchState,
    frozen_targets: Vec<GlobalSearchTarget>,
    pane_results: BTreeMap<String, PaneResultState>,
    pending_totals: BTreeSet<String>,
    current_selection: Option<Selection>,
    pending_navigation: Option<GlobalSearchDirection>,
    pending_query: Option<String>,
}

impl GlobalSearchCoordinator {
    pub fn show(&mut self, targets: &[GlobalSearchTarget]) {
        if !self.state.has_remembered_search {
            self.capture_targets(targets);
            self.clear_results();
        }
        self.state.visible = true;
    }

    #[must_use]
    pub fn state(&self) -> &GlobalSearchState {
        &self.state
    }

    #[must_use]
    pub fn frozen_targets(&self) -> &[GlobalSearchTarget] {
        &self.frozen_targets
    }

    #[must_use]
    pub fn has_pending_query(&self) -> bool {
        self.pending_query.is_some()
    }

    #[must_use]
    pub fn update_query(
        &mut self,
        needle: &str,
        targets: &[GlobalSearchTarget],
    ) -> Vec<GlobalSearchEffect> {
        self.capture_targets(targets);
        self.clear_results();
        self.pending_navigation = None;
        self.pending_query = None;
        needle.clone_into(&mut self.state.needle);
        self.state.visible = true;

        if needle.is_empty() {
            self.state.has_remembered_search = false;
            self.pending_totals.clear();
            return self.end_effects();
        }

        self.state.has_remembered_search = true;
        if needle.chars().count() >= 3 {
            self.dispatch_query()
        } else {
            self.pending_query = Some(needle.to_owned());
            Vec::new()
        }
    }

    #[must_use]
    pub fn dispatch_pending_query(&mut self) -> Vec<GlobalSearchEffect> {
        if self.pending_query.take().is_some() {
            self.dispatch_query()
        } else {
            Vec::new()
        }
    }

    #[must_use]
    pub fn find_next(
        &mut self,
        current_target: Option<&GlobalSearchTarget>,
    ) -> Vec<GlobalSearchEffect> {
        self.navigate(GlobalSearchDirection::Next, current_target)
    }

    #[must_use]
    pub fn find_previous(
        &mut self,
        current_target: Option<&GlobalSearchTarget>,
    ) -> Vec<GlobalSearchEffect> {
        self.navigate(GlobalSearchDirection::Previous, current_target)
    }

    #[must_use]
    pub fn handle_total(&mut self, pane_id: &str, total: usize) -> Vec<GlobalSearchEffect> {
        let Some(result) = self.pane_results.get_mut(pane_id) else {
            return Vec::new();
        };
        result.total = total;
        self.pending_totals.remove(pane_id);
        if self
            .current_selection
            .as_ref()
            .is_some_and(|selection| selection.pane_id == pane_id && selection.index >= total)
        {
            self.current_selection = None;
            result.selected = None;
            self.state.selected = None;
        }
        self.recompute_total();
        self.perform_pending_navigation_if_ready()
    }

    pub fn handle_selected(&mut self, pane_id: &str, selected: Option<usize>) {
        let Some(pane_state) = self.pane_results.get(pane_id) else {
            return;
        };
        if selected.is_some_and(|index| index >= pane_state.total) {
            return;
        }
        for result in self.pane_results.values_mut() {
            result.selected = None;
        }
        let Some(selected) = selected else {
            if self
                .current_selection
                .as_ref()
                .is_some_and(|selection| selection.pane_id == pane_id)
            {
                self.current_selection = None;
                self.state.selected = None;
            }
            return;
        };
        if let Some(result) = self.pane_results.get_mut(pane_id) {
            result.selected = Some(selected);
        }
        self.current_selection = Some(Selection {
            pane_id: pane_id.to_owned(),
            index: selected,
        });
        self.state.selected = self.global_ordinal(pane_id, selected);
    }

    #[must_use]
    pub fn reconcile_live_panes<'a>(
        &mut self,
        live_pane_ids: impl IntoIterator<Item = &'a str>,
    ) -> Vec<GlobalSearchEffect> {
        if !self.state.has_remembered_search {
            return Vec::new();
        }
        let live = live_pane_ids.into_iter().collect::<BTreeSet<_>>();
        let removed = self
            .frozen_targets
            .iter()
            .filter(|target| !live.contains(target.pane_id.as_str()))
            .map(|target| target.pane_id.clone())
            .collect::<Vec<_>>();
        self.frozen_targets
            .retain(|target| live.contains(target.pane_id.as_str()));
        for pane_id in &removed {
            self.pane_results.remove(pane_id);
            self.pending_totals.remove(pane_id);
        }
        if self
            .current_selection
            .as_ref()
            .is_some_and(|selection| !self.pane_results.contains_key(&selection.pane_id))
        {
            self.current_selection = None;
            self.state.selected = None;
        }
        self.recompute_total();
        if let Some(selection) = &self.current_selection {
            self.state.selected = self.global_ordinal(&selection.pane_id, selection.index);
        }
        let mut effects = removed
            .into_iter()
            .map(|pane_id| GlobalSearchEffect::End { pane_id })
            .collect::<Vec<_>>();
        effects.extend(self.perform_pending_navigation_if_ready());
        effects
    }

    #[must_use]
    pub fn end(&mut self) -> Vec<GlobalSearchEffect> {
        let effects = self.end_effects();
        *self = Self::default();
        effects
    }

    fn navigate(
        &mut self,
        direction: GlobalSearchDirection,
        _current_target: Option<&GlobalSearchTarget>,
    ) -> Vec<GlobalSearchEffect> {
        if !self.state.has_remembered_search {
            return Vec::new();
        }
        self.state.visible = true;
        let effects = self.dispatch_pending_query();
        if !effects.is_empty() {
            self.pending_navigation = Some(direction);
            return effects;
        }
        if !self.pending_totals.is_empty() {
            self.pending_navigation = Some(direction);
            return Vec::new();
        }
        self.navigation_effects(direction)
    }

    fn navigation_effects(&mut self, direction: GlobalSearchDirection) -> Vec<GlobalSearchEffect> {
        if self.state.total == 0 {
            return Vec::new();
        }
        let matching_targets = self
            .frozen_targets
            .iter()
            .filter(|target| {
                self.pane_results
                    .get(&target.pane_id)
                    .is_some_and(|result| result.total > 0)
            })
            .cloned()
            .collect::<Vec<_>>();
        if matching_targets.is_empty() {
            return Vec::new();
        }

        let Some(selection) = self.current_selection.clone() else {
            let target = match direction {
                GlobalSearchDirection::Next => matching_targets.first(),
                GlobalSearchDirection::Previous => matching_targets.last(),
            }
            .expect("nonempty matching target set");
            let selected_index = match direction {
                GlobalSearchDirection::Next => 0,
                GlobalSearchDirection::Previous => self.pane_results[&target.pane_id].total - 1,
            };
            return vec![GlobalSearchEffect::Navigate {
                target: target.clone(),
                direction,
                selected_index,
            }];
        };

        let Some(target_index) = matching_targets
            .iter()
            .position(|target| target.pane_id == selection.pane_id)
        else {
            self.current_selection = None;
            self.state.selected = None;
            return self.navigation_effects(direction);
        };
        let pane_total = self.pane_results[&selection.pane_id].total;
        let within_index = match direction {
            GlobalSearchDirection::Next if selection.index + 1 < pane_total => {
                Some(selection.index + 1)
            }
            GlobalSearchDirection::Previous if selection.index > 0 => Some(selection.index - 1),
            _ => None,
        };
        if let Some(selected_index) = within_index {
            return vec![GlobalSearchEffect::Navigate {
                target: matching_targets[target_index].clone(),
                direction,
                selected_index,
            }];
        }

        let destination_index = match direction {
            GlobalSearchDirection::Next => (target_index + 1) % matching_targets.len(),
            GlobalSearchDirection::Previous => {
                (target_index + matching_targets.len() - 1) % matching_targets.len()
            }
        };
        let destination = matching_targets[destination_index].clone();
        let selected_index = match direction {
            GlobalSearchDirection::Next => 0,
            GlobalSearchDirection::Previous => self.pane_results[&destination.pane_id].total - 1,
        };
        let mut effects = Vec::new();
        if destination.pane_id != selection.pane_id {
            effects.push(GlobalSearchEffect::ResetSelection {
                pane_id: selection.pane_id,
            });
        }
        effects.push(GlobalSearchEffect::Navigate {
            target: destination,
            direction,
            selected_index,
        });
        effects
    }

    fn perform_pending_navigation_if_ready(&mut self) -> Vec<GlobalSearchEffect> {
        if !self.pending_totals.is_empty() || self.state.total == 0 {
            return Vec::new();
        }
        self.pending_navigation
            .take()
            .map_or_else(Vec::new, |direction| self.navigation_effects(direction))
    }

    fn capture_targets(&mut self, targets: &[GlobalSearchTarget]) {
        self.frozen_targets = targets.to_vec();
        for target in targets {
            self.pane_results.entry(target.pane_id.clone()).or_default();
        }
    }

    fn clear_results(&mut self) {
        self.pane_results = self
            .frozen_targets
            .iter()
            .map(|target| (target.pane_id.clone(), PaneResultState::default()))
            .collect();
        self.pending_totals.clear();
        self.current_selection = None;
        self.state.selected = None;
        self.state.total = 0;
    }

    fn dispatch_query(&mut self) -> Vec<GlobalSearchEffect> {
        self.pending_query = None;
        self.pending_totals = self
            .frozen_targets
            .iter()
            .map(|target| target.pane_id.clone())
            .collect();
        self.frozen_targets
            .iter()
            .cloned()
            .map(|target| GlobalSearchEffect::Start {
                target,
                needle: self.state.needle.clone(),
            })
            .collect()
    }

    fn end_effects(&self) -> Vec<GlobalSearchEffect> {
        self.frozen_targets
            .iter()
            .map(|target| GlobalSearchEffect::End {
                pane_id: target.pane_id.clone(),
            })
            .collect()
    }

    fn recompute_total(&mut self) {
        self.state.total = self.pane_results.values().map(|result| result.total).sum();
        if self.state.total == 0 {
            self.state.selected = None;
        }
    }

    fn global_ordinal(&self, pane_id: &str, selected_index: usize) -> Option<usize> {
        let mut offset = 0;
        for target in &self.frozen_targets {
            let result = self.pane_results.get(&target.pane_id)?;
            if target.pane_id == pane_id {
                return Some(offset + selected_index);
            }
            offset += result.total;
        }
        None
    }
}
