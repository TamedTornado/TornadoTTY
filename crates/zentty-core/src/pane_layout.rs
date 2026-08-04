#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PaneRightInsertionBehavior {
    VisibleSplit,
    WorklaneAdd,
}

pub struct PaneLayoutPolicy;

impl PaneLayoutPolicy {
    pub const ADAPTIVE_VISIBLE_SPLIT_THRESHOLD: i32 = 1920;
    pub const INTER_PANE_SPACING: i32 = 1;

    #[must_use]
    pub const fn adaptive_right_behavior(viewport_width: i32) -> PaneRightInsertionBehavior {
        if viewport_width >= Self::ADAPTIVE_VISIBLE_SPLIT_THRESHOLD {
            PaneRightInsertionBehavior::VisibleSplit
        } else {
            PaneRightInsertionBehavior::WorklaneAdd
        }
    }

    #[must_use]
    pub fn visible_split_width(available_width: i32) -> i32 {
        available_width
            .saturating_sub(Self::INTER_PANE_SPACING)
            .max(2)
            / 2
    }
}

#[cfg(test)]
mod tests {
    use super::{PaneLayoutPolicy, PaneRightInsertionBehavior};

    const SOURCE: &str = include_str!("../../../Zentty/Layout/PaneLayoutPreferences.swift");

    #[test]
    fn default_adaptive_policy_preserves_the_two_distinct_source_commands() {
        assert_eq!(
            PaneLayoutPolicy::adaptive_right_behavior(1919),
            PaneRightInsertionBehavior::WorklaneAdd
        );
        assert_eq!(
            PaneLayoutPolicy::adaptive_right_behavior(1920),
            PaneRightInsertionBehavior::VisibleSplit
        );
        assert_eq!(PaneLayoutPolicy::visible_split_width(1000), 499);
        assert!(SOURCE.contains("visibleSplitWindowWidth: .px1920"));
        assert!(SOURCE.contains("case .visibleSplit:"));
        assert!(SOURCE.contains("case .worklaneAdd:"));
        assert!(SOURCE.contains("var visibleSplitColumnWidth: CGFloat"));
        assert!(SOURCE.contains("(availableWidth - sizing.interPaneSpacing) / 2"));
        assert!(SOURCE.contains("Add Pane Right"));
    }
}
