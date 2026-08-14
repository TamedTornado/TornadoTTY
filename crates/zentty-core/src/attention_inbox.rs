use crate::{AgentInteractionKind, PaneAgentStatus};
use std::collections::{HashMap, HashSet};

const MAX_ITEMS: usize = 50;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AttentionTarget {
    pub window_id: String,
    pub worklane_id: String,
    pub pane_id: String,
}

impl AttentionTarget {
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
pub struct AttentionItem {
    pub id: u64,
    pub target: AttentionTarget,
    pub agent_name: String,
    pub interaction: AgentInteractionKind,
    pub status_text: String,
    pub primary_text: String,
    pub created_at_ms: u64,
    pub resolved_at_ms: Option<u64>,
}

impl AttentionItem {
    #[must_use]
    pub fn is_resolved(&self) -> bool {
        self.resolved_at_ms.is_some()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AttentionSignature {
    agent_name: String,
    interaction: AgentInteractionKind,
    text: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AttentionInbox {
    items: Vec<AttentionItem>,
    signatures: HashMap<AttentionTarget, AttentionSignature>,
    next_id: u64,
}

impl AttentionInbox {
    #[must_use]
    pub fn items(&self) -> &[AttentionItem] {
        &self.items
    }

    #[must_use]
    pub fn unresolved_count(&self) -> usize {
        self.items.iter().filter(|item| !item.is_resolved()).count()
    }

    #[must_use]
    pub fn most_urgent_unresolved(&self) -> Option<&AttentionItem> {
        self.items.iter().find(|item| !item.is_resolved())
    }

    pub fn observe(
        &mut self,
        target: AttentionTarget,
        status: Option<&PaneAgentStatus>,
        now_ms: u64,
    ) -> bool {
        let Some(status) = status.filter(|status| status.requires_attention()) else {
            self.signatures.remove(&target);
            return self.resolve_target(&target, now_ms);
        };
        let signature = AttentionSignature {
            agent_name: status.agent_name.clone(),
            interaction: status.interaction,
            text: status.text.clone(),
        };
        if self.signatures.get(&target) == Some(&signature) {
            return false;
        }

        self.resolve_target(&target, now_ms);
        self.signatures.insert(target.clone(), signature);
        self.next_id = self.next_id.saturating_add(1);
        self.items.insert(
            0,
            AttentionItem {
                id: self.next_id,
                target,
                agent_name: status.agent_name.clone(),
                interaction: status.interaction,
                status_text: interaction_status(status.interaction).to_owned(),
                primary_text: status
                    .text
                    .clone()
                    .unwrap_or_else(|| interaction_fallback(status.interaction).to_owned()),
                created_at_ms: now_ms,
                resolved_at_ms: None,
            },
        );
        self.items.truncate(MAX_ITEMS);
        true
    }

    pub fn resolve_target(&mut self, target: &AttentionTarget, now_ms: u64) -> bool {
        let mut changed = false;
        for item in &mut self.items {
            if item.target == *target && !item.is_resolved() {
                item.resolved_at_ms = Some(now_ms);
                changed = true;
            }
        }
        changed
    }

    pub fn resolve_stale(
        &mut self,
        window_id: &str,
        live_targets: &HashSet<AttentionTarget>,
        now_ms: u64,
    ) -> bool {
        self.signatures
            .retain(|target, _| target.window_id != window_id || live_targets.contains(target));
        let mut changed = false;
        for item in &mut self.items {
            if item.target.window_id == window_id
                && !live_targets.contains(&item.target)
                && !item.is_resolved()
            {
                item.resolved_at_ms = Some(now_ms);
                changed = true;
            }
        }
        changed
    }

    pub fn dismiss(&mut self, id: u64) -> bool {
        let before = self.items.len();
        self.items.retain(|item| item.id != id);
        self.items.len() != before
    }

    pub fn clear(&mut self) -> bool {
        if self.items.is_empty() {
            return false;
        }
        self.items.clear();
        true
    }
}

#[must_use]
pub fn interaction_status(interaction: AgentInteractionKind) -> &'static str {
    match interaction {
        AgentInteractionKind::Approval => "Needs approval",
        AgentInteractionKind::Decision => "Requires a decision",
        AgentInteractionKind::Question => "Has a question",
        AgentInteractionKind::Auth => "Needs sign-in",
        AgentInteractionKind::GenericInput | AgentInteractionKind::None => "Needs input",
    }
}

#[must_use]
pub fn interaction_fallback(interaction: AgentInteractionKind) -> &'static str {
    match interaction {
        AgentInteractionKind::Approval => "Approval required.",
        AgentInteractionKind::Decision => "Decision required.",
        AgentInteractionKind::Question => "Question pending.",
        AgentInteractionKind::Auth => "Sign-in required.",
        AgentInteractionKind::GenericInput | AgentInteractionKind::None => "Input required.",
    }
}
