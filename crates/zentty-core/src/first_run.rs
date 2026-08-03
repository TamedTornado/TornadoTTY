use std::path::{Path, PathBuf};

use crate::{Pane, StableId, Workspace, WorkspaceError};

pub trait StableIdSource {
    /// Produces one stable version-4 identity.
    ///
    /// # Errors
    ///
    /// Returns a model error when the platform identity source is unavailable
    /// or produces an invalid identifier.
    fn next_id(&mut self) -> Result<StableId, WorkspaceError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FirstRunSpec {
    cwd: PathBuf,
    launch_profile_id: String,
}

impl FirstRunSpec {
    #[must_use]
    pub fn new(cwd: impl Into<PathBuf>, launch_profile_id: impl Into<String>) -> Self {
        Self {
            cwd: cwd.into(),
            launch_profile_id: launch_profile_id.into(),
        }
    }

    #[must_use]
    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    #[must_use]
    pub fn launch_profile_id(&self) -> &str {
        &self.launch_profile_id
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum WorkspaceLoad {
    Existing(Workspace),
    Created(Workspace),
}

impl WorkspaceLoad {
    #[must_use]
    pub const fn workspace(&self) -> &Workspace {
        match self {
            Self::Existing(workspace) | Self::Created(workspace) => workspace,
        }
    }

    #[must_use]
    pub const fn was_created(&self) -> bool {
        matches!(self, Self::Created(_))
    }

    #[must_use]
    pub fn into_workspace(self) -> Workspace {
        match self {
            Self::Existing(workspace) | Self::Created(workspace) => workspace,
        }
    }
}

impl Workspace {
    /// Constructs the documented first-run topology: one window, one
    /// worklane, and one active pane.
    ///
    /// # Errors
    ///
    /// Returns an error if identity generation fails, identities collide, or
    /// the initial pane launch reference is structurally invalid.
    pub fn first_run(
        source: &mut impl StableIdSource,
        spec: &FirstRunSpec,
    ) -> Result<Self, WorkspaceError> {
        let workspace_id = source.next_id()?;
        let window_id = source.next_id()?;
        let worklane_id = source.next_id()?;
        let pane = Pane::new(
            source.next_id()?,
            spec.cwd.clone(),
            spec.launch_profile_id.clone(),
        )?;
        Self::new(workspace_id, window_id, worklane_id, pane)
    }
}
