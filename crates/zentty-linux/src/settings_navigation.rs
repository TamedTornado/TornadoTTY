#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SettingsSection {
    General,
    Appearance,
    Shortcuts,
    Notifications,
    UpdatesPrivacy,
    PaneLayout,
    OpenWith,
    DevServers,
    Agents,
}

impl SettingsSection {
    pub(crate) const ALL: [Self; 9] = [
        Self::General,
        Self::Appearance,
        Self::Shortcuts,
        Self::Notifications,
        Self::UpdatesPrivacy,
        Self::PaneLayout,
        Self::OpenWith,
        Self::DevServers,
        Self::Agents,
    ];

    pub(crate) const fn id(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Appearance => "appearance",
            Self::Shortcuts => "shortcuts",
            Self::Notifications => "notifications",
            Self::UpdatesPrivacy => "updates-privacy",
            Self::PaneLayout => "pane-layout",
            Self::OpenWith => "open-with",
            Self::DevServers => "dev-servers",
            Self::Agents => "agents",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|section| section.id() == value)
    }

    pub(crate) const fn title(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Appearance => "Appearance",
            Self::Shortcuts => "Shortcuts",
            Self::Notifications => "Notifications",
            Self::UpdatesPrivacy => "Updates & Privacy",
            Self::PaneLayout => "Worklanes & Panes",
            Self::OpenWith => "Open With",
            Self::DevServers => "Dev Servers",
            Self::Agents => "Agents",
        }
    }

    pub(crate) const fn subtitle(self) -> &'static str {
        match self {
            Self::General => "Confirmations, restore, and clipboard",
            Self::Appearance => "Theme, opacity, and terminal colors",
            Self::Shortcuts => "Keyboard shortcuts and conflicts",
            Self::Notifications => "Desktop alerts and notification sound",
            Self::UpdatesPrivacy => "Update channel and crash reporting",
            Self::PaneLayout => "Worklane placement, labels, icons, opacity, and split behavior",
            Self::OpenWith => "Default apps and custom launchers",
            Self::DevServers => "Dev server detection and browsers",
            Self::Agents => "Agent status, teams, and sleep behavior",
        }
    }

    pub(crate) const fn search_keywords(self) -> &'static str {
        match self {
            Self::General => {
                "confirm quit close restore workspace clipboard copy flatten markdown url"
            }
            Self::Appearance => "theme opacity color font terminal ghostty background",
            Self::Shortcuts => "keyboard keybinding hotkey binding shortcut",
            Self::Notifications => "sound alert notify permission desktop",
            Self::UpdatesPrivacy => "update channel beta stable crash error report privacy sentry",
            Self::PaneLayout => "worklane workspace pane split layout opacity label icon scroll",
            Self::OpenWith => "app editor launch file manager vscode cursor",
            Self::DevServers => "server localhost browser port detect ignored hidden",
            Self::Agents => "agent claude codex gemini opencode team sleep status",
        }
    }

    pub(crate) fn matches(self, query: &str) -> bool {
        let query = query.trim().to_lowercase();
        query.is_empty()
            || self.title().to_lowercase().contains(&query)
            || self.subtitle().to_lowercase().contains(&query)
            || self.id().contains(&query)
            || self.search_keywords().contains(&query)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SettingsHistory {
    entries: Vec<SettingsSection>,
    index: usize,
}

impl SettingsHistory {
    pub(crate) fn new(initial: SettingsSection) -> Self {
        Self {
            entries: vec![initial],
            index: 0,
        }
    }

    pub(crate) fn record(&mut self, section: SettingsSection) {
        if self.current() == section {
            return;
        }
        self.entries.truncate(self.index + 1);
        self.entries.push(section);
        self.index = self.entries.len() - 1;
    }

    pub(crate) fn back(&mut self) -> Option<SettingsSection> {
        if self.index == 0 {
            return None;
        }
        self.index -= 1;
        Some(self.current())
    }

    pub(crate) fn forward(&mut self) -> Option<SettingsSection> {
        if self.index + 1 >= self.entries.len() {
            return None;
        }
        self.index += 1;
        Some(self.current())
    }

    pub(crate) fn current(&self) -> SettingsSection {
        self.entries[self.index]
    }

    pub(crate) fn can_back(&self) -> bool {
        self.index > 0
    }

    pub(crate) fn can_forward(&self) -> bool {
        self.index + 1 < self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::{SettingsHistory, SettingsSection};

    #[test]
    fn source_sections_have_exact_identity_order_and_search_vocabulary() {
        assert_eq!(
            SettingsSection::ALL.map(SettingsSection::id),
            [
                "general",
                "appearance",
                "shortcuts",
                "notifications",
                "updates-privacy",
                "pane-layout",
                "open-with",
                "dev-servers",
                "agents",
            ]
        );
        assert_eq!(
            SettingsSection::ALL.map(SettingsSection::title),
            [
                "General",
                "Appearance",
                "Shortcuts",
                "Notifications",
                "Updates & Privacy",
                "Worklanes & Panes",
                "Open With",
                "Dev Servers",
                "Agents",
            ]
        );
        assert_eq!(
            SettingsSection::ALL.map(SettingsSection::subtitle),
            [
                "Confirmations, restore, and clipboard",
                "Theme, opacity, and terminal colors",
                "Keyboard shortcuts and conflicts",
                "Desktop alerts and notification sound",
                "Update channel and crash reporting",
                "Worklane placement, labels, icons, opacity, and split behavior",
                "Default apps and custom launchers",
                "Dev server detection and browsers",
                "Agent status, teams, and sleep behavior",
            ]
        );
        for section in SettingsSection::ALL {
            assert_eq!(SettingsSection::parse(section.id()), Some(section));
            assert!(section.matches(section.title()));
        }
        assert!(SettingsSection::General.matches("clipboard"));
        assert!(SettingsSection::UpdatesPrivacy.matches("crash"));
        assert!(SettingsSection::Agents.matches("opencode"));
        assert!(SettingsSection::OpenWith.matches("custom launchers"));
        assert!(!SettingsSection::Appearance.matches("server"));
        assert_eq!(SettingsSection::parse("unknown"), None);
    }

    #[test]
    fn browser_history_deduplicates_truncates_and_bounds_navigation() {
        let mut history = SettingsHistory::new(SettingsSection::General);
        assert!(!history.can_back());
        assert!(!history.can_forward());
        assert_eq!(history.back(), None);
        history.record(SettingsSection::Appearance);
        history.record(SettingsSection::Shortcuts);
        history.record(SettingsSection::Shortcuts);
        assert!(history.can_back());
        assert_eq!(history.back(), Some(SettingsSection::Appearance));
        assert!(history.can_forward());
        assert_eq!(history.forward(), Some(SettingsSection::Shortcuts));
        assert_eq!(history.back(), Some(SettingsSection::Appearance));
        history.record(SettingsSection::Agents);
        assert!(!history.can_forward());
        assert_eq!(history.forward(), None);
        assert_eq!(history.back(), Some(SettingsSection::Appearance));
        assert_eq!(history.back(), Some(SettingsSection::General));
        assert_eq!(history.back(), None);
    }
}
