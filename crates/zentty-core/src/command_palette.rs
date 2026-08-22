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
    pub recent_eligible: bool,
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
            recent_eligible: false,
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
            recent_eligible: true,
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
            recent_eligible: false,
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
            recent_eligible: false,
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

    #[must_use]
    pub fn with_recent_eligibility(mut self, eligible: bool) -> Self {
        self.recent_eligible = eligible;
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandPaletteSectionKind {
    Actions,
    RecentPanes,
    RecentActions,
    Results,
}

impl CommandPaletteSectionKind {
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::Actions => "Actions",
            Self::RecentPanes => "Recent Panes",
            Self::RecentActions => "Recent Actions",
            Self::Results => "Results",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandPaletteSection {
    pub kind: CommandPaletteSectionKind,
    pub items: Vec<CommandPaletteItem>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RecentCommandTargets {
    targets: Vec<CommandPaletteTarget>,
}

impl RecentCommandTargets {
    pub const MAX_RECENT: usize = 8;

    pub fn record(&mut self, item: &CommandPaletteItem) {
        if !item.enabled || !item.recent_eligible {
            return;
        }
        self.targets.retain(|target| target != &item.target);
        self.targets.insert(0, item.target.clone());
        self.targets.truncate(Self::MAX_RECENT);
    }

    #[must_use]
    pub fn resolve(&self, items: &[CommandPaletteItem]) -> Vec<CommandPaletteItem> {
        self.targets
            .iter()
            .filter_map(|target| {
                items
                    .iter()
                    .find(|item| item.enabled && item.recent_eligible && &item.target == target)
                    .cloned()
            })
            .collect()
    }
}

#[must_use]
pub fn resolve_command_palette_sections(
    query: &str,
    items: &[CommandPaletteItem],
    recent_panes: &[PaneReference],
    current_pane: Option<&PaneReference>,
    recent_commands: &RecentCommandTargets,
    immediate_actions: &[CommandPaletteTarget],
) -> Vec<CommandPaletteSection> {
    if !normalize(query).is_empty() {
        let items = resolve_command_palette(query, items, recent_panes, current_pane);
        return (!items.is_empty())
            .then_some(CommandPaletteSection {
                kind: CommandPaletteSectionKind::Results,
                items,
            })
            .into_iter()
            .collect();
    }

    let actions = immediate_actions
        .iter()
        .filter_map(|target| {
            items
                .iter()
                .find(|item| item.enabled && &item.target == target)
                .cloned()
        })
        .collect::<Vec<_>>();
    let recent_panes = resolve_command_palette("", items, recent_panes, current_pane)
        .into_iter()
        .filter(|item| item.group == CommandPaletteGroup::Pane)
        .collect::<Vec<_>>();
    let recent_actions = recent_commands
        .resolve(items)
        .into_iter()
        .filter(|item| !actions.iter().any(|action| action.target == item.target))
        .collect::<Vec<_>>();

    [
        (CommandPaletteSectionKind::Actions, actions),
        (CommandPaletteSectionKind::RecentPanes, recent_panes),
        (CommandPaletteSectionKind::RecentActions, recent_actions),
    ]
    .into_iter()
    .filter_map(|(kind, items)| {
        (!items.is_empty()).then_some(CommandPaletteSection { kind, items })
    })
    .collect()
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
    use super::{
        CommandPaletteItem, CommandPaletteSectionKind, CommandPaletteTarget, RecentCommandTargets,
        resolve_command_palette, resolve_command_palette_sections,
    };
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

    #[test]
    fn recent_commands_follow_source_eligibility_deduplication_and_capacity() {
        let mut recent = RecentCommandTargets::default();
        let pane = pane("lane-1", "pane-1", "Shell");
        recent.record(&pane);
        assert!(recent.targets.is_empty());
        assert!(recent.resolve(std::slice::from_ref(&pane)).is_empty());

        let items = (0..9)
            .map(|index| {
                CommandPaletteItem::parameterized_action(
                    format!("Action {index}"),
                    "Dynamic action",
                    "action",
                    "dynamic-action",
                    index.to_string(),
                )
                .with_recent_eligibility(true)
            })
            .collect::<Vec<_>>();
        for item in &items {
            recent.record(item);
        }
        recent.record(&items[4]);

        let resolved = recent.resolve(&items);
        assert_eq!(resolved.len(), RecentCommandTargets::MAX_RECENT);
        assert_eq!(resolved[0].target, items[4].target);
        assert_eq!(
            resolved
                .iter()
                .filter(|item| item.target == items[4].target)
                .count(),
            1
        );
        assert!(!resolved.iter().any(|item| item.target == items[0].target));
    }

    #[test]
    fn recent_commands_prune_disabled_stale_and_ineligible_targets() {
        let eligible = CommandPaletteItem::parameterized_action(
            "Open Editor",
            "Open With",
            "editor",
            "open-with",
            "editor",
        )
        .with_recent_eligibility(true);
        let mut recent = RecentCommandTargets::default();
        recent.record(&eligible);

        let mut disabled = eligible.clone();
        disabled.enabled = false;
        let mut disabled_recent = RecentCommandTargets::default();
        disabled_recent.record(&disabled);
        assert!(disabled_recent.targets.is_empty());
        assert!(recent.resolve(&[disabled]).is_empty());
        assert!(recent.resolve(&[]).is_empty());

        let ineligible = CommandPaletteItem::parameterized_action(
            "Run tests",
            "Task Runner",
            "task",
            "run-task",
            "tests",
        );
        assert!(recent.resolve(&[ineligible]).is_empty());
    }

    #[test]
    fn empty_results_have_source_sections_without_duplicates_or_current_pane() {
        let current = PaneReference::new("lane-1", "pane-1");
        let recent_pane = PaneReference::new("lane-1", "pane-2");
        let new_lane = CommandPaletteItem::action(
            "New Worklane",
            "Create a worklane",
            "workspace",
            "new-worklane",
        );
        let settings = CommandPaletteItem::action(
            "Open Settings",
            "Configure Zentty",
            "settings",
            "open-settings",
        );
        let items = vec![
            pane("lane-1", "pane-1", "Current"),
            pane("lane-1", "pane-2", "Recent"),
            new_lane.clone(),
            settings.clone(),
        ];
        let mut recent = RecentCommandTargets::default();
        recent.record(&settings);
        let sections = resolve_command_palette_sections(
            "",
            &items,
            &[current.clone(), recent_pane.clone(), recent_pane.clone()],
            Some(&current),
            &recent,
            std::slice::from_ref(&new_lane.target),
        );

        assert_eq!(
            sections
                .iter()
                .map(|section| section.kind)
                .collect::<Vec<_>>(),
            [
                CommandPaletteSectionKind::Actions,
                CommandPaletteSectionKind::RecentPanes,
                CommandPaletteSectionKind::RecentActions,
            ]
        );
        assert_eq!(sections[0].items, [new_lane]);
        assert_eq!(
            sections[1].items[0].target,
            CommandPaletteTarget::Pane(recent_pane)
        );
        assert_eq!(sections[2].items, [settings]);
        assert_eq!(
            sections
                .iter()
                .map(|section| section.kind.title())
                .collect::<Vec<_>>(),
            ["Actions", "Recent Panes", "Recent Actions"]
        );
    }

    #[test]
    fn immediate_action_is_not_repeated_in_recent_actions() {
        let action =
            CommandPaletteItem::action("New Worklane", "Create", "workspace", "new-worklane");
        let mut recent = RecentCommandTargets::default();
        recent.record(&action);
        let sections = resolve_command_palette_sections(
            "",
            std::slice::from_ref(&action),
            &[],
            None,
            &recent,
            std::slice::from_ref(&action.target),
        );
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].kind, CommandPaletteSectionKind::Actions);
    }

    #[test]
    fn active_query_returns_only_the_results_section() {
        let items = vec![
            pane("lane-1", "pane-1", "Tests"),
            CommandPaletteItem::action("Settings", "Configure", "preferences", "settings"),
        ];
        let sections = resolve_command_palette_sections(
            "tests",
            &items,
            &[],
            None,
            &RecentCommandTargets::default(),
            &[],
        );
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].kind, CommandPaletteSectionKind::Results);
        assert_eq!(sections[0].kind.title(), "Results");
        assert_eq!(sections[0].items[0].title, "Tests");
    }
}
