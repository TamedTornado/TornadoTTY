use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamAnchor {
    #[serde(rename = "leaderPaneID")]
    pub leader_pane_id: String,
    #[serde(rename = "columnPaneIDs")]
    pub column_pane_ids: Vec<String>,
    pub pre_team_leader_width: Option<u32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TeamTransition {
    FirstSplit,
    StackedSplit,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamStore {
    version: u32,
    anchors: BTreeMap<String, TeamAnchor>,
    #[serde(rename = "activePaneIDs")]
    active_pane_ids: BTreeMap<String, String>,
    buffers: BTreeMap<String, String>,
}

impl Default for TeamStore {
    fn default() -> Self {
        Self {
            version: 1,
            anchors: BTreeMap::new(),
            active_pane_ids: BTreeMap::new(),
            buffers: BTreeMap::new(),
        }
    }
}

impl TeamStore {
    pub const MAX_BUFFER_BYTES: usize = 256 * 1024;
    pub const MAX_STORE_BYTES: usize = 1024 * 1024;
    pub const MAX_IDENTIFIERS: usize = 256;
    pub const MAX_IDENTIFIER_BYTES: usize = 256;

    /// Decodes and validates compatibility state without performing file I/O.
    ///
    /// # Errors
    ///
    /// Rejects malformed JSON, unsupported versions, or bounded-field limits.
    pub fn from_json(bytes: &[u8]) -> Result<Self, StoreError> {
        if bytes.len() > Self::MAX_STORE_BYTES {
            return Err(StoreError::LimitExceeded);
        }
        let store: Self = serde_json::from_slice(bytes)
            .map_err(|error| StoreError::InvalidJson(error.to_string()))?;
        store.validate()?;
        Ok(store)
    }

    /// Encodes validated compatibility state using source-compatible field
    /// names.
    ///
    /// # Errors
    ///
    /// Rejects unsupported versions, bounded-field violations, or JSON errors.
    pub fn to_json(&self) -> Result<Vec<u8>, StoreError> {
        self.validate()?;
        let encoded = serde_json::to_vec_pretty(self)
            .map_err(|error| StoreError::InvalidJson(error.to_string()))?;
        if encoded.len() > Self::MAX_STORE_BYTES {
            return Err(StoreError::LimitExceeded);
        }
        Ok(encoded)
    }

    fn validate(&self) -> Result<(), StoreError> {
        if self.version != 1 {
            return Err(StoreError::UnsupportedVersion(self.version));
        }
        if self.anchors.len() > Self::MAX_IDENTIFIERS
            || self.active_pane_ids.len() > Self::MAX_IDENTIFIERS
            || self.buffers.len() > Self::MAX_IDENTIFIERS
        {
            return Err(StoreError::LimitExceeded);
        }
        for (worklane_id, anchor) in &self.anchors {
            validate_identifier(worklane_id)?;
            validate_identifier(&anchor.leader_pane_id)?;
            if anchor.column_pane_ids.len() > Self::MAX_IDENTIFIERS {
                return Err(StoreError::LimitExceeded);
            }
            for pane_id in &anchor.column_pane_ids {
                validate_identifier(pane_id)?;
            }
        }
        for (worklane_id, pane_id) in &self.active_pane_ids {
            validate_identifier(worklane_id)?;
            validate_identifier(pane_id)?;
        }
        for (name, value) in &self.buffers {
            validate_identifier(name)?;
            if value.len() > Self::MAX_BUFFER_BYTES {
                return Err(StoreError::LimitExceeded);
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn record_split(
        &mut self,
        worklane_id: &str,
        leader_pane_id: &str,
        new_pane_id: &str,
        detached: bool,
        pre_team_leader_width: Option<u32>,
    ) -> TeamTransition {
        let transition = if let Some(anchor) = self.anchors.get_mut(worklane_id) {
            anchor.column_pane_ids.push(new_pane_id.to_owned());
            TeamTransition::StackedSplit
        } else {
            self.anchors.insert(
                worklane_id.to_owned(),
                TeamAnchor {
                    leader_pane_id: leader_pane_id.to_owned(),
                    column_pane_ids: vec![new_pane_id.to_owned()],
                    pre_team_leader_width,
                },
            );
            TeamTransition::FirstSplit
        };
        if !detached {
            self.active_pane_ids
                .insert(worklane_id.to_owned(), new_pane_id.to_owned());
        }
        transition
    }

    /// Removes a teammate and returns the leader width to restore when that
    /// teammate dissolved the team column.
    #[must_use]
    pub fn remove_pane(&mut self, worklane_id: &str, pane_id: &str) -> Option<u32> {
        if self.active_pane_ids.get(worklane_id).map(String::as_str) == Some(pane_id) {
            self.active_pane_ids.remove(worklane_id);
        }
        let anchor = self.anchors.get_mut(worklane_id)?;
        if anchor.leader_pane_id == pane_id {
            self.anchors.remove(worklane_id);
            return None;
        }
        anchor.column_pane_ids.retain(|member| member != pane_id);
        if anchor.column_pane_ids.is_empty() {
            let width = anchor.pre_team_leader_width;
            self.anchors.remove(worklane_id);
            return width;
        }
        None
    }

    #[must_use]
    pub fn anchor(&self, worklane_id: &str) -> Option<&TeamAnchor> {
        self.anchors.get(worklane_id)
    }

    #[must_use]
    pub fn active_pane(&self, worklane_id: &str) -> Option<&str> {
        self.active_pane_ids.get(worklane_id).map(String::as_str)
    }
}

fn validate_identifier(value: &str) -> Result<(), StoreError> {
    if value.is_empty() || value.len() > TeamStore::MAX_IDENTIFIER_BYTES {
        Err(StoreError::LimitExceeded)
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StoreError {
    InvalidJson(String),
    UnsupportedVersion(u32),
    LimitExceeded,
}

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson(error) => {
                write!(formatter, "invalid tmux compatibility store: {error}")
            }
            Self::UnsupportedVersion(version) => {
                write!(
                    formatter,
                    "unsupported tmux compatibility store version: {version}"
                )
            }
            Self::LimitExceeded => formatter.write_str("tmux compatibility store limit exceeded"),
        }
    }
}

impl std::error::Error for StoreError {}
