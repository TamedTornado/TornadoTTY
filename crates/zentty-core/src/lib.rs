#![forbid(unsafe_code)]

mod first_run;
mod workspace;
mod workspace_persistence;
mod workspace_store;

pub use first_run::{FirstRunSpec, StableIdSource, WorkspaceLoad};
pub use workspace::{
    AgentResume, ColumnLayout, Pane, PaneLayout, StableId, Window, Worklane, Workspace,
    WorkspaceError,
};
pub use workspace_store::{WorkspaceStore, WorkspaceStoreError};

/// Stable product identity shared by platform composition roots.
pub const PRODUCT_NAME: &str = "Zentty";
