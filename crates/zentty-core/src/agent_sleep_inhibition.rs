#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SleepInhibitionTransition {
    None,
    Acquire,
    Release,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentSleepInhibitionState {
    release_debounce_ms: u64,
    lease_requested: bool,
    release_deadline_ms: Option<u64>,
    blocked_while_qualifying: bool,
}

impl AgentSleepInhibitionState {
    #[must_use]
    pub fn new(release_debounce_ms: u64) -> Self {
        Self {
            release_debounce_ms,
            lease_requested: false,
            release_deadline_ms: None,
            blocked_while_qualifying: false,
        }
    }

    #[must_use]
    pub fn update(
        &mut self,
        enabled: bool,
        has_running_agent: bool,
        now_ms: u64,
    ) -> SleepInhibitionTransition {
        let qualifies = enabled && has_running_agent;
        if qualifies {
            self.release_deadline_ms = None;
            if self.blocked_while_qualifying || self.lease_requested {
                return SleepInhibitionTransition::None;
            }
            self.lease_requested = true;
            return SleepInhibitionTransition::Acquire;
        }

        self.blocked_while_qualifying = false;
        if !enabled {
            self.release_deadline_ms = None;
            return self.release_now();
        }
        if !self.lease_requested {
            self.release_deadline_ms = None;
            return SleepInhibitionTransition::None;
        }

        let deadline = *self
            .release_deadline_ms
            .get_or_insert_with(|| now_ms.saturating_add(self.release_debounce_ms));
        if now_ms < deadline {
            SleepInhibitionTransition::None
        } else {
            self.release_deadline_ms = None;
            self.release_now()
        }
    }

    pub fn mark_backend_lost(&mut self, still_qualifies: bool) {
        self.lease_requested = false;
        self.release_deadline_ms = None;
        self.blocked_while_qualifying = still_qualifies;
    }

    #[must_use]
    pub fn force_release(&mut self) -> SleepInhibitionTransition {
        self.blocked_while_qualifying = false;
        self.release_deadline_ms = None;
        self.release_now()
    }

    #[must_use]
    pub fn lease_requested(&self) -> bool {
        self.lease_requested
    }

    #[must_use]
    pub fn release_deadline_ms(&self) -> Option<u64> {
        self.release_deadline_ms
    }

    fn release_now(&mut self) -> SleepInhibitionTransition {
        if !self.lease_requested {
            return SleepInhibitionTransition::None;
        }
        self.lease_requested = false;
        SleepInhibitionTransition::Release
    }
}

impl Default for AgentSleepInhibitionState {
    fn default() -> Self {
        Self::new(10_000)
    }
}
