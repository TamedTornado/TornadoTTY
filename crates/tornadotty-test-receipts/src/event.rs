use std::collections::BTreeSet;

use serde::{Deserialize, Deserializer, Serialize};

use crate::{ReceiptErrorKind, Result, error};

const MAX_ID_BYTES: usize = 96;
const MAX_WORKLANES: usize = 128;
const MAX_COLUMNS: usize = 128;
const MAX_PANES: usize = 512;
const MAX_WINDOW_EDGE: u32 = 65_535;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ReceiptId(String);

impl ReceiptId {
    /// Builds an identifier that is safe to include in a product receipt.
    ///
    /// # Errors
    ///
    /// Returns an error when the value is empty, oversized, or contains bytes
    /// outside the deliberately small identifier alphabet.
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        if value.is_empty()
            || value.len() > MAX_ID_BYTES
            || !value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':')
            })
        {
            return Err(error(
                ReceiptErrorKind::InvalidEvent,
                "receipt IDs must be 1..=96 safe ASCII bytes",
            ));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for ReceiptId {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleState {
    ProcessStarted,
    TerminalReady,
    ChildExited,
    ProcessStopped,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "target", rename_all = "snake_case", deny_unknown_fields)]
pub enum FocusTarget {
    Pane { pane_id: ReceiptId },
    Widget { widget: WidgetName },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WidgetName {
    MainWindow,
    SettingsWindow,
    NotificationsSection,
    NotificationSoundImport,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorklaneTopology {
    pub worklane_id: ReceiptId,
    pub pane_ids: Vec<ReceiptId>,
    pub selected_pane_id: ReceiptId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PaneColumn {
    pub column_id: ReceiptId,
    pub pane_ids: Vec<ReceiptId>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "scope", rename_all = "snake_case", deny_unknown_fields)]
pub enum GeometrySnapshot {
    Window {
        window_id: ReceiptId,
        width: u32,
        height: u32,
    },
    PaneLayout {
        window_id: ReceiptId,
        worklane_id: ReceiptId,
        columns: Vec<PaneColumn>,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionName {
    OpenSettings,
    CloseSettingsWindow,
    ResolveCommandPalette,
    SelectNotificationsSettings,
    SendTestNotification,
    ImportNotificationSound,
    PreviewNotificationSound,
    RemoveNotificationSound,
    SplitPaneRight,
    SplitPaneBelow,
    RestoreWorkspace,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionOutcome {
    Completed,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureCode {
    ApplicationTick,
    ReceiptWrite,
    RestoreWorkspace,
    SettingsAction,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "category", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReceiptEvent {
    Lifecycle {
        state: LifecycleState,
        #[serde(skip_serializing_if = "Option::is_none")]
        pane_id: Option<ReceiptId>,
    },
    Topology {
        window_id: ReceiptId,
        worklanes: Vec<WorklaneTopology>,
        focused_pane_id: ReceiptId,
    },
    Focus {
        focus: FocusTarget,
    },
    Geometry {
        geometry: GeometrySnapshot,
    },
    ActionCompletion {
        action: ActionName,
        outcome: ActionOutcome,
        #[serde(skip_serializing_if = "Option::is_none")]
        target_id: Option<ReceiptId>,
    },
    Failure {
        code: FailureCode,
        #[serde(skip_serializing_if = "Option::is_none")]
        target_id: Option<ReceiptId>,
    },
}

impl ReceiptEvent {
    pub(crate) fn validate(&self) -> Result<()> {
        match self {
            Self::Lifecycle { state, pane_id } => match state {
                LifecycleState::ProcessStarted | LifecycleState::ProcessStopped
                    if pane_id.is_some() =>
                {
                    Err(error(
                        ReceiptErrorKind::InvalidEvent,
                        "process lifecycle events may not name a pane",
                    ))
                }
                LifecycleState::TerminalReady | LifecycleState::ChildExited
                    if pane_id.is_none() =>
                {
                    Err(error(
                        ReceiptErrorKind::InvalidEvent,
                        "pane lifecycle events require a pane ID",
                    ))
                }
                _ => Ok(()),
            },
            Self::Topology {
                worklanes,
                focused_pane_id,
                ..
            } => validate_topology(worklanes, focused_pane_id),
            Self::Geometry { geometry } => validate_geometry(geometry),
            Self::Focus { .. } | Self::ActionCompletion { .. } | Self::Failure { .. } => Ok(()),
        }
    }
}

fn validate_topology(worklanes: &[WorklaneTopology], focused_pane_id: &ReceiptId) -> Result<()> {
    if worklanes.is_empty() || worklanes.len() > MAX_WORKLANES {
        return Err(error(
            ReceiptErrorKind::InvalidEvent,
            "topology worklane count is outside its bound",
        ));
    }
    let mut worklane_ids = BTreeSet::new();
    let mut pane_ids = BTreeSet::new();
    for worklane in worklanes {
        if !worklane_ids.insert(worklane.worklane_id.clone())
            || worklane.pane_ids.is_empty()
            || worklane.pane_ids.len() > MAX_PANES
            || !worklane.pane_ids.contains(&worklane.selected_pane_id)
        {
            return Err(error(
                ReceiptErrorKind::InvalidEvent,
                "topology contains a duplicate/empty worklane or invalid selection",
            ));
        }
        for pane_id in &worklane.pane_ids {
            if !pane_ids.insert(pane_id.clone()) || pane_ids.len() > MAX_PANES {
                return Err(error(
                    ReceiptErrorKind::InvalidEvent,
                    "topology pane IDs must be globally unique and bounded",
                ));
            }
        }
    }
    if !pane_ids.contains(focused_pane_id) {
        return Err(error(
            ReceiptErrorKind::InvalidEvent,
            "focused pane is absent from the topology",
        ));
    }
    Ok(())
}

fn validate_geometry(geometry: &GeometrySnapshot) -> Result<()> {
    match geometry {
        GeometrySnapshot::Window { width, height, .. } => {
            if *width == 0 || *height == 0 || *width > MAX_WINDOW_EDGE || *height > MAX_WINDOW_EDGE
            {
                return Err(error(
                    ReceiptErrorKind::InvalidEvent,
                    "window geometry is outside its bound",
                ));
            }
        }
        GeometrySnapshot::PaneLayout { columns, .. } => {
            if columns.is_empty() || columns.len() > MAX_COLUMNS {
                return Err(error(
                    ReceiptErrorKind::InvalidEvent,
                    "pane geometry column count is outside its bound",
                ));
            }
            let mut column_ids = BTreeSet::new();
            let mut pane_ids = BTreeSet::new();
            for column in columns {
                if !column_ids.insert(column.column_id.clone())
                    || column.pane_ids.is_empty()
                    || column.pane_ids.len() > MAX_PANES
                {
                    return Err(error(
                        ReceiptErrorKind::InvalidEvent,
                        "pane geometry contains a duplicate or empty column",
                    ));
                }
                for pane_id in &column.pane_ids {
                    if !pane_ids.insert(pane_id.clone()) || pane_ids.len() > MAX_PANES {
                        return Err(error(
                            ReceiptErrorKind::InvalidEvent,
                            "pane geometry IDs must be globally unique and bounded",
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}
