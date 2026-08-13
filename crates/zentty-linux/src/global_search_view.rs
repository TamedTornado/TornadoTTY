use std::cell::RefCell;
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;
use zentty_core::GlobalSearchState;

pub(crate) const ROW_NAME: &str = "zentty-global-search-row";
const GLOBAL_FIND: &str = "Global Find";
const PLACEHOLDER: &str = "Search across panes";
const FIND_PREVIOUS: &str = "Find Previous";
const FIND_NEXT: &str = "Find Next";
const CLEAR_GLOBAL_FIND: &str = "Clear Global Find";
const GLOBAL_FIND_ACTION: &str = "workspace.global-find";
const CLEAR_GLOBAL_FIND_ACTION: &str = "workspace.clear-global-find";
const FIND_PREVIOUS_ACTION: &str = "workspace.global-find-previous";
const FIND_NEXT_ACTION: &str = "workspace.global-find-next";

#[derive(Clone)]
pub(crate) struct GlobalSearchView {
    pub(crate) root: gtk::Box,
    pub(crate) entry: gtk::SearchEntry,
    count: gtk::Label,
    previous: gtk::Button,
    next: gtk::Button,
    clear: gtk::Button,
    search_changed_handler: Rc<RefCell<Option<glib::SignalHandlerId>>>,
}

impl GlobalSearchView {
    pub(crate) fn attach(sidebar: &gtk::Box) -> Self {
        let header = sidebar
            .first_child()
            .and_then(|widget| widget.downcast::<gtk::Box>().ok())
            .expect("sidebar header must exist before Global Find is attached");

        let toggle = icon_button("edit-find-symbolic", GLOBAL_FIND);
        toggle.set_action_name(Some(GLOBAL_FIND_ACTION));
        header.append(&toggle);

        let root = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        root.set_widget_name(ROW_NAME);
        root.add_css_class("sidebar-global-search");
        root.set_visible(false);

        let entry = gtk::SearchEntry::new();
        entry.set_placeholder_text(Some(PLACEHOLDER));
        entry.set_hexpand(true);
        entry.update_property(&[gtk::accessible::Property::Label(GLOBAL_FIND)]);
        let entry_focus = gtk::EventControllerFocus::new();
        entry_focus.connect_enter(|_| {
            eprintln!("zentty-linux: global-find focus=entry");
        });
        entry.add_controller(entry_focus);
        let count = gtk::Label::new(None);
        count.add_css_class("sidebar-global-search-count");
        let clear = icon_button("edit-clear-symbolic", CLEAR_GLOBAL_FIND);
        let previous = icon_button("go-up-symbolic", FIND_PREVIOUS);
        let next = icon_button("go-down-symbolic", FIND_NEXT);
        clear.set_action_name(Some(CLEAR_GLOBAL_FIND_ACTION));
        previous.set_action_name(Some(FIND_PREVIOUS_ACTION));
        next.set_action_name(Some(FIND_NEXT_ACTION));
        root.append(&entry);
        root.append(&count);
        root.append(&clear);
        root.append(&previous);
        root.append(&next);
        sidebar.insert_child_after(&root, Some(&header));

        Self {
            root,
            entry,
            count,
            previous,
            next,
            clear,
            search_changed_handler: Rc::new(RefCell::new(None)),
        }
    }

    pub(crate) fn set_search_changed_handler(&self, handler: glib::SignalHandlerId) {
        *self.search_changed_handler.borrow_mut() = Some(handler);
    }

    pub(crate) fn render(&self, state: &GlobalSearchState) {
        let presentation = presentation(state);
        self.root.set_visible(state.visible);
        if self.entry.text().as_str() != state.needle {
            let handler = self.search_changed_handler.borrow();
            if let Some(handler) = handler.as_ref() {
                self.entry.block_signal(handler);
            }
            self.entry.set_text(&state.needle);
            if let Some(handler) = handler.as_ref() {
                self.entry.unblock_signal(handler);
            }
        }
        self.count.set_visible(presentation.has_query);
        self.clear.set_visible(presentation.has_query);
        self.count.set_label(&presentation.count);
        self.previous
            .set_sensitive(presentation.navigation_sensitive);
        self.next.set_sensitive(presentation.navigation_sensitive);
    }

    pub(crate) fn focus(&self, select_all: bool) {
        self.entry.grab_focus();
        if select_all {
            self.entry.select_region(0, -1);
        }
    }
}

struct GlobalSearchPresentation {
    count: String,
    has_query: bool,
    navigation_sensitive: bool,
}

fn presentation(state: &GlobalSearchState) -> GlobalSearchPresentation {
    GlobalSearchPresentation {
        count: state.selected.map_or_else(
            || format!("-/{}", state.total),
            |selected| format!("{}/{}", selected + 1, state.total),
        ),
        has_query: !state.needle.is_empty(),
        navigation_sensitive: state.total > 0,
    }
}

fn icon_button(icon: &str, label: &'static str) -> gtk::Button {
    let button = gtk::Button::from_icon_name(icon);
    button.add_css_class("sidebar-global-search-button");
    button.set_tooltip_text(Some(label));
    button.update_property(&[gtk::accessible::Property::Label(label)]);
    button
}

#[cfg(test)]
mod tests {
    use super::{
        CLEAR_GLOBAL_FIND, CLEAR_GLOBAL_FIND_ACTION, FIND_NEXT, FIND_NEXT_ACTION, FIND_PREVIOUS,
        FIND_PREVIOUS_ACTION, GLOBAL_FIND, GLOBAL_FIND_ACTION, PLACEHOLDER, presentation,
    };
    use zentty_core::GlobalSearchState;

    const SOURCE_ROW: &str =
        include_str!("../../../Zentty/UI/Sidebar/SidebarGlobalSearchRowView.swift");
    const SOURCE_BUTTON: &str =
        include_str!("../../../Zentty/UI/Sidebar/SidebarGlobalSearchButton.swift");

    #[test]
    fn aggregate_count_is_one_based_only_after_selection() {
        let mut state = GlobalSearchState {
            total: 9,
            ..GlobalSearchState::default()
        };
        assert_eq!(presentation(&state).count, "-/9");
        state.selected = Some(6);
        assert_eq!(presentation(&state).count, "7/9");
    }

    #[test]
    fn query_visibility_navigation_sensitivity_and_actions_are_exact() {
        let mut state = GlobalSearchState::default();
        let empty = presentation(&state);
        assert!(!empty.has_query);
        assert!(!empty.navigation_sensitive);

        state.needle = "needle".to_owned();
        let zero = presentation(&state);
        assert!(zero.has_query);
        assert!(!zero.navigation_sensitive);
        state.total = 1;
        assert!(presentation(&state).navigation_sensitive);

        assert_eq!(GLOBAL_FIND_ACTION, "workspace.global-find");
        assert_eq!(CLEAR_GLOBAL_FIND_ACTION, "workspace.clear-global-find");
        assert_eq!(FIND_PREVIOUS_ACTION, "workspace.global-find-previous");
        assert_eq!(FIND_NEXT_ACTION, "workspace.global-find-next");
    }

    #[test]
    fn sidebar_vocabulary_and_accessibility_are_source_exact() {
        for label in [PLACEHOLDER, FIND_PREVIOUS, FIND_NEXT, CLEAR_GLOBAL_FIND] {
            assert!(SOURCE_ROW.contains(label), "source row is missing {label}");
        }
        assert!(SOURCE_BUTTON.contains(GLOBAL_FIND));
    }
}
