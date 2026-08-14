use crate::{
    AgentInteractionKind, AgentPhase, AgentProgress, AttentionTarget, PaneAgentStatus,
    SidebarWorklaneSummary,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FleetState {
    Waiting,
    Stopped,
    Compacting,
    Active,
    Idle,
}

impl FleetState {
    #[must_use]
    pub fn priority(self) -> u8 {
        match self {
            Self::Waiting => 1,
            Self::Stopped => 2,
            Self::Compacting => 3,
            Self::Active => 4,
            Self::Idle => 5,
        }
    }

    #[must_use]
    pub fn status_label(self, interaction: AgentInteractionKind) -> &'static str {
        match self {
            Self::Waiting => match interaction {
                AgentInteractionKind::Approval => "Requires approval",
                AgentInteractionKind::Question | AgentInteractionKind::Decision => "Needs decision",
                AgentInteractionKind::Auth => "Needs sign-in",
                AgentInteractionKind::GenericInput | AgentInteractionKind::None => "Needs input",
            },
            Self::Stopped => "Stopped early",
            Self::Compacting => "Compacting",
            Self::Active => "Running",
            Self::Idle => "Idle",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FleetPaneSnapshot {
    pub target: AttentionTarget,
    pub window_title: String,
    pub worklane_title: String,
    pub agent_name: String,
    pub primary_text: String,
    pub context_text: String,
    pub status_label: String,
    pub state: FleetState,
    pub updated_at_ms: u64,
    pub progress: Option<AgentProgress>,
}

#[derive(Clone, Copy, Debug)]
pub struct FleetWindowSource<'a> {
    pub window_id: &'a str,
    pub window_title: &'a str,
    pub worklanes: &'a [SidebarWorklaneSummary],
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FleetSummary {
    pub waiting_count: usize,
    pub stopped_count: usize,
    pub compacting_count: usize,
    pub active_count: usize,
    pub idle_count: usize,
}

impl FleetSummary {
    #[must_use]
    pub fn from_snapshots(snapshots: &[FleetPaneSnapshot]) -> Self {
        let mut summary = Self::default();
        for snapshot in snapshots {
            match snapshot.state {
                FleetState::Waiting => summary.waiting_count += 1,
                FleetState::Stopped => summary.stopped_count += 1,
                FleetState::Compacting => summary.compacting_count += 1,
                FleetState::Active => summary.active_count += 1,
                FleetState::Idle => summary.idle_count += 1,
            }
        }
        summary
    }

    #[must_use]
    pub fn total_count(self) -> usize {
        self.waiting_count
            + self.stopped_count
            + self.compacting_count
            + self.active_count
            + self.idle_count
    }

    #[must_use]
    pub fn waiting_section_count(self) -> usize {
        self.waiting_count + self.stopped_count
    }

    #[must_use]
    pub fn running_section_count(self) -> usize {
        self.compacting_count + self.active_count
    }

    #[must_use]
    pub fn aggregate_state(self) -> FleetState {
        if self.waiting_count > 0 {
            FleetState::Waiting
        } else if self.stopped_count > 0 {
            FleetState::Stopped
        } else if self.compacting_count > 0 {
            FleetState::Compacting
        } else if self.active_count > 0 {
            FleetState::Active
        } else {
            FleetState::Idle
        }
    }

    #[must_use]
    pub fn header(self) -> String {
        if self.total_count() == 0 {
            return "No agent panes".to_owned();
        }
        let mut parts = Vec::new();
        if self.waiting_count > 0 {
            parts.push(format!("{} waiting", self.waiting_count));
        }
        if self.stopped_count > 0 {
            parts.push(format!("{} stopped", self.stopped_count));
        }
        if self.compacting_count > 0 {
            parts.push(format!("{} compacting", self.compacting_count));
        }
        if self.active_count > 0 {
            parts.push(format!("{} active", self.active_count));
        }
        if self.idle_count > 0 {
            parts.push(format!("{} idle", self.idle_count));
        }
        parts.join(" · ")
    }

    #[must_use]
    pub fn accessibility_label(self) -> String {
        let base = match (self.aggregate_state(), self.total_count()) {
            (_, 0) => "No agent panes",
            (FleetState::Waiting, _) => "Agent status: waiting for input",
            (FleetState::Stopped, _) => "Agent status: stopped early",
            (FleetState::Compacting, _) => "Agent status: compacting context",
            (FleetState::Active, _) => "Agent status: active",
            (FleetState::Idle, _) => "Agent status: idle",
        };
        if self.total_count() <= 1 {
            return base.to_owned();
        }
        let mut detail = Vec::new();
        if self.waiting_count > 0 {
            detail.push(format!("{} waiting", self.waiting_count));
        }
        if self.stopped_count > 0 {
            detail.push(format!("{} stopped", self.stopped_count));
        }
        let running = self.running_section_count();
        if running > 0 {
            detail.push(format!("{running} running"));
        }
        if self.idle_count > 0 {
            detail.push(format!("{} idle", self.idle_count));
        }
        format!("{base}. {}", detail.join(", "))
    }
}

#[must_use]
pub fn build_fleet_snapshots(sources: &[FleetWindowSource<'_>]) -> Vec<FleetPaneSnapshot> {
    let mut snapshots = sources
        .iter()
        .flat_map(|source| snapshots_for_window(*source))
        .collect::<Vec<_>>();
    snapshots.sort_by(|left, right| {
        left.state
            .priority()
            .cmp(&right.state.priority())
            .then_with(|| right.updated_at_ms.cmp(&left.updated_at_ms))
            .then_with(|| {
                left.primary_text
                    .to_ascii_lowercase()
                    .cmp(&right.primary_text.to_ascii_lowercase())
            })
            .then_with(|| left.target.pane_id.cmp(&right.target.pane_id))
    });
    snapshots
}

fn snapshots_for_window(source: FleetWindowSource<'_>) -> Vec<FleetPaneSnapshot> {
    source
        .worklanes
        .iter()
        .flat_map(|worklane| {
            worklane.pane_rows.iter().filter_map(|pane| {
                let status = pane.agent_status.as_ref()?;
                let state = fleet_state(status);
                let worklane_title = worklane
                    .top_label
                    .clone()
                    .unwrap_or_else(|| worklane.worklane_id.clone());
                let primary_text = meaningful_primary_text(&pane.primary_text, status);
                let context_text = project_context(pane).map_or_else(
                    || format!("{} · {worklane_title}", source.window_title),
                    |project| format!("{project} · {worklane_title}"),
                );
                Some(FleetPaneSnapshot {
                    target: AttentionTarget::new(
                        source.window_id,
                        &worklane.worklane_id,
                        &pane.pane_id,
                    ),
                    window_title: source.window_title.to_owned(),
                    worklane_title,
                    agent_name: status.agent_name.clone(),
                    primary_text,
                    context_text,
                    status_label: state.status_label(status.interaction).to_owned(),
                    state,
                    updated_at_ms: status.updated_at,
                    progress: status
                        .progress
                        .filter(|progress| progress.done < progress.total),
                })
            })
        })
        .collect()
}

fn meaningful_primary_text(value: &str, status: &PaneAgentStatus) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("shell") {
        status.agent_name.clone()
    } else {
        trimmed.to_owned()
    }
}

fn project_context(pane: &crate::SidebarPaneSummary) -> Option<String> {
    let context = pane.project_context.as_ref()?;
    let folder = context.repository_root.file_name()?.to_str()?;
    Some(format!("{} · {}", folder, context.reference.display()))
}

fn fleet_state(status: &PaneAgentStatus) -> FleetState {
    match status.phase {
        AgentPhase::NeedsInput => FleetState::Waiting,
        AgentPhase::UnresolvedStop => FleetState::Stopped,
        AgentPhase::Starting | AgentPhase::Running if is_compacting(status.text.as_deref()) => {
            FleetState::Compacting
        }
        AgentPhase::Starting | AgentPhase::Running => FleetState::Active,
        AgentPhase::Idle => FleetState::Idle,
    }
}

fn is_compacting(text: Option<&str>) -> bool {
    text.is_some_and(|text| {
        let lowered = text.to_ascii_lowercase();
        lowered.contains("compact") || lowered.contains("summariz")
    })
}
