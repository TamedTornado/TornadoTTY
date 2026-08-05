use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TeamAnchor {
    pub leader_pane_id: String,
    pub column_pane_ids: Vec<String>,
    pub pre_team_leader_width: Option<u32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TeamTransition {
    FirstSplit,
    StackedSplit,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct TeamStore {
    version: u32,
    anchors: BTreeMap<String, TeamAnchor>,
    active_pane_ids: BTreeMap<String, String>,
    buffers: BTreeMap<String, String>,
}

impl TeamStore {
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
