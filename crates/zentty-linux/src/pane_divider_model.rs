//! Display-independent pane-divider interaction semantics.
//!
//! GTK translates physical events into [`DividerKey`] or pointer coordinates;
//! this model decides which topology boundary is targeted and constructs the
//! resize payload delivered to the workspace callback.

pub const KEYBOARD_RESIZE_STEP: f64 = 16.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DividerAxis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DividerKey {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PaneDivider {
    Column {
        after_column_id: String,
    },
    Pane {
        column_id: String,
        after_pane_id: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct DividerResize {
    pub target: PaneDivider,
    pub axis: DividerAxis,
    pub delta: f64,
}

impl DividerResize {
    pub fn apply(self, callback: impl FnOnce(&Self) -> f64) -> f64 {
        callback(&self)
    }
}

impl PaneDivider {
    #[must_use]
    pub fn axis(&self) -> DividerAxis {
        match self {
            Self::Column { .. } => DividerAxis::Horizontal,
            Self::Pane { .. } => DividerAxis::Vertical,
        }
    }

    #[must_use]
    pub fn pointer_offset(&self, x: f64, y: f64) -> f64 {
        match self.axis() {
            DividerAxis::Horizontal => x,
            DividerAxis::Vertical => y,
        }
    }

    #[must_use]
    pub fn resize_request(&self, delta: f64) -> Option<DividerResize> {
        if delta.abs() <= f64::EPSILON {
            return None;
        }
        Some(DividerResize {
            target: self.clone(),
            axis: self.axis(),
            delta,
        })
    }

    #[must_use]
    pub fn keyboard_request(&self, key: DividerKey) -> Option<DividerResize> {
        let delta = match (self.axis(), key) {
            (DividerAxis::Horizontal, DividerKey::Left)
            | (DividerAxis::Vertical, DividerKey::Up) => -KEYBOARD_RESIZE_STEP,
            (DividerAxis::Horizontal, DividerKey::Right)
            | (DividerAxis::Vertical, DividerKey::Down) => KEYBOARD_RESIZE_STEP,
            _ => return None,
        };
        self.resize_request(delta)
    }

    #[must_use]
    pub fn widget_name(&self) -> String {
        match self {
            Self::Column { after_column_id } => {
                format!("pane-divider-column-after-{after_column_id}")
            }
            Self::Pane {
                column_id,
                after_pane_id,
            } => format!("pane-divider-{column_id}-after-{after_pane_id}"),
        }
    }

    #[must_use]
    pub fn accessible_label(&self) -> String {
        match self {
            Self::Column { after_column_id } => {
                format!("Resize columns after {after_column_id}")
            }
            Self::Pane { after_pane_id, .. } => {
                format!("Resize panes after {after_pane_id}")
            }
        }
    }
}

#[must_use]
pub fn adjusted_vertical_margin(current: i32, applied_delta: f64) -> i32 {
    let margin = (f64::from(current) + applied_delta)
        .round()
        .clamp(0.0, f64::from(i32::MAX));
    #[allow(clippy::cast_possible_truncation)]
    {
        margin as i32
    }
}
