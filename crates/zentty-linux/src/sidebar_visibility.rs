#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum Mode {
    #[default]
    PinnedOpen,
    Hidden,
    HoverPeek,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Event {
    TogglePressed,
    HoverRailEntered,
    HoverRailExited,
    SidebarEntered,
    SidebarExited,
    GlobalSearchFocusEntered,
    GlobalSearchFocusExited,
    DismissTimerElapsed,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct State {
    mode: Mode,
    pointer_in_rail: bool,
    pointer_in_sidebar: bool,
    global_search_focused: bool,
}

impl State {
    pub(crate) fn from_persisted(visibility: zentty_core::SidebarVisibilityMode) -> Self {
        Self {
            mode: match visibility {
                zentty_core::SidebarVisibilityMode::PinnedOpen => Mode::PinnedOpen,
                zentty_core::SidebarVisibilityMode::Hidden => Mode::Hidden,
            },
            ..Self::default()
        }
    }

    pub(crate) fn mode(self) -> Mode {
        self.mode
    }

    pub(crate) fn persisted_mode(self) -> zentty_core::SidebarVisibilityMode {
        match self.mode {
            Mode::PinnedOpen => zentty_core::SidebarVisibilityMode::PinnedOpen,
            Mode::Hidden | Mode::HoverPeek => zentty_core::SidebarVisibilityMode::Hidden,
        }
    }

    pub(crate) fn should_schedule_dismissal(self) -> bool {
        self.mode == Mode::HoverPeek
            && !self.pointer_in_rail
            && !self.pointer_in_sidebar
            && !self.global_search_focused
    }

    pub(crate) fn handle(&mut self, event: Event) -> bool {
        let previous = self.mode;
        match event {
            Event::TogglePressed => {
                self.mode = if self.mode == Mode::PinnedOpen {
                    Mode::Hidden
                } else {
                    Mode::PinnedOpen
                };
                self.pointer_in_rail = false;
                self.pointer_in_sidebar = false;
                self.global_search_focused = false;
            }
            Event::HoverRailEntered => {
                self.pointer_in_rail = true;
                if self.mode == Mode::Hidden {
                    self.mode = Mode::HoverPeek;
                }
            }
            Event::HoverRailExited => self.pointer_in_rail = false,
            Event::SidebarEntered => self.pointer_in_sidebar = true,
            Event::SidebarExited => self.pointer_in_sidebar = false,
            Event::GlobalSearchFocusEntered => {
                self.global_search_focused = true;
                if self.mode == Mode::Hidden {
                    self.mode = Mode::HoverPeek;
                }
            }
            Event::GlobalSearchFocusExited => self.global_search_focused = false,
            Event::DismissTimerElapsed if self.should_schedule_dismissal() => {
                self.mode = Mode::Hidden;
                self.pointer_in_rail = false;
                self.pointer_in_sidebar = false;
                self.global_search_focused = false;
            }
            Event::DismissTimerElapsed => {}
        }
        self.mode != previous
    }
}

#[cfg(test)]
mod tests {
    use super::{Event, Mode, State};
    use zentty_core::SidebarVisibilityMode;

    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../Zentty/UI/Sidebar/SidebarVisibilityController.swift"
    ));

    #[test]
    fn toggle_and_hover_peek_match_the_source_state_machine() {
        let mut state = State::default();
        assert!(state.handle(Event::TogglePressed));
        assert_eq!(state.mode(), Mode::Hidden);
        assert!(state.handle(Event::HoverRailEntered));
        assert_eq!(state.mode(), Mode::HoverPeek);
        assert!(!state.handle(Event::HoverRailExited));
        assert!(state.should_schedule_dismissal());
        assert!(state.handle(Event::DismissTimerElapsed));
        assert_eq!(state.mode(), Mode::Hidden);
        assert!(SOURCE.contains("case pinnedOpen"));
        assert!(SOURCE.contains("case hidden"));
        assert!(SOURCE.contains("case hoverPeek"));
    }

    #[test]
    fn entering_the_sidebar_cancels_hover_dismissal_and_toggle_pins_it() {
        let mut state = State::default();
        state.handle(Event::TogglePressed);
        state.handle(Event::HoverRailEntered);
        state.handle(Event::HoverRailExited);
        state.handle(Event::SidebarEntered);
        assert!(!state.should_schedule_dismissal());
        assert!(!state.handle(Event::DismissTimerElapsed));
        assert_eq!(state.mode(), Mode::HoverPeek);
        assert!(state.handle(Event::TogglePressed));
        assert_eq!(state.mode(), Mode::PinnedOpen);
    }

    #[test]
    fn persisted_hidden_and_global_search_focus_hold_only_a_transient_peek() {
        let mut state = State::from_persisted(SidebarVisibilityMode::Hidden);
        assert_eq!(state.mode(), Mode::Hidden);
        assert_eq!(state.persisted_mode(), SidebarVisibilityMode::Hidden);
        assert!(state.handle(Event::GlobalSearchFocusEntered));
        assert_eq!(state.mode(), Mode::HoverPeek);
        assert_eq!(state.persisted_mode(), SidebarVisibilityMode::Hidden);
        assert!(!state.should_schedule_dismissal());
        assert!(!state.handle(Event::DismissTimerElapsed));
        assert!(!state.handle(Event::GlobalSearchFocusExited));
        assert!(state.should_schedule_dismissal());
        assert!(state.handle(Event::DismissTimerElapsed));
        assert_eq!(state.mode(), Mode::Hidden);
        assert!(SOURCE.contains("case globalSearchFocusEntered"));
        assert!(SOURCE.contains("case globalSearchFocusExited"));
    }
}
