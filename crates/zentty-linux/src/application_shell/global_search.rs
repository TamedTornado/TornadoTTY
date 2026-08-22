use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gtk::glib;
use gtk::prelude::*;
use zentty_core::{
    GlobalSearchDirection, GlobalSearchEffect, GlobalSearchState, GlobalSearchTarget, PaneReference,
};

use super::{ApplicationShell, find_ghostty_search_overlay, find_search_entry};

impl ApplicationShell {
    pub(super) fn install_global_search_callbacks(shell: &Rc<RefCell<Self>>) {
        let weak = Rc::downgrade(shell);
        let search_changed_handler = shell
            .borrow()
            .global_search_view
            .entry
            .connect_search_changed(move |entry| {
                let Some(shell) = weak.upgrade() else {
                    return;
                };
                shell
                    .borrow_mut()
                    .update_global_find_query(entry.text().as_str());
            });
        shell
            .borrow()
            .global_search_view
            .set_search_changed_handler(search_changed_handler);
        let weak = Rc::downgrade(shell);
        shell
            .borrow()
            .global_search_view
            .connect_focus_changed(move |focused| {
                let Some(shell) = weak.upgrade() else {
                    return;
                };
                if focused {
                    // `focus()` can deliver this notification synchronously while
                    // `toggle_global_find` already owns the shell. That path has
                    // projected the focus hold itself, so only independently
                    // delivered focus changes need work here.
                    let Ok(mut borrowed) = shell.try_borrow_mut() else {
                        return;
                    };
                    borrowed
                        .sidebar_visibility
                        .handle(super::SidebarVisibilityEvent::GlobalSearchFocusEntered);
                    borrowed.apply_sidebar_visibility();
                } else {
                    let Ok(mut borrowed) = shell.try_borrow_mut() else {
                        return;
                    };
                    borrowed
                        .sidebar_visibility
                        .handle(super::SidebarVisibilityEvent::GlobalSearchFocusExited);
                    drop(borrowed);
                    super::ApplicationShell::schedule_sidebar_dismissal(&shell);
                }
            });
        let weak = Rc::downgrade(shell);
        shell
            .borrow()
            .global_search_view
            .entry
            .connect_activate(move |_| {
                if let Some(shell) = weak.upgrade() {
                    shell
                        .borrow_mut()
                        .navigate_global_find(GlobalSearchDirection::Next);
                }
            });
    }

    fn global_search_targets(&self) -> Vec<GlobalSearchTarget> {
        self.pane_references_in_sidebar_order()
            .into_iter()
            .filter(|target| self.pane_runtime.surface(&target.pane_id).is_some())
            .map(|target| GlobalSearchTarget::new(target.worklane_id, target.pane_id))
            .collect()
    }

    pub(super) fn toggle_global_find(&mut self) {
        if self.global_search.state().visible {
            self.close_global_find();
            return;
        }
        self.sidebar_visibility
            .handle(super::SidebarVisibilityEvent::GlobalSearchFocusEntered);
        self.apply_sidebar_visibility();
        let targets = self.global_search_targets();
        if !self.global_search.state().has_remembered_search {
            for target in &targets {
                if let Some(surface) = self.pane_runtime.surface(&target.pane_id) {
                    let _ = surface.perform_binding_action("end_search");
                }
            }
        }
        self.global_search.show(&targets);
        self.render_global_search();
        self.global_search_view.focus(true);
        eprintln!("zentty-linux: global-find shown targets={}", targets.len());
    }

    pub(super) fn close_global_find(&mut self) {
        let effects = self.global_search.end();
        self.apply_global_search_effects(effects);
        self.render_global_search();
        self.sidebar_visibility
            .handle(super::SidebarVisibilityEvent::GlobalSearchFocusExited);
        let weak = self.self_handle.borrow().clone();
        glib::idle_add_local_once(move || {
            if let Some(shell) = weak.upgrade() {
                Self::schedule_sidebar_dismissal(&shell);
            }
        });
        self.focus_selected_surface();
    }

    pub(super) fn update_global_find_query(&mut self, needle: &str) {
        if self.global_search.state().needle == needle {
            return;
        }
        self.global_search_generation = self.global_search_generation.wrapping_add(1);
        let targets = self.global_search_targets();
        let effects = self.global_search.update_query(needle, &targets);
        self.apply_global_search_effects(effects);
        self.render_global_search();
        if self.global_search.state().visible {
            self.global_search_view.focus(false);
        }
        if self.global_search.has_pending_query() {
            let generation = self.global_search_generation;
            let weak = self.self_handle.borrow().clone();
            glib::timeout_add_local_once(Duration::from_millis(150), move || {
                let Some(shell) = weak.upgrade() else {
                    return;
                };
                let mut shell = shell.borrow_mut();
                if shell.global_search_generation != generation {
                    return;
                }
                let effects = shell.global_search.dispatch_pending_query();
                shell.apply_global_search_effects(effects);
                shell.render_global_search();
                shell.global_search_view.focus(false);
            });
        }
    }

