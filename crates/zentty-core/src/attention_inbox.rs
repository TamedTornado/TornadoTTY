use crate::{AgentInteractionKind, AgentPhase, PaneAgentStatus};
use std::collections::{HashMap, HashSet};

const MAX_ITEMS: usize = 50;
const NEEDS_INPUT_DEBOUNCE_MS: u64 = 3_000;

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
    pub state: AttentionState,
    pub interaction: AgentInteractionKind,
    pub status_text: String,
    pub primary_text: String,
    pub location_text: Option<String>,
    pub created_at_ms: u64,
    pub resolved_at_ms: Option<u64>,
    origin: AttentionOrigin,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AttentionOrigin {
    Agent,
    PaneNotification,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttentionState {
    NeedsInput,
    Ready,
    UnresolvedStop,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttentionDelivery {
    pub item: AttentionItem,
    pub desktop_allowed: bool,
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
    state: AttentionState,
    interaction: AgentInteractionKind,
    text: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PendingAttention {
    target: AttentionTarget,
    agent_name: String,
    state: AttentionState,
    interaction: AgentInteractionKind,
    status_text: String,
    primary_text: String,
    location_text: Option<String>,
    created_at_ms: u64,
    commit_at_ms: u64,
    desktop_allowed: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AttentionInbox {
    items: Vec<AttentionItem>,
    signatures: HashMap<AttentionTarget, AttentionSignature>,
    active_views: HashMap<AttentionTarget, bool>,
    pending: HashMap<AttentionTarget, PendingAttention>,
    deliveries: Vec<AttentionDelivery>,
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

    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    pub fn observe(
        &mut self,
        target: AttentionTarget,
        status: Option<&PaneAgentStatus>,
        now_ms: u64,
    ) -> bool {
        self.observe_with_context(target, status, false, None, now_ms)
    }

    pub fn observe_with_context(
        &mut self,
        target: AttentionTarget,
        status: Option<&PaneAgentStatus>,
        is_actively_viewed: bool,
        location_text: Option<String>,
        now_ms: u64,
    ) -> bool {
        let was_actively_viewed = self
            .active_views
            .insert(target.clone(), is_actively_viewed)
            .unwrap_or(false);
        let Some(status) = status else {
            self.signatures.remove(&target);
            return self.cancel_and_resolve(&target, now_ms);
        };
        let Some((state, status_text, primary_text)) = attention_candidate(status) else {
            self.signatures.remove(&target);
            return self.cancel_and_resolve(&target, now_ms);
        };
        let signature = AttentionSignature {
            agent_name: status.agent_name.clone(),
            state,
            interaction: status.interaction,
            text: if state == AttentionState::Ready {
                None
            } else {
                status.text.clone()
            },
        };
        if self.signatures.get(&target) == Some(&signature) {
            let mut changed = if state == AttentionState::Ready {
                self.enrich_ready_item(&target, &primary_text, location_text.as_deref())
            } else {
                false
            };
            if is_actively_viewed && !was_actively_viewed {
                changed |= self.cancel_and_resolve(&target, now_ms);
            }
            return changed;
        }

        let mut changed = self.cancel_and_resolve(&target, now_ms);
        self.signatures.insert(target.clone(), signature);
        let pending = PendingAttention {
            target: target.clone(),
            agent_name: status.agent_name.clone(),
            state,
            interaction: status.interaction,
            status_text: status_text.to_owned(),
            primary_text,
            location_text,
            created_at_ms: now_ms,
            commit_at_ms: now_ms.saturating_add(if state == AttentionState::NeedsInput {
                NEEDS_INPUT_DEBOUNCE_MS
            } else {
                0
            }),
            desktop_allowed: !is_actively_viewed,
        };
        if pending.commit_at_ms > now_ms {
            self.pending.insert(target, pending);
            return changed;
        }
        changed |= self.commit(pending);
        changed
    }

    pub fn advance(&mut self, now_ms: u64) -> bool {
        let mut ready = self
            .pending
            .values()
            .filter(|pending| pending.commit_at_ms <= now_ms)
            .cloned()
            .collect::<Vec<_>>();
        ready.sort_by(|lhs, rhs| {
            lhs.created_at_ms
                .cmp(&rhs.created_at_ms)
                .then_with(|| lhs.target.window_id.cmp(&rhs.target.window_id))
                .then_with(|| lhs.target.worklane_id.cmp(&rhs.target.worklane_id))
                .then_with(|| lhs.target.pane_id.cmp(&rhs.target.pane_id))
        });
        let mut changed = false;
        for pending in ready {
            self.pending.remove(&pending.target);
            changed |= self.commit(pending);
        }
        changed
    }

    pub fn drain_deliveries(&mut self) -> Vec<AttentionDelivery> {
        std::mem::take(&mut self.deliveries)
    }

    /// Records an explicit pane-local notification in the shared inbox. The
    /// desktop delivery is intentionally owned by the caller so title/body and
    /// silent semantics are not coerced through the agent-attention formatter.
    pub fn record_pane_notification(
        &mut self,
        target: AttentionTarget,
        title: impl Into<String>,
        primary_text: impl Into<String>,
        now_ms: u64,
    ) -> bool {
        self.next_id = self.next_id.saturating_add(1);
        self.items.insert(
            0,
            AttentionItem {
                id: self.next_id,
                target,
                agent_name: "Zentty".to_owned(),
                state: AttentionState::Ready,
                interaction: AgentInteractionKind::None,
                status_text: title.into(),
                primary_text: primary_text.into(),
                location_text: None,
                created_at_ms: now_ms,
                resolved_at_ms: None,
                origin: AttentionOrigin::PaneNotification,
            },
        );
        self.items.truncate(MAX_ITEMS);
        true
    }

    fn enrich_ready_item(
        &mut self,
        target: &AttentionTarget,
        primary_text: &str,
        location_text: Option<&str>,
    ) -> bool {
        let Some(item) = self.items.iter_mut().find(|item| {
            item.target == *target
                && item.origin == AttentionOrigin::Agent
                && item.state == AttentionState::Ready
        }) else {
            return false;
        };
        let mut changed = false;
        if item.primary_text != primary_text {
            item.primary_text = primary_text.to_owned();
            changed = true;
        }
        if item.location_text.as_deref() != location_text {
            item.location_text = location_text.map(str::to_owned);
            changed = true;
        }
        changed
    }

    fn commit(&mut self, pending: PendingAttention) -> bool {
        self.next_id = self.next_id.saturating_add(1);
        let item = AttentionItem {
            id: self.next_id,
            target: pending.target,
            agent_name: pending.agent_name,
            state: pending.state,
            interaction: pending.interaction,
            status_text: pending.status_text,
            primary_text: pending.primary_text,
            location_text: pending.location_text,
            created_at_ms: pending.created_at_ms,
            resolved_at_ms: None,
            origin: AttentionOrigin::Agent,
        };
        self.items.insert(0, item.clone());
        self.deliveries.push(AttentionDelivery {
            item,
            desktop_allowed: pending.desktop_allowed,
        });
        self.items.truncate(MAX_ITEMS);
        true
    }

    pub fn resolve_target(&mut self, target: &AttentionTarget, now_ms: u64) -> bool {
        let before = self.deliveries.len();
        self.deliveries
            .retain(|delivery| delivery.item.target != *target);
        let mut changed = self.deliveries.len() != before;
        for item in &mut self.items {
            if item.target == *target && !item.is_resolved() {
                item.resolved_at_ms = Some(now_ms);
                changed = true;
            }
        }
        changed
    }

    fn resolve_agent_target(&mut self, target: &AttentionTarget, now_ms: u64) -> bool {
        let before = self.deliveries.len();
        self.deliveries
            .retain(|delivery| delivery.item.target != *target);
        let mut changed = self.deliveries.len() != before;
        for item in &mut self.items {
            if item.target == *target
                && item.origin == AttentionOrigin::Agent
                && !item.is_resolved()
            {
                item.resolved_at_ms = Some(now_ms);
                changed = true;
            }
        }
        changed
    }

    fn cancel_pending(&mut self, target: &AttentionTarget) -> bool {
        self.pending.remove(target).is_some()
    }

    fn cancel_and_resolve(&mut self, target: &AttentionTarget, now_ms: u64) -> bool {
        let mut changed = self.cancel_pending(target);
        if self.resolve_agent_target(target, now_ms) {
            changed = true;
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
        self.active_views
            .retain(|target, _| target.window_id != window_id || live_targets.contains(target));
        let mut changed = false;
        self.pending.retain(|target, _| {
            let keep = target.window_id != window_id || live_targets.contains(target);
            if !keep {
                changed = true;
            }
            keep
        });
        self.deliveries.retain(|delivery| {
            let keep = delivery.item.target.window_id != window_id
                || live_targets.contains(&delivery.item.target);
            if !keep {
                changed = true;
            }
            keep
        });
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
        let mut changed = self.items.len() != before;
        let before = self.deliveries.len();
        self.deliveries.retain(|delivery| delivery.item.id != id);
        if self.deliveries.len() != before {
            changed = true;
        }
        changed
    }

    pub fn clear(&mut self) -> bool {
        if self.items.is_empty() && self.pending.is_empty() && self.deliveries.is_empty() {
            return false;
        }
        self.items.clear();
        self.pending.clear();
        self.deliveries.clear();
        true
    }
}

fn attention_candidate(status: &PaneAgentStatus) -> Option<(AttentionState, &'static str, String)> {
    match status.phase {
        AgentPhase::NeedsInput if status.interaction != AgentInteractionKind::None => Some((
            AttentionState::NeedsInput,
            interaction_status(status.interaction),
            status
                .text
                .clone()
                .unwrap_or_else(|| interaction_fallback(status.interaction).to_owned()),
        )),
        AgentPhase::Idle => Some((
            AttentionState::Ready,
            "Agent ready",
            status
                .text
                .clone()
                .unwrap_or_else(|| "Agent is ready.".to_owned()),
        )),
        AgentPhase::UnresolvedStop => Some((
            AttentionState::UnresolvedStop,
            "Stopped early",
            status
                .text
                .clone()
                .unwrap_or_else(|| "Agent stopped early.".to_owned()),
        )),
        AgentPhase::Starting | AgentPhase::Running | AgentPhase::NeedsInput => None,
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
