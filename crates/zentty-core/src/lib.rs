#![forbid(unsafe_code)]

mod workspace;
mod workspace_persistence;

pub use workspace::{
    AgentResume, ColumnLayout, Pane, PaneLayout, StableId, Window, Worklane, Workspace,
    WorkspaceError,
};

/// Stable product identity shared by platform composition roots.
pub const PRODUCT_NAME: &str = "Zentty";
