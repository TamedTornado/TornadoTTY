#![forbid(unsafe_code)]

mod workspace;

pub use workspace::{Pane, StableId, Window, Worklane, Workspace, WorkspaceError};

/// Stable product identity shared by platform composition roots.
pub const PRODUCT_NAME: &str = "Zentty";
