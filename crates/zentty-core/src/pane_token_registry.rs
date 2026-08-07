use crate::AgentEvent;
use std::fmt;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AgentTarget {
    pub window_id: String,
    pub worklane_id: String,
    pub pane_id: String,
}

impl AgentTarget {
    #[must_use]
    pub fn new(
        window_id: impl Into<String>,
        worklane_id: impl Into<String>,
        pane_id: impl Into<String>,
    ) -> Self {
        Self {
            window_id: window_id.into(),
            worklane_id: worklane_id.into(),
            pane_id: pane_id.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthenticatedAgentEvent {
    pub target: AgentTarget,
    pub pane_token: String,
    pub event: AgentEvent,
}

impl AuthenticatedAgentEvent {
    /// Returns the canonical protocol event name after authentication.
    #[must_use]
    pub fn event_kind(&self) -> &'static str {
        self.event.kind()
    }

    /// Returns the agent session identity carried by the authenticated event.
    #[must_use]
    pub fn session_id(&self) -> Option<&str> {
        self.event.session_id()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PaneTokenError {
    EmptyToken,
    DuplicateToken,
    InvalidToken,
}

impl fmt::Display for PaneTokenError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyToken => formatter.write_str("pane token must not be empty"),
            Self::DuplicateToken => formatter.write_str("pane token is already registered"),
            Self::InvalidToken => formatter.write_str("pane token is invalid"),
        }
    }
}

impl std::error::Error for PaneTokenError {}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PaneTokenRegistry {
    entries: Vec<(String, AgentTarget)>,
}

impl PaneTokenRegistry {
    /// Registers an opaque token as the sole authority for a canonical pane.
    ///
    /// # Errors
    ///
    /// Returns an error when the token is empty or already registered.
    pub fn register(
        &mut self,
        token: impl Into<String>,
        target: AgentTarget,
    ) -> Result<(), PaneTokenError> {
        let token = token.into();
        if token.is_empty() {
            return Err(PaneTokenError::EmptyToken);
        }
        if self
            .entries
            .iter()
            .any(|(candidate, _)| constant_time_eq(candidate.as_bytes(), token.as_bytes()))
        {
            return Err(PaneTokenError::DuplicateToken);
        }
        self.entries.push((token, target));
        Ok(())
    }

    /// Resolves an event to the pane owned by its opaque token.
    ///
    /// # Errors
    ///
    /// Returns an error when the token is not registered.
    pub fn authenticate(
        &self,
        token: &str,
        event: AgentEvent,
    ) -> Result<AuthenticatedAgentEvent, PaneTokenError> {
        let target = self.authenticate_target(token)?;
        Ok(AuthenticatedAgentEvent {
            target,
            pane_token: token.to_owned(),
            event,
        })
    }

    /// Resolves a pane capability to its server-canonical target.
    ///
    /// This is the common authentication primitive for event and command
    /// protocols; forwarded window, worklane, and pane identifiers are never
    /// consulted.
    ///
    /// # Errors
    ///
    /// Returns an error when the token is not registered.
    pub fn authenticate_target(&self, token: &str) -> Result<AgentTarget, PaneTokenError> {
        self.entries
            .iter()
            .find(|(candidate, _)| constant_time_eq(candidate.as_bytes(), token.as_bytes()))
            .map(|(_, target)| target.clone())
            .ok_or(PaneTokenError::InvalidToken)
    }

    /// Changes the canonical target owned by an already registered token.
    ///
    /// # Errors
    ///
    /// Returns an error when the token is not registered.
    pub fn retarget(&mut self, token: &str, target: AgentTarget) -> Result<(), PaneTokenError> {
        let (_, stored_target) = self
            .entries
            .iter_mut()
            .find(|(candidate, _)| constant_time_eq(candidate.as_bytes(), token.as_bytes()))
            .ok_or(PaneTokenError::InvalidToken)?;
        *stored_target = target;
        Ok(())
    }

    pub fn unregister(&mut self, token: &str) -> bool {
        let Some(index) = self
            .entries
            .iter()
            .position(|(candidate, _)| constant_time_eq(candidate.as_bytes(), token.as_bytes()))
        else {
            return false;
        };
        self.entries.remove(index);
        true
    }
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let maximum = left.len().max(right.len());
    let mut difference = left.len() ^ right.len();
    for index in 0..maximum {
        difference |= usize::from(*left.get(index).unwrap_or(&0) ^ *right.get(index).unwrap_or(&0));
    }
    difference == 0
}
