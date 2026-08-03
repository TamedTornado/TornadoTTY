use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct StableId(String);

impl StableId {
    /// Parses the UUID-shaped stable identifier used by the durable schema.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceError::InvalidStableId`] unless `value` is a
    /// lowercase RFC 4122 version-4 UUID.
    pub fn parse(value: impl Into<String>) -> Result<Self, WorkspaceError> {
        let value = value.into();
        if is_stable_id(&value) {
            Ok(Self(value))
        } else {
            Err(WorkspaceError::InvalidStableId(value))
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StableId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pane {
    id: StableId,
    title: Option<String>,
    cwd: PathBuf,
    launch_profile_id: String,
}

impl Pane {
    /// Constructs a persistable pane launch record.
    ///
    /// This structural check deliberately does not access the filesystem;
    /// platform launch validation owns missing directories and profiles.
    ///
    /// # Errors
    ///
    /// Returns an error when the CWD is not absolute or the launch-profile ID
    /// is outside the durable schema's approved syntax.
    pub fn new(
        id: StableId,
        cwd: impl Into<PathBuf>,
        launch_profile_id: impl Into<String>,
    ) -> Result<Self, WorkspaceError> {
        let cwd = cwd.into();
        if !is_valid_cwd(&cwd) {
            return Err(WorkspaceError::InvalidCwd(cwd));
        }
        let launch_profile_id = launch_profile_id.into();
        if !is_launch_profile_id(&launch_profile_id) {
            return Err(WorkspaceError::InvalidLaunchProfileId(launch_profile_id));
        }
        Ok(Self {
            id,
            title: None,
            cwd,
            launch_profile_id,
        })
    }

    #[must_use]
    pub const fn id(&self) -> &StableId {
        &self.id
    }

    #[must_use]
    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    #[must_use]
    pub fn launch_profile_id(&self) -> &str {
        &self.launch_profile_id
    }

    #[must_use]
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Worklane {
    id: StableId,
    title: Option<String>,
    panes: Vec<Pane>,
    active_pane_id: StableId,
}

impl Worklane {
    #[must_use]
    pub const fn id(&self) -> &StableId {
        &self.id
    }

    #[must_use]
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    #[must_use]
    pub fn panes(&self) -> &[Pane] {
        &self.panes
    }

    #[must_use]
    pub const fn active_pane_id(&self) -> &StableId {
        &self.active_pane_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Window {
    id: StableId,
    worklanes: Vec<Worklane>,
    active_worklane_id: StableId,
}

impl Window {
    #[must_use]
    pub const fn id(&self) -> &StableId {
        &self.id
    }

    #[must_use]
    pub fn worklanes(&self) -> &[Worklane] {
        &self.worklanes
    }

    #[must_use]
    pub const fn active_worklane_id(&self) -> &StableId {
        &self.active_worklane_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Workspace {
    id: StableId,
    revision: u64,
    windows: Vec<Window>,
    active_window_id: StableId,
}

impl Workspace {
    /// Creates the required first window, worklane, and pane atomically.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceError::DuplicateId`] if any durable identity is
    /// reused within the initial topology.
    pub fn new(
        id: StableId,
        window_id: StableId,
        worklane_id: StableId,
        initial_pane: Pane,
    ) -> Result<Self, WorkspaceError> {
        let mut initial_ids = [&id, &window_id, &worklane_id, initial_pane.id()];
        initial_ids.sort_by(|left, right| left.as_str().cmp(right.as_str()));
        if let Some(duplicate) = initial_ids
            .windows(2)
            .find(|pair| pair[0] == pair[1])
            .map(|pair| pair[0].clone())
        {
            return Err(WorkspaceError::DuplicateId(duplicate));
        }
        let active_pane_id = initial_pane.id.clone();
        Ok(Self {
            id,
            revision: 0,
            active_window_id: window_id.clone(),
            windows: vec![Window {
                id: window_id,
                active_worklane_id: worklane_id.clone(),
                worklanes: vec![Worklane {
                    id: worklane_id,
                    title: None,
                    panes: vec![initial_pane],
                    active_pane_id,
                }],
            }],
        })
    }

    #[must_use]
    pub const fn id(&self) -> &StableId {
        &self.id
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub fn windows(&self) -> &[Window] {
        &self.windows
    }

    #[must_use]
    pub const fn active_window_id(&self) -> &StableId {
        &self.active_window_id
    }

    /// Appends a worklane containing its required initial pane.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown window, invalid title, or duplicate ID.
    pub fn add_worklane(
        &mut self,
        window_id: &StableId,
        worklane_id: StableId,
        title: Option<String>,
        initial_pane: Pane,
    ) -> Result<(), WorkspaceError> {
        validate_title(title.as_deref())?;
        self.reject_duplicate(&worklane_id)?;
        self.reject_duplicate(initial_pane.id())?;
        if initial_pane.id() == &worklane_id {
            return Err(WorkspaceError::DuplicateId(worklane_id));
        }
        let window = self.window_mut(window_id)?;
        let active_pane_id = initial_pane.id.clone();
        window.worklanes.push(Worklane {
            id: worklane_id,
            title,
            panes: vec![initial_pane],
            active_pane_id,
        });
        self.changed();
        Ok(())
    }

    /// Renames a worklane without changing its identity or position.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid title or unknown window/worklane.
    pub fn rename_worklane(
        &mut self,
        window_id: &StableId,
        worklane_id: &StableId,
        title: Option<String>,
    ) -> Result<(), WorkspaceError> {
        validate_title(title.as_deref())?;
        self.worklane_mut(window_id, worklane_id)?.title = title;
        self.changed();
        Ok(())
    }

    /// Moves a worklane to an existing zero-based position.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown entity or out-of-range destination.
    pub fn move_worklane(
        &mut self,
        window_id: &StableId,
        worklane_id: &StableId,
        destination: usize,
    ) -> Result<(), WorkspaceError> {
        let window = self.window_mut(window_id)?;
        let source = position(&window.worklanes, worklane_id, |lane| lane.id())
            .ok_or_else(|| WorkspaceError::WorklaneNotFound(worklane_id.clone()))?;
        move_item(&mut window.worklanes, source, destination)?;
        self.changed();
        Ok(())
    }

    /// Selects an existing worklane.
    ///
    /// # Errors
    ///
    /// Returns an error when the window or worklane is unknown.
    pub fn select_worklane(
        &mut self,
        window_id: &StableId,
        worklane_id: &StableId,
    ) -> Result<(), WorkspaceError> {
        let window = self.window_mut(window_id)?;
        if !window.worklanes.iter().any(|lane| lane.id() == worklane_id) {
            return Err(WorkspaceError::WorklaneNotFound(worklane_id.clone()));
        }
        window.active_worklane_id.clone_from(worklane_id);
        self.changed();
        Ok(())
    }

    /// Removes a worklane and deterministically repairs active selection.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown entity or an attempt to remove the
    /// final worklane.
    pub fn remove_worklane(
        &mut self,
        window_id: &StableId,
        worklane_id: &StableId,
    ) -> Result<Worklane, WorkspaceError> {
        let window = self.window_mut(window_id)?;
        if window.worklanes.len() == 1 {
            return Err(WorkspaceError::CannotRemoveFinalWorklane);
        }
        let index = position(&window.worklanes, worklane_id, |lane| lane.id())
            .ok_or_else(|| WorkspaceError::WorklaneNotFound(worklane_id.clone()))?;
        let removed = window.worklanes.remove(index);
        if &window.active_worklane_id == worklane_id {
            let repaired = index.min(window.worklanes.len() - 1);
            window.active_worklane_id = window.worklanes[repaired].id.clone();
        }
        self.changed();
        Ok(removed)
    }

    /// Appends a pane to a worklane.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown owner or globally duplicated ID.
    pub fn add_pane(
        &mut self,
        window_id: &StableId,
        worklane_id: &StableId,
        pane: Pane,
    ) -> Result<(), WorkspaceError> {
        self.reject_duplicate(pane.id())?;
        self.worklane_mut(window_id, worklane_id)?.panes.push(pane);
        self.changed();
        Ok(())
    }

    /// Moves a pane to an existing zero-based position.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown entity or out-of-range destination.
    pub fn move_pane(
        &mut self,
        window_id: &StableId,
        worklane_id: &StableId,
        pane_id: &StableId,
        destination: usize,
    ) -> Result<(), WorkspaceError> {
        let lane = self.worklane_mut(window_id, worklane_id)?;
        let source = position(&lane.panes, pane_id, |pane| pane.id())
            .ok_or_else(|| WorkspaceError::PaneNotFound(pane_id.clone()))?;
        move_item(&mut lane.panes, source, destination)?;
        self.changed();
        Ok(())
    }

    /// Selects an existing pane.
    ///
    /// # Errors
    ///
    /// Returns an error when the window, worklane, or pane is unknown.
    pub fn select_pane(
        &mut self,
        window_id: &StableId,
        worklane_id: &StableId,
        pane_id: &StableId,
    ) -> Result<(), WorkspaceError> {
        let lane = self.worklane_mut(window_id, worklane_id)?;
        if !lane.panes.iter().any(|pane| pane.id() == pane_id) {
            return Err(WorkspaceError::PaneNotFound(pane_id.clone()));
        }
        lane.active_pane_id.clone_from(pane_id);
        self.changed();
        Ok(())
    }

    /// Removes a pane and deterministically repairs active selection.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown entity or an attempt to remove the
    /// final pane.
    pub fn remove_pane(
        &mut self,
        window_id: &StableId,
        worklane_id: &StableId,
        pane_id: &StableId,
    ) -> Result<Pane, WorkspaceError> {
        let lane = self.worklane_mut(window_id, worklane_id)?;
        if lane.panes.len() == 1 {
            return Err(WorkspaceError::CannotRemoveFinalPane);
        }
        let index = position(&lane.panes, pane_id, |pane| pane.id())
            .ok_or_else(|| WorkspaceError::PaneNotFound(pane_id.clone()))?;
        let removed = lane.panes.remove(index);
        if &lane.active_pane_id == pane_id {
            let repaired = index.min(lane.panes.len() - 1);
            lane.active_pane_id = lane.panes[repaired].id.clone();
        }
        self.changed();
        Ok(removed)
    }

    fn window_mut(&mut self, window_id: &StableId) -> Result<&mut Window, WorkspaceError> {
        self.windows
            .iter_mut()
            .find(|window| window.id() == window_id)
            .ok_or_else(|| WorkspaceError::WindowNotFound(window_id.clone()))
    }

    fn worklane_mut(
        &mut self,
        window_id: &StableId,
        worklane_id: &StableId,
    ) -> Result<&mut Worklane, WorkspaceError> {
        self.window_mut(window_id)?
            .worklanes
            .iter_mut()
            .find(|lane| lane.id() == worklane_id)
            .ok_or_else(|| WorkspaceError::WorklaneNotFound(worklane_id.clone()))
    }

    fn reject_duplicate(&self, id: &StableId) -> Result<(), WorkspaceError> {
        let duplicate = self.windows.iter().any(|window| {
            window.id() == id
                || window
                    .worklanes
                    .iter()
                    .any(|lane| lane.id() == id || lane.panes.iter().any(|pane| pane.id() == id))
        });
        if duplicate {
            Err(WorkspaceError::DuplicateId(id.clone()))
        } else {
            Ok(())
        }
    }

    fn changed(&mut self) {
        self.revision = self.revision.saturating_add(1);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkspaceError {
    InvalidStableId(String),
    InvalidTitle,
    InvalidCwd(PathBuf),
    InvalidLaunchProfileId(String),
    DuplicateId(StableId),
    WindowNotFound(StableId),
    WorklaneNotFound(StableId),
    PaneNotFound(StableId),
    InvalidDestination { destination: usize, len: usize },
    CannotRemoveFinalWorklane,
    CannotRemoveFinalPane,
}

impl fmt::Display for WorkspaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidStableId(value) => write!(formatter, "invalid stable ID: {value}"),
            Self::InvalidTitle => formatter.write_str("title must be non-empty and contain no NUL"),
            Self::InvalidCwd(path) => write!(
                formatter,
                "pane CWD must be an absolute path without NUL: {}",
                path.display()
            ),
            Self::InvalidLaunchProfileId(value) => {
                write!(formatter, "invalid launch profile ID: {value}")
            }
            Self::DuplicateId(id) => write!(formatter, "duplicate entity ID: {id}"),
            Self::WindowNotFound(id) => write!(formatter, "window not found: {id}"),
            Self::WorklaneNotFound(id) => write!(formatter, "worklane not found: {id}"),
            Self::PaneNotFound(id) => write!(formatter, "pane not found: {id}"),
            Self::InvalidDestination { destination, len } => write!(
                formatter,
                "destination index {destination} is outside collection of length {len}"
            ),
            Self::CannotRemoveFinalWorklane => {
                formatter.write_str("cannot remove the final worklane from a window")
            }
            Self::CannotRemoveFinalPane => {
                formatter.write_str("cannot remove the final pane from a worklane")
            }
        }
    }
}

impl Error for WorkspaceError {}

fn is_stable_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => *byte == b'-',
            14 => *byte == b'4',
            19 => matches!(*byte, b'8' | b'9' | b'a' | b'b'),
            _ => byte.is_ascii_digit() || matches!(*byte, b'a'..=b'f'),
        })
}

fn is_valid_cwd(path: &Path) -> bool {
    path.is_absolute() && !path.as_os_str().to_string_lossy().contains('\0')
}

fn is_launch_profile_id(value: &str) -> bool {
    let mut bytes = value.bytes();
    matches!(bytes.next(), Some(b'a'..=b'z'))
        && value.len() <= 64
        && bytes.all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(&byte)
        })
}

fn validate_title(title: Option<&str>) -> Result<(), WorkspaceError> {
    if title.is_some_and(|value| value.is_empty() || value.contains('\0')) {
        Err(WorkspaceError::InvalidTitle)
    } else {
        Ok(())
    }
}

fn position<T>(items: &[T], id: &StableId, get_id: impl Fn(&T) -> &StableId) -> Option<usize> {
    items.iter().position(|item| get_id(item) == id)
}

fn move_item<T>(
    items: &mut Vec<T>,
    source: usize,
    destination: usize,
) -> Result<(), WorkspaceError> {
    if destination >= items.len() {
        return Err(WorkspaceError::InvalidDestination {
            destination,
            len: items.len(),
        });
    }
    let item = items.remove(source);
    items.insert(destination, item);
    Ok(())
}