    pub(super) fn navigate_global_find(&mut self, direction: GlobalSearchDirection) {
        self.global_search_generation = self.global_search_generation.wrapping_add(1);
        let live = self
            .global_search_targets()
            .into_iter()
            .map(|target| target.pane_id)
            .collect::<Vec<_>>();
        let reconciliation = self
            .global_search
            .reconcile_live_panes(live.iter().map(String::as_str));
        self.apply_global_search_effects(reconciliation);
        let current = self
            .current_pane_reference()
            .map(|target| GlobalSearchTarget::new(target.worklane_id, target.pane_id));
        let effects = match direction {
            GlobalSearchDirection::Next => self.global_search.find_next(current.as_ref()),
            GlobalSearchDirection::Previous => self.global_search.find_previous(current.as_ref()),
        };
        self.apply_global_search_effects(effects);
        self.render_global_search();
        let weak = self.self_handle.borrow().clone();
        glib::idle_add_local_once(move || {
            let Some(shell) = weak.upgrade() else {
                return;
            };
            let shell = shell.borrow();
            if shell.global_search.state().visible && !shell.command_palette.is_visible() {
                shell.global_search_view.focus(false);
            }
        });
    }

    pub(super) fn handle_global_search_state(
        &mut self,
        pane_id: &str,
        total: Option<usize>,
        selected: Option<usize>,
    ) {
        let effects = total.map_or_else(Vec::new, |total| {
            self.global_search.handle_total(pane_id, total)
        });
        if total.is_some() {
            self.global_search.handle_selected(pane_id, selected);
        }
        self.apply_global_search_effects(effects);
        self.render_global_search();
    }

    pub(super) fn render_global_search(&self) {
        let state: GlobalSearchState = self.global_search.state().clone();
        self.global_search_view.render(&state);
        eprintln!(
            "zentty-linux: global-find-state visible={} remembered={} selected={:?} total={}",
            state.visible, state.has_remembered_search, state.selected, state.total
        );
    }

    pub(super) fn apply_global_search_effects(&mut self, effects: Vec<GlobalSearchEffect>) {
        for effect in effects {
            match effect {
                GlobalSearchEffect::Start { target, needle } => {
                    let Some(surface) = self.pane_runtime.surface(&target.pane_id) else {
                        continue;
                    };
                    let overlay = find_ghostty_search_overlay(surface.widget());
                    let entry = overlay.as_ref().and_then(find_search_entry);
                    // `start_search` deliberately replays the GTK entry's
                    // remembered pane-local text. Clear that inactive widget
                    // before installing the coordinator's needle.
                    if let Some(entry) = &entry {
                        entry.set_text("");
                    }
                    if let Err(error) = surface.perform_binding_action("start_search") {
                        eprintln!(
                            "zentty-linux: global-find pane={} error={error}",
                            target.pane_id
                        );
                        continue;
                    }
                    if let Some(overlay) = overlay
                        && let Some(entry) = entry
                    {
                        entry.set_text(&needle);
                        // `active` is lifecycle-bearing on Wayland. Preserve
                        // the real search and hide only pane-local presentation.
                        overlay.set_opacity(0.0);
                        overlay.set_can_target(false);
                    }
                }
                GlobalSearchEffect::End { pane_id } => {
                    if let Some(surface) = self.pane_runtime.surface(&pane_id) {
                        if let Some(overlay) = find_ghostty_search_overlay(surface.widget()) {
                            overlay.set_opacity(1.0);
                            overlay.set_can_target(true);
                        }
                        let _ = surface.perform_binding_action("end_search");
                    }
                }
                GlobalSearchEffect::ResetSelection { pane_id } => {
                    if let Some(surface) = self.pane_runtime.surface(&pane_id)
                        && let Some(overlay) = find_ghostty_search_overlay(surface.widget())
                        && let Some(entry) = find_search_entry(&overlay)
                    {
                        entry.set_text(&self.global_search.state().needle);
                    }
                }
                GlobalSearchEffect::Navigate {
                    target,
                    direction,
                    selected_index,
                } => {
                    let reference = PaneReference::new(&target.worklane_id, &target.pane_id);
                    self.select_pane_reference(&reference, false);
                    if let Some(surface) = self.pane_runtime.surface(&target.pane_id) {
                        let binding = match direction {
                            GlobalSearchDirection::Next => "navigate_search:next",
                            GlobalSearchDirection::Previous => "navigate_search:previous",
                        };
                        let _ = surface.perform_binding_action(binding);
                    }
                    eprintln!(
                        "zentty-linux: global-find navigate pane={} selected={selected_index}",
                        target.pane_id
                    );
                }
            }
        }
    }
}
