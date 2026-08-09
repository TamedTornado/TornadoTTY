#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CloseWindowDecision {
    KeepRunning,
    QuitApplication,
    UnknownWindow,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum WindowSetError {
    EmptyId,
    DuplicateId(String),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct WindowSet {
    ordered_ids: Vec<String>,
    active_id: Option<String>,
    next_generated_number: u64,
}

impl WindowSet {
    pub(crate) fn restore(
        ordered_ids: impl IntoIterator<Item = String>,
        requested_active_id: Option<&str>,
    ) -> Result<Self, WindowSetError> {
        let mut set = Self::default();
        for id in ordered_ids {
            set.insert(id)?;
        }
        set.active_id = requested_active_id
            .filter(|id| set.contains(id))
            .map(str::to_owned)
            .or_else(|| set.ordered_ids.first().cloned());
        Ok(set)
    }

    pub(crate) fn insert(&mut self, id: String) -> Result<(), WindowSetError> {
        if id.is_empty() {
            return Err(WindowSetError::EmptyId);
        }
        if self.contains(&id) {
            return Err(WindowSetError::DuplicateId(id));
        }
        if self.active_id.is_none() {
            self.active_id = Some(id.clone());
        }
        self.ordered_ids.push(id);
        Ok(())
    }

    pub(crate) fn generate_id(&mut self) -> String {
        loop {
            self.next_generated_number = self.next_generated_number.wrapping_add(1);
            let id = format!("window-{}", self.next_generated_number);
            if !self.contains(&id) {
                return id;
            }
        }
    }

    pub(crate) fn mark_active(&mut self, id: &str) -> bool {
        if !self.contains(id) || self.active_id.as_deref() == Some(id) {
            return false;
        }
        self.active_id = Some(id.to_owned());
        true
    }

    pub(crate) fn close(&mut self, id: &str) -> CloseWindowDecision {
        let Some(index) = self
            .ordered_ids
            .iter()
            .position(|candidate| candidate == id)
        else {
            return CloseWindowDecision::UnknownWindow;
        };
        self.ordered_ids.remove(index);
        if self.ordered_ids.is_empty() {
            self.active_id = None;
            return CloseWindowDecision::QuitApplication;
        }
        if self.active_id.as_deref() == Some(id) {
            self.active_id = self
                .ordered_ids
                .get(index.min(self.ordered_ids.len() - 1))
                .cloned();
        }
        CloseWindowDecision::KeepRunning
    }

    pub(crate) fn ordered_ids(&self) -> &[String] {
        &self.ordered_ids
    }

    pub(crate) fn active_id(&self) -> Option<&str> {
        self.active_id.as_deref()
    }

    pub(crate) fn contains(&self, id: &str) -> bool {
        self.ordered_ids.iter().any(|candidate| candidate == id)
    }

    #[cfg(test)]
    fn invariants_hold(&self) -> bool {
        let unique = self
            .ordered_ids
            .iter()
            .collect::<std::collections::BTreeSet<_>>();
        unique.len() == self.ordered_ids.len()
            && self.active_id.as_deref().is_none_or(|id| self.contains(id))
    }
}

#[cfg(test)]
mod tests {
    use super::{CloseWindowDecision, WindowSet, WindowSetError};

    #[test]
    fn restore_preserves_order_and_requested_active_window() {
        let set = WindowSet::restore(
            ["window-a".to_owned(), "window-b".to_owned()],
            Some("window-b"),
        )
        .unwrap();

        assert_eq!(set.ordered_ids(), ["window-a", "window-b"]);
        assert_eq!(set.active_id(), Some("window-b"));
        assert!(set.invariants_hold());
    }

    #[test]
    fn restore_falls_back_to_first_window_for_stale_active_id() {
        let set = WindowSet::restore(
            ["window-a".to_owned(), "window-b".to_owned()],
            Some("missing"),
        )
        .unwrap();

        assert_eq!(set.active_id(), Some("window-a"));
        assert!(set.invariants_hold());
    }

    #[test]
    fn duplicate_and_empty_ids_are_rejected_without_mutation() {
        let mut set = WindowSet::restore(["window-a".to_owned()], None).unwrap();

        assert_eq!(set.insert(String::new()), Err(WindowSetError::EmptyId));
        assert_eq!(
            set.insert("window-a".to_owned()),
            Err(WindowSetError::DuplicateId("window-a".to_owned()))
        );
        assert_eq!(set.ordered_ids(), ["window-a"]);
        assert!(set.invariants_hold());
    }

    #[test]
    fn generated_ids_skip_restored_collisions() {
        let mut set = WindowSet::restore(["window-1".to_owned()], None).unwrap();

        assert_eq!(set.generate_id(), "window-2");
        assert!(set.invariants_hold());
    }

    #[test]
    fn closing_active_window_selects_adjacent_survivor() {
        let mut set = WindowSet::restore(
            [
                "window-a".to_owned(),
                "window-b".to_owned(),
                "window-c".to_owned(),
            ],
            Some("window-b"),
        )
        .unwrap();

        assert_eq!(set.close("window-b"), CloseWindowDecision::KeepRunning);
        assert_eq!(set.ordered_ids(), ["window-a", "window-c"]);
        assert_eq!(set.active_id(), Some("window-c"));
        assert!(set.invariants_hold());
    }

    #[test]
    fn closing_inactive_window_preserves_active_window() {
        let mut set = WindowSet::restore(
            ["window-a".to_owned(), "window-b".to_owned()],
            Some("window-b"),
        )
        .unwrap();

        assert_eq!(set.close("window-a"), CloseWindowDecision::KeepRunning);
        assert_eq!(set.active_id(), Some("window-b"));
        assert!(set.invariants_hold());
    }

    #[test]
    fn closing_last_active_window_selects_the_previous_survivor() {
        let mut set = WindowSet::restore(
            [
                "window-a".to_owned(),
                "window-b".to_owned(),
                "window-c".to_owned(),
            ],
            Some("window-c"),
        )
        .unwrap();

        assert_eq!(set.close("window-c"), CloseWindowDecision::KeepRunning);
        assert_eq!(set.ordered_ids(), ["window-a", "window-b"]);
        assert_eq!(set.active_id(), Some("window-b"));
        assert!(set.invariants_hold());
    }

    #[test]
    fn final_and_unknown_close_decisions_are_distinct() {
        let mut set = WindowSet::restore(["window-a".to_owned()], None).unwrap();

        assert_eq!(set.close("missing"), CloseWindowDecision::UnknownWindow);
        assert_eq!(set.close("window-a"), CloseWindowDecision::QuitApplication);
        assert_eq!(set.close("window-a"), CloseWindowDecision::UnknownWindow);
        assert_eq!(set.active_id(), None);
        assert!(set.invariants_hold());
    }

    #[test]
    fn only_known_window_can_become_active() {
        let mut set =
            WindowSet::restore(["window-a".to_owned(), "window-b".to_owned()], None).unwrap();

        assert!(!set.mark_active("missing"));
        assert!(set.mark_active("window-b"));
        assert!(!set.mark_active("window-b"));
        assert_eq!(set.active_id(), Some("window-b"));
        assert!(set.invariants_hold());
    }
}
