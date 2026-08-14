use crate::PaneReference;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandPaletteGroup {
    Pane,
    Action,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandPaletteTarget {
    Pane(PaneReference),
    Action(&'static str),
    ParameterizedAction {
        action: &'static str,
        parameter: String,
    },
    TripleParameterizedAction {
        action: &'static str,
        parameters: (String, String, String),
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandPaletteItem {
    pub title: String,
    pub subtitle: String,
    pub search_text: String,
    pub group: CommandPaletteGroup,
    pub target: CommandPaletteTarget,
    pub enabled: bool,
}

impl CommandPaletteItem {
    #[must_use]
    pub fn pane(
        title: impl Into<String>,
        subtitle: impl Into<String>,
        target: PaneReference,
    ) -> Self {
        let title = title.into();
        let subtitle = subtitle.into();
        Self {
            search_text: normalize(&format!("{title} {subtitle}")),
            title,
            subtitle,
            group: CommandPaletteGroup::Pane,
            target: CommandPaletteTarget::Pane(target),
            enabled: true,
        }
    }

    #[must_use]
    pub fn action(
        title: impl Into<String>,
        subtitle: impl Into<String>,
        keywords: &str,
        action: &'static str,
    ) -> Self {
        let title = title.into();
        let subtitle = subtitle.into();
        Self {
            search_text: normalize(&format!("{title} {subtitle} {keywords}")),
            title,
            subtitle,
            group: CommandPaletteGroup::Action,
            target: CommandPaletteTarget::Action(action),
            enabled: true,
        }
    }

    #[must_use]
    pub fn parameterized_action(
        title: impl Into<String>,
        subtitle: impl Into<String>,
        keywords: &str,
        action: &'static str,
        parameter: impl Into<String>,
    ) -> Self {
        let title = title.into();
        let subtitle = subtitle.into();
        Self {
            search_text: normalize(&format!("{title} {subtitle} {keywords}")),
            title,
            subtitle,
            group: CommandPaletteGroup::Action,
            target: CommandPaletteTarget::ParameterizedAction {
                action,
                parameter: parameter.into(),
            },
            enabled: true,
        }
    }

    #[must_use]
    pub fn triple_parameterized_action(
        title: impl Into<String>,
        subtitle: impl Into<String>,
        keywords: &str,
        action: &'static str,
        parameters: (String, String, String),
    ) -> Self {
        let title = title.into();
        let subtitle = subtitle.into();
        Self {
            search_text: normalize(&format!("{title} {subtitle} {keywords}")),
            title,
            subtitle,
            group: CommandPaletteGroup::Action,
            target: CommandPaletteTarget::TripleParameterizedAction { action, parameters },
            enabled: true,
        }
    }

    #[must_use]
    pub fn parameterized_action_with_enabled(
        title: impl Into<String>,
        subtitle: impl Into<String>,
        keywords: &str,
        action: &'static str,
        parameter: impl Into<String>,
        enabled: bool,
    ) -> Self {
        let mut item = Self::parameterized_action(title, subtitle, keywords, action, parameter);
        item.enabled = enabled;
        item
    }
}

/// Resolves a compact source-style command-palette list. Empty search puts
/// recent panes first and excludes the current pane; active search ranks exact
/// title, title prefix, token-prefix, then ordinary substring matches.
#[must_use]
pub fn resolve_command_palette(
    query: &str,
    items: &[CommandPaletteItem],
    recent_panes: &[PaneReference],
    current_pane: Option<&PaneReference>,
) -> Vec<CommandPaletteItem> {
    let query = normalize(query);
    if query.is_empty() {
        let mut resolved = Vec::new();
        for reference in recent_panes {
            if Some(reference) == current_pane
                || resolved.iter().any(|item: &CommandPaletteItem| {
                    item.target == CommandPaletteTarget::Pane(reference.clone())
                })
            {
                continue;
            }
            if let Some(item) = items
                .iter()
                .find(|item| item.target == CommandPaletteTarget::Pane(reference.clone()))
            {
                resolved.push(item.clone());
            }
        }
        resolved.extend(
            items
                .iter()
                .filter(|item| item.group == CommandPaletteGroup::Action)
                .take(5)
                .cloned(),
        );
        return resolved;
    }

    let mut scored = items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| match_score(&query, item).map(|score| (score, index, item)))
        .collect::<Vec<_>>();
    scored.sort_by_key(|(score, index, _)| (*score, *index));
    scored
        .into_iter()
        .map(|(_, _, item)| item.clone())
        .collect()
}

fn match_score(query: &str, item: &CommandPaletteItem) -> Option<u8> {
    let title = normalize(&item.title);
    if title == query {
        return Some(0);
    }
    if title.starts_with(query) {
        return Some(1);
    }
    if item
        .search_text
        .split_whitespace()
        .any(|word| word.starts_with(query))
    {
        return Some(2);
    }
    item.search_text.contains(query).then_some(3)
}

fn normalize(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::{CommandPaletteItem, CommandPaletteTarget, resolve_command_palette};
    use crate::PaneReference;

    fn pane(lane: &str, pane: &str, title: &str) -> CommandPaletteItem {
        CommandPaletteItem::pane(
            title,
            format!("{lane} · /tmp/project"),
            PaneReference::new(lane, pane),
        )
    }

    #[test]
    fn empty_palette_shows_unique_recent_panes_then_actions_without_current_pane() {
        let one = PaneReference::new("lane-1", "pane-1");
        let two = PaneReference::new("lane-1", "pane-2");
        let items = vec![
            pane("lane-1", "pane-1", "Editor"),
            pane("lane-1", "pane-2", "Tests"),
            CommandPaletteItem::action(
                "New Worklane",
                "Create a worklane",
                "workspace",
                "new-worklane",
            ),
        ];
        let resolved = resolve_command_palette(
            "",
            &items,
            &[two.clone(), one.clone(), two.clone()],
            Some(&one),
        );
        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved[0].target, CommandPaletteTarget::Pane(two));
        assert_eq!(
            resolved[1].target,
            CommandPaletteTarget::Action("new-worklane")
        );
    }

    #[test]
    fn active_search_ranks_exact_title_before_prefix_token_and_substring() {
        let items = vec![
            pane("lane-1", "pane-1", "Frontend tests"),
            pane("lane-2", "pane-2", "Tests"),
            pane("lane-3", "pane-3", "Integration tests"),
            pane("lane-4", "pane-4", "Protests output"),
        ];
        let resolved = resolve_command_palette("tests", &items, &[], None);
        assert_eq!(
            resolved
                .iter()
                .map(|item| item.title.as_str())
                .collect::<Vec<_>>(),
            [
                "Tests",
                "Frontend tests",
                "Integration tests",
                "Protests output"
            ]
        );

        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../Zentty/UI/CommandPalette/CommandPaletteResultsResolver.swift"
        ));
        assert!(source.contains("Recent Panes"));
        assert!(source.contains("promotedMatch == .exact"));
    }

    #[test]
    fn parameterized_action_keeps_opaque_parameter_out_of_the_action_name() {
        let item = CommandPaletteItem::parameterized_action(
            "Open localhost:5173",
            "Development server",
            "browser server",
            "open-server",
            "http://localhost:5173",
        );
        assert_eq!(
            item.target,
            CommandPaletteTarget::ParameterizedAction {
                action: "open-server",
                parameter: "http://localhost:5173".into(),
            }
        );
    }

    #[test]
    fn triple_parameterized_action_preserves_exact_cross_window_identity() {
        let item = CommandPaletteItem::triple_parameterized_action(
            "Focus Claude Code",
            "Requires approval · project",
            "agent fleet",
            "activate-fleet-pane",
            ("window-2".into(), "worklane-3".into(), "pane-7".into()),
        );
        assert_eq!(
            item.target,
            CommandPaletteTarget::TripleParameterizedAction {
                action: "activate-fleet-pane",
                parameters: ("window-2".into(), "worklane-3".into(), "pane-7".into()),
            }
        );
        assert!(item.search_text.contains("agent fleet"));
    }
}
