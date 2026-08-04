use std::collections::BTreeSet;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PaneReference {
    pub worklane_id: String,
    pub pane_id: String,
}

impl PaneReference {
    #[must_use]
    pub fn new(worklane_id: impl Into<String>, pane_id: impl Into<String>) -> Self {
        Self {
            worklane_id: worklane_id.into(),
            pane_id: pane_id.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PaneFocusHistory {
    back_stack: Vec<PaneReference>,
    forward_stack: Vec<PaneReference>,
    max_depth: usize,
}

impl Default for PaneFocusHistory {
    fn default() -> Self {
        Self::new(100)
    }
}

impl PaneFocusHistory {
    fn new(max_depth: usize) -> Self {
        Self {
            back_stack: Vec::new(),
            forward_stack: Vec::new(),
            max_depth,
        }
    }

    pub(crate) fn can_go_back(&self) -> bool {
        !self.back_stack.is_empty()
    }

    pub(crate) fn can_go_forward(&self) -> bool {
        !self.forward_stack.is_empty()
    }

    pub(crate) fn record(&mut self, reference: PaneReference) {
        self.back_stack.push(reference);
        self.forward_stack.clear();
        if self.back_stack.len() > self.max_depth {
            self.back_stack
                .drain(..self.back_stack.len() - self.max_depth);
        }
    }

    pub(crate) fn recent_references(&self, live: &BTreeSet<PaneReference>) -> Vec<PaneReference> {
        let mut recent = Vec::new();
        for reference in self.back_stack.iter().rev() {
            if live.contains(reference) && !recent.contains(reference) {
                recent.push(reference.clone());
            }
        }
        recent
    }

    pub(crate) fn navigate_back(
        &mut self,
        current: PaneReference,
        live: &BTreeSet<PaneReference>,
    ) -> Option<PaneReference> {
        while let Some(reference) = self.back_stack.pop() {
            if live.contains(&reference) {
                self.forward_stack.push(current);
                return Some(reference);
            }
        }
        None
    }

    pub(crate) fn navigate_forward(
        &mut self,
        current: PaneReference,
        live: &BTreeSet<PaneReference>,
    ) -> Option<PaneReference> {
        while let Some(reference) = self.forward_stack.pop() {
            if live.contains(&reference) {
                self.back_stack.push(current);
                return Some(reference);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{PaneFocusHistory, PaneReference};

    const SOURCE: &str = include_str!("../../../Zentty/AppState/PaneFocusHistory.swift");

    #[test]
    fn browser_history_skips_closed_panes_clears_forward_and_caps_depth() {
        let one = PaneReference::new("lane-1", "pane-1");
        let two = PaneReference::new("lane-1", "pane-2");
        let three = PaneReference::new("lane-2", "pane-3");
        let four = PaneReference::new("lane-2", "pane-4");
        let live = BTreeSet::from([one.clone(), three.clone(), four.clone()]);
        let mut history = PaneFocusHistory::new(2);

        history.record(one.clone());
        history.record(two);
        history.record(three.clone());
        assert_eq!(history.navigate_back(four.clone(), &live), Some(three));
        assert_eq!(history.navigate_back(four.clone(), &live), None);
        assert_eq!(history.navigate_forward(one.clone(), &live), Some(four));

        history.record(one);
        assert!(!history.can_go_forward());

        assert!(SOURCE.contains("maxDepth: Int = 100"));
        assert!(SOURCE.contains("while let entry = backStack.popLast()"));
        assert!(SOURCE.contains("forwardStack.removeAll()"));
    }
}
