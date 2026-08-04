#![forbid(unsafe_code)]

mod command_palette;
mod pane_focus_history;
mod pane_layout;
mod session_restore_store;
mod sidebar_width;
mod workspace_recipe;
mod workspace_state;

pub use pane_focus_history::PaneReference;
pub use pane_layout::{PaneLayoutPolicy, PaneRightInsertionBehavior};
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
pub use command_palette::{
    CommandPaletteGroup, CommandPaletteItem, CommandPaletteTarget, resolve_command_palette,
};
