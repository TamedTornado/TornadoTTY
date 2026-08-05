#![forbid(unsafe_code)]

mod agent_adapters;
mod agent_consent;
mod agent_launch;
mod agent_protocol;
mod agent_status;
mod command_palette;
mod pane_focus_history;
mod pane_layout;
mod pane_token_registry;
mod session_restore_store;
mod sidebar_width;
mod workspace_recipe;
mod workspace_state;

pub use agent_consent::{
    AgentIntegrationClass, AgentIntegrationGate, AgentIntegrationState, resolve_integration_gate,
};
pub use agent_launch::{
    AgentLaunchError, AgentLaunchPlan, AgentLaunchTool, build_agent_launch_plan,
};
pub use agent_protocol::{AgentEvent, AgentInteractionKind, AgentProtocolError};
pub use agent_status::{AgentPhase, AgentProgress, AgentStatusStore, PaneAgentStatus};
pub use pane_focus_history::PaneReference;
pub use pane_layout::{PaneLayoutPolicy, PaneRightInsertionBehavior};
pub use pane_token_registry::{
    AgentTarget, AuthenticatedAgentEvent, PaneTokenError, PaneTokenRegistry,
};
pub use session_restore_store::{
    LaunchDecision, LaunchReason, PersistenceRequest, SessionRestoreStore,
    SessionRestoreStoreError, SnapshotPersistence,
};
pub use sidebar_width::SidebarWidthPreference;
pub use workspace_recipe::{
    AgentLaunchSnapshot, ColumnRecipe, PaneRecipe, PaneRestoreDraft, RestoreDraftKind, SaveReason,
    SessionRestoreDraftWindow, SessionRestoreEnvelope, WindowFrame, WindowRecipe, WorklaneRecipe,
    WorkspaceRecipe,
};
pub use workspace_state::{
    ClosePaneOutcome, PaneColumnState, PaneState, SidebarPaneSummary, SidebarWorklaneSummary,
    WorklaneColor, WorklaneState, WorkspaceState, WorkspaceStateImportError,
};

/// Stable product identity shared by platform composition roots.
pub const PRODUCT_NAME: &str = "Zentty";
pub use agent_adapters::{AgentAdapterError, adapt_claude_hook, adapt_codex_hook};
pub use command_palette::{
    CommandPaletteGroup, CommandPaletteItem, CommandPaletteTarget, resolve_command_palette,
};
