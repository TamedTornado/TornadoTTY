use serde_json::{Value, json};

const PAYLOAD_VERSION: u64 = 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PaneDragPayload {
    pub(crate) source_window_id: String,
    pub(crate) source_worklane_id: String,
    pub(crate) source_column_id: String,
    pub(crate) pane_id: String,
    pub(crate) source_generation: u64,
    pub(crate) presentation: PaneDragPresentation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PaneDragPresentation {
    pub(crate) worklane_title: String,
    pub(crate) pane_title: String,
    pub(crate) context: String,
    pub(crate) agent_status: Option<String>,
}

impl PaneDragPayload {
    pub(crate) fn encode(&self) -> String {
        json!({
            "version": PAYLOAD_VERSION,
            "sourceWindowID": self.source_window_id,
            "sourceWorklaneID": self.source_worklane_id,
            "sourceColumnID": self.source_column_id,
            "paneID": self.pane_id,
            "sourceGeneration": self.source_generation,
            "presentation": {
                "worklaneTitle": self.presentation.worklane_title,
                "paneTitle": self.presentation.pane_title,
                "context": self.presentation.context,
                "agentStatus": self.presentation.agent_status,
            },
        })
        .to_string()
    }

    pub(crate) fn decode(value: &str) -> Option<Self> {
        let value: Value = serde_json::from_str(value).ok()?;
        if value.get("version")?.as_u64()? != PAYLOAD_VERSION {
            return None;
        }
        let presentation = value.get("presentation")?;
        let payload = Self {
            source_window_id: value.get("sourceWindowID")?.as_str()?.to_owned(),
            source_worklane_id: value.get("sourceWorklaneID")?.as_str()?.to_owned(),
            source_column_id: value.get("sourceColumnID")?.as_str()?.to_owned(),
            pane_id: value.get("paneID")?.as_str()?.to_owned(),
            source_generation: value.get("sourceGeneration")?.as_u64()?,
            presentation: PaneDragPresentation {
                worklane_title: presentation.get("worklaneTitle")?.as_str()?.to_owned(),
                pane_title: presentation.get("paneTitle")?.as_str()?.to_owned(),
                context: presentation.get("context")?.as_str()?.to_owned(),
                agent_status: match presentation.get("agentStatus")? {
                    Value::Null => None,
                    value => Some(value.as_str()?.to_owned()),
                },
            },
        };
        (!payload.source_window_id.is_empty()
            && !payload.source_worklane_id.is_empty()
            && !payload.source_column_id.is_empty()
            && !payload.pane_id.is_empty())
        .then_some(payload)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SplitAxis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SidebarDropTarget {
    Worklane {
        window_id: String,
        worklane_id: String,
        pane_index: Option<usize>,
        generation: u64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StackGapHit {
    pub(crate) window_id: String,
    pub(crate) worklane_id: String,
    pub(crate) column_id: String,
    pub(crate) pane_index: usize,
    pub(crate) generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SplitHit {
    pub(crate) window_id: String,
    pub(crate) worklane_id: String,
    pub(crate) target_pane_id: String,
    pub(crate) axis: SplitAxis,
    pub(crate) leading: bool,
    pub(crate) generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ColumnInsertionHit {
    pub(crate) window_id: String,
    pub(crate) worklane_id: String,
    pub(crate) column_index: usize,
    pub(crate) generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PaneDropInput {
    pub(crate) payload: PaneDragPayload,
    pub(crate) sidebar_target: Option<SidebarDropTarget>,
    pub(crate) stack_gap_hit: Option<StackGapHit>,
    pub(crate) split_hit: Option<SplitHit>,
    pub(crate) column_insertion_hit: Option<ColumnInsertionHit>,
    pub(crate) is_duplicate: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CanvasHit {
    StackGap { after: bool },
    Split { axis: SplitAxis, leading: bool },
    ColumnGap { after: bool },
}

const CANVAS_GAP_ZONE: f64 = 16.0;

pub(crate) fn canvas_hit(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    column_pane_count: usize,
) -> Option<CanvasHit> {
    if width <= 0.0 || height <= 0.0 || x < 0.0 || y < 0.0 || x > width || y > height {
        return None;
    }
    if y <= CANVAS_GAP_ZONE {
        return Some(CanvasHit::StackGap { after: false });
    }
    if height - y <= CANVAS_GAP_ZONE {
        return Some(CanvasHit::StackGap { after: true });
    }
    let nx = x / width;
    let ny = y / height;
    let candidates = [
        (nx, SplitAxis::Horizontal, true),
        (1.0 - nx, SplitAxis::Horizontal, false),
        (ny, SplitAxis::Vertical, true),
        (1.0 - ny, SplitAxis::Vertical, false),
    ];
    if let Some((_, axis, leading)) = candidates
        .into_iter()
        .filter(|(distance, axis, _)| {
            *distance <= 0.28
                && match axis {
                    SplitAxis::Horizontal => column_pane_count == 1,
                    SplitAxis::Vertical => height / 2.0 >= 160.0,
                }
        })
        .min_by(|left, right| left.0.total_cmp(&right.0))
    {
        return Some(CanvasHit::Split { axis, leading });
    }
    if x <= CANVAS_GAP_ZONE {
        return Some(CanvasHit::ColumnGap { after: false });
    }
    if width - x <= CANVAS_GAP_ZONE {
        return Some(CanvasHit::ColumnGap { after: true });
    }
    None
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PaneDropOutcome {
    ReorderColumn {
        payload: PaneDragPayload,
        hit: ColumnInsertionHit,
        is_duplicate: bool,
    },
    ReorderInColumn {
        payload: PaneDragPayload,
        hit: StackGapHit,
        is_duplicate: bool,
    },
    Split {
        payload: PaneDragPayload,
        hit: SplitHit,
        is_duplicate: bool,
    },
    Worklane {
        payload: PaneDragPayload,
        target: SidebarDropTarget,
        is_duplicate: bool,
    },
}

impl PaneDropOutcome {
    pub(crate) fn destination_window_id(&self) -> &str {
        match self {
            Self::ReorderColumn { hit, .. } => &hit.window_id,
            Self::ReorderInColumn { hit, .. } => &hit.window_id,
            Self::Split { hit, .. } => &hit.window_id,
            Self::Worklane {
                target: SidebarDropTarget::Worklane { window_id, .. },
                ..
            } => window_id,
        }
    }

    pub(crate) fn destination_generation(&self) -> u64 {
        match self {
            Self::ReorderColumn { hit, .. } => hit.generation,
            Self::ReorderInColumn { hit, .. } => hit.generation,
            Self::Split { hit, .. } => hit.generation,
            Self::Worklane {
                target: SidebarDropTarget::Worklane { generation, .. },
                ..
            } => *generation,
        }
    }

    pub(crate) fn payload(&self) -> &PaneDragPayload {
        match self {
            Self::ReorderColumn { payload, .. }
            | Self::ReorderInColumn { payload, .. }
            | Self::Split { payload, .. }
            | Self::Worklane { payload, .. } => payload,
        }
    }
}

pub(crate) fn resolve(input: PaneDropInput) -> Option<PaneDropOutcome> {
    let PaneDropInput {
        payload,
        sidebar_target,
        stack_gap_hit,
        split_hit,
        column_insertion_hit,
        is_duplicate,
    } = input;
    if let Some(target) = sidebar_target {
        return Some(PaneDropOutcome::Worklane {
            payload,
            target,
            is_duplicate,
        });
    }
    if let Some(hit) = stack_gap_hit {
        return Some(PaneDropOutcome::ReorderInColumn {
            payload,
            hit,
            is_duplicate,
        });
    }
    if let Some(hit) = split_hit {
        return Some(PaneDropOutcome::Split {
            payload,
            hit,
            is_duplicate,
        });
    }
    column_insertion_hit.map(|hit| PaneDropOutcome::ReorderColumn {
        payload,
        hit,
        is_duplicate,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ValidationError {
    SourceGenerationChanged,
    DestinationGenerationChanged,
    SamePane,
    DuplicateUnsupported,
}

pub(crate) fn validate(
    outcome: &PaneDropOutcome,
    current_source_generation: u64,
    current_destination_generation: u64,
) -> Result<(), ValidationError> {
    if outcome.payload().source_generation != current_source_generation {
        return Err(ValidationError::SourceGenerationChanged);
    }
    if outcome.destination_generation() != current_destination_generation {
        return Err(ValidationError::DestinationGenerationChanged);
    }
    if matches!(
        outcome,
        PaneDropOutcome::Split { payload, hit, .. } if payload.pane_id == hit.target_pane_id
    ) {
        return Err(ValidationError::SamePane);
    }
    let duplicate = match outcome {
        PaneDropOutcome::ReorderColumn { is_duplicate, .. }
        | PaneDropOutcome::ReorderInColumn { is_duplicate, .. }
        | PaneDropOutcome::Split { is_duplicate, .. }
        | PaneDropOutcome::Worklane { is_duplicate, .. } => *is_duplicate,
    };
    if duplicate {
        return Err(ValidationError::DuplicateUnsupported);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload() -> PaneDragPayload {
        PaneDragPayload {
            source_window_id: "window-a".to_owned(),
            source_worklane_id: "lane-a".to_owned(),
            source_column_id: "column-a".to_owned(),
            pane_id: "pane-a".to_owned(),
            source_generation: 7,
            presentation: PaneDragPresentation {
                worklane_title: "Frontend".to_owned(),
                pane_title: "pnpm dev".to_owned(),
                context: "frontend • main".to_owned(),
                agent_status: Some("Codex · Running".to_owned()),
            },
        }
    }

    fn worklane_target() -> SidebarDropTarget {
        SidebarDropTarget::Worklane {
            window_id: "window-b".to_owned(),
            worklane_id: "lane-b".to_owned(),
            pane_index: Some(2),
            generation: 11,
        }
    }

    fn base_input() -> PaneDropInput {
        PaneDropInput {
            payload: payload(),
            sidebar_target: None,
            stack_gap_hit: None,
            split_hit: None,
            column_insertion_hit: None,
            is_duplicate: false,
        }
    }

    #[test]
    fn pane_drag_payload_round_trip_is_versioned_and_rejects_malformed_identity() {
        let payload = payload();
        assert_eq!(PaneDragPayload::decode(&payload.encode()), Some(payload));
        assert!(PaneDragPayload::decode("not-json").is_none());
        assert!(PaneDragPayload::decode(r#"{"version":2}"#).is_none());
        assert!(PaneDragPayload::decode(
            r#"{"version":1,"sourceWindowID":"","sourceWorklaneID":"lane","sourceColumnID":"column","paneID":"pane","sourceGeneration":1,"presentation":{"worklaneTitle":"Lane","paneTitle":"Pane","context":"","agentStatus":null}}"#
        )
        .is_none());
    }

    #[test]
    fn pane_drag_resolver_matches_source_precedence_and_cancels_without_a_hit() {
        assert_eq!(resolve(base_input()), None);
        let mut input = base_input();
        input.sidebar_target = Some(worklane_target());
        input.stack_gap_hit = Some(StackGapHit {
            window_id: "window-a".to_owned(),
            worklane_id: "lane-a".to_owned(),
            column_id: "column-a".to_owned(),
            pane_index: 1,
            generation: 7,
        });
        input.split_hit = Some(SplitHit {
            window_id: "window-a".to_owned(),
            worklane_id: "lane-a".to_owned(),
            target_pane_id: "pane-b".to_owned(),
            axis: SplitAxis::Vertical,
            leading: false,
            generation: 7,
        });
        input.column_insertion_hit = Some(ColumnInsertionHit {
            window_id: "window-a".to_owned(),
            worklane_id: "lane-a".to_owned(),
            column_index: 2,
            generation: 7,
        });
        assert!(matches!(
            resolve(input),
            Some(PaneDropOutcome::Worklane { .. })
        ));

        let mut input = base_input();
        input.stack_gap_hit = Some(StackGapHit {
            window_id: "window-a".to_owned(),
            worklane_id: "lane-a".to_owned(),
            column_id: "column-a".to_owned(),
            pane_index: 1,
            generation: 7,
        });
        input.split_hit = Some(SplitHit {
            window_id: "window-a".to_owned(),
            worklane_id: "lane-a".to_owned(),
            target_pane_id: "pane-b".to_owned(),
            axis: SplitAxis::Horizontal,
            leading: true,
            generation: 7,
        });
        assert!(matches!(
            resolve(input),
            Some(PaneDropOutcome::ReorderInColumn { .. })
        ));
    }

    #[test]
    fn pane_drag_resolver_threads_duplicate_and_exact_pane_identity_through_every_outcome() {
        let mut input = base_input();
        input.is_duplicate = true;
        input.sidebar_target = Some(worklane_target());
        let outcome = resolve(input).expect("worklane target resolves");
        assert_eq!(outcome.payload().pane_id, "pane-a");
        assert_eq!(
            validate(&outcome, 7, 11),
            Err(ValidationError::DuplicateUnsupported)
        );
    }

    #[test]
    fn pane_drag_validation_fails_closed_for_stale_source_destination_and_self_split() {
        let mut input = base_input();
        input.sidebar_target = Some(worklane_target());
        let outcome = resolve(input).expect("worklane target resolves");
        assert_eq!(
            validate(&outcome, 8, 11),
            Err(ValidationError::SourceGenerationChanged)
        );
        assert_eq!(
            validate(&outcome, 7, 12),
            Err(ValidationError::DestinationGenerationChanged)
        );

        let mut input = base_input();
        input.split_hit = Some(SplitHit {
            window_id: "window-a".to_owned(),
            worklane_id: "lane-a".to_owned(),
            target_pane_id: "pane-a".to_owned(),
            axis: SplitAxis::Vertical,
            leading: true,
            generation: 7,
        });
        let outcome = resolve(input).expect("split target resolves");
        assert_eq!(validate(&outcome, 7, 7), Err(ValidationError::SamePane));
    }

    #[test]
    fn pane_drag_all_source_outcome_shapes_are_represented() {
        let mut input = base_input();
        input.column_insertion_hit = Some(ColumnInsertionHit {
            window_id: "window-a".to_owned(),
            worklane_id: "lane-a".to_owned(),
            column_index: 0,
            generation: 7,
        });
        assert!(matches!(
            resolve(input),
            Some(PaneDropOutcome::ReorderColumn { .. })
        ));

        let mut input = base_input();
        input.split_hit = Some(SplitHit {
            window_id: "window-a".to_owned(),
            worklane_id: "lane-a".to_owned(),
            target_pane_id: "pane-b".to_owned(),
            axis: SplitAxis::Horizontal,
            leading: false,
            generation: 7,
        });
        assert!(matches!(
            resolve(input),
            Some(PaneDropOutcome::Split { .. })
        ));
    }

    #[test]
    fn pane_drag_canvas_hit_uses_stack_then_split_then_column_and_keeps_a_dead_center() {
        for invalid in [
            canvas_hit(0.0, 0.0, 0.0, 400.0, 1),
            canvas_hit(0.0, 0.0, 400.0, 0.0, 1),
            canvas_hit(-1.0, 200.0, 400.0, 400.0, 1),
            canvas_hit(200.0, -1.0, 400.0, 400.0, 1),
            canvas_hit(401.0, 200.0, 400.0, 400.0, 1),
            canvas_hit(200.0, 401.0, 400.0, 400.0, 1),
        ] {
            assert_eq!(invalid, None);
        }
        assert_eq!(
            canvas_hit(100.0, 3.0, 400.0, 400.0, 2),
            Some(CanvasHit::StackGap { after: false })
        );
        assert_eq!(
            canvas_hit(200.0, 0.0, 400.0, 400.0, 2),
            Some(CanvasHit::StackGap { after: false })
        );
        assert_eq!(
            canvas_hit(100.0, 397.0, 400.0, 400.0, 2),
            Some(CanvasHit::StackGap { after: true })
        );
        assert_eq!(
            canvas_hit(200.0, 400.0, 400.0, 400.0, 2),
            Some(CanvasHit::StackGap { after: true })
        );
        assert_eq!(
            canvas_hit(4.0, 200.0, 400.0, 400.0, 1),
            Some(CanvasHit::Split {
                axis: SplitAxis::Horizontal,
                leading: true
            })
        );
        assert_eq!(
            canvas_hit(4.0, 200.0, 400.0, 300.0, 2),
            Some(CanvasHit::ColumnGap { after: false })
        );
        assert_eq!(
            canvas_hit(0.0, 200.0, 400.0, 300.0, 2),
            Some(CanvasHit::ColumnGap { after: false })
        );
        assert_eq!(
            canvas_hit(396.0, 200.0, 400.0, 300.0, 2),
            Some(CanvasHit::ColumnGap { after: true })
        );
        assert_eq!(
            canvas_hit(200.0, 100.0, 400.0, 400.0, 2),
            Some(CanvasHit::Split {
                axis: SplitAxis::Vertical,
                leading: true
            })
        );
        assert_eq!(
            canvas_hit(200.0, 300.0, 400.0, 400.0, 2),
            Some(CanvasHit::Split {
                axis: SplitAxis::Vertical,
                leading: false
            })
        );
        assert_eq!(
            canvas_hit(112.0, 200.0, 400.0, 400.0, 1),
            Some(CanvasHit::Split {
                axis: SplitAxis::Horizontal,
                leading: true
            })
        );
        assert_eq!(canvas_hit(113.0, 200.0, 400.0, 300.0, 1), None);
        assert_eq!(
            canvas_hit(200.0, 80.0, 400.0, 320.0, 2),
            Some(CanvasHit::Split {
                axis: SplitAxis::Vertical,
                leading: true
            })
        );
        assert_eq!(canvas_hit(200.0, 79.0, 400.0, 318.0, 2), None);
        assert_eq!(
            canvas_hit(400.0, 200.0, 400.0, 400.0, 1),
            Some(CanvasHit::Split {
                axis: SplitAxis::Horizontal,
                leading: false
            })
        );
        assert_eq!(canvas_hit(200.0, 200.0, 400.0, 400.0, 1), None);
    }
}
