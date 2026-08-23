use std::cell::Cell;

use gtk::prelude::*;

use crate::sidebar_visibility::Mode;

const STANDARD_DURATION_MS: u32 = 240;

/// Native GTK projection of Zentty's existing sidebar visibility state.
///
/// The visibility state machine remains in `sidebar_visibility`. This revealer
/// only interpolates the sidebar itself. Content reservation is projected
/// synchronously by the shell so a transition can never expose a blank gap.
/// Calling `apply` again reverses GTK's in-flight transition instead of
/// starting a second animation coordinator.
pub(crate) struct SidebarMotion {
    sidebar_revealer: gtk::Revealer,
    initialized: Cell<bool>,
}

impl SidebarMotion {
    pub(crate) fn new(sidebar: &gtk::ScrolledWindow) -> Self {
        let sidebar_revealer = revealer(gtk::RevealerTransitionType::SlideRight);
        sidebar_revealer.set_halign(gtk::Align::Start);
        sidebar_revealer.set_valign(gtk::Align::Fill);
        sidebar_revealer.set_child(Some(sidebar));

        sidebar_revealer.connect_child_revealed_notify(move |revealer| {
            let reveal = revealer.reveals_child();
            if revealer.is_child_revealed() == reveal {
                eprintln!(
                    "zentty-linux: sidebar-motion-settled reveal={}",
                    u8::from(reveal)
                );
            }
        });

        Self {
            sidebar_revealer,
            initialized: Cell::new(false),
        }
    }

    pub(crate) fn sidebar_widget(&self) -> &gtk::Revealer {
        &self.sidebar_revealer
    }

    pub(crate) fn apply(&self, mode: Mode) {
        let (reveal_sidebar, reserve_width) = targets(mode);
        let initial_projection = !self.initialized.replace(true);
        let animations_enabled = animations_enabled();
        let duration = duration(initial_projection, animations_enabled);
        self.sidebar_revealer.set_transition_duration(duration);
        self.sidebar_revealer.set_reveal_child(reveal_sidebar);
        eprintln!(
            "zentty-linux: sidebar-motion mode={} reveal={} reserve={} duration-ms={} animations-enabled={}",
            mode_name(mode),
            u8::from(reveal_sidebar),
            u8::from(reserve_width),
            duration,
            animations_enabled
        );
    }
}

fn revealer(transition: gtk::RevealerTransitionType) -> gtk::Revealer {
    let revealer = gtk::Revealer::new();
    revealer.set_transition_type(transition);
    revealer.set_transition_duration(STANDARD_DURATION_MS);
    revealer
}

fn targets(mode: Mode) -> (bool, bool) {
    match mode {
        Mode::PinnedOpen => (true, true),
        Mode::Hidden => (false, false),
        Mode::HoverPeek => (true, false),
    }
}

fn duration(initial_projection: bool, animations_enabled: bool) -> u32 {
    if initial_projection || !animations_enabled {
        0
    } else {
        STANDARD_DURATION_MS
    }
}

fn mode_name(mode: Mode) -> &'static str {
    match mode {
        Mode::PinnedOpen => "pinned-open",
        Mode::Hidden => "hidden",
        Mode::HoverPeek => "hover-peek",
    }
}

fn animations_enabled() -> bool {
    gtk::Settings::default()
        .is_some_and(|settings| settings.property::<bool>("gtk-enable-animations"))
}

#[cfg(test)]
mod tests {
    use super::{STANDARD_DURATION_MS, duration, targets};
    use crate::sidebar_visibility::Mode;

    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../Zentty/UI/Sidebar/SidebarVisibilityController.swift"
    ));

    #[test]
    fn projects_source_motion_fractions_without_owning_visibility_state() {
        assert_eq!(targets(Mode::PinnedOpen), (true, true));
        assert_eq!(targets(Mode::Hidden), (false, false));
        assert_eq!(targets(Mode::HoverPeek), (true, false));
        assert!(SOURCE.contains(
            "static let pinnedOpen = SidebarMotionState(revealFraction: 1, reservedFraction: 1)"
        ));
        assert!(SOURCE.contains(
            "static let hidden = SidebarMotionState(revealFraction: 0, reservedFraction: 0)"
        ));
        assert!(SOURCE.contains(
            "static let hoverPeek = SidebarMotionState(revealFraction: 1, reservedFraction: 0)"
        ));
    }

    #[test]
    fn native_transition_uses_the_source_standard_duration() {
        assert_eq!(STANDARD_DURATION_MS, 240);
        assert_eq!(duration(false, true), STANDARD_DURATION_MS);
        assert_eq!(duration(true, true), 0);
        assert_eq!(duration(false, false), 0);
        assert!(SOURCE.contains("static let standardDuration: TimeInterval = 0.24"));
    }
}
