//! Keep materialized worklane widgets parented during ordinary navigation.
//! Only real topology/geometry changes or explicit Peek reparenting discard
//! layouts. Hidden widgets retain their GTK/Ghostty realization and PTYs.
use gtk::prelude::*;
use std::collections::BTreeMap;
use zentty_core::{WorklaneState, WorkspaceState};

#[derive(PartialEq)]
struct LayoutKey {
    columns: Vec<(String, u64, Vec<String>, Vec<u64>)>,
}

impl LayoutKey {
    fn new(lane: &WorklaneState) -> Self {
        Self {
            columns: lane
                .columns
                .iter()
                .map(|column| {
                    (
                        column.id.clone(),
                        column.width.to_bits(),
                        column.panes.iter().map(|pane| pane.id.clone()).collect(),
                        column
                            .pane_heights
                            .iter()
                            .map(|height| height.to_bits())
                            .collect(),
                    )
                })
                .collect(),
        }
    }
}

struct MountedLayout {
    key: LayoutKey,
    columns: BTreeMap<String, gtk::Overlay>,
}

#[derive(Default)]
pub(super) struct MountedLayouts {
    lanes: BTreeMap<String, MountedLayout>,
}

fn detach(container: &gtk::Box, layout: &MountedLayout) {
    for column in layout.columns.values() {
        if column.parent().as_ref() == Some(container.upcast_ref()) {
            container.remove(column);
        }
    }
}

impl MountedLayouts {
    pub(super) fn clear(&mut self) {
        self.lanes.clear();
    }

    pub(super) fn remove_inactive(&mut self, container: &gtk::Box, active: &str) {
        self.lanes.retain(|id, layout| {
            if id == active {
                return true;
            }
            detach(container, layout);
            false
        });
    }

    pub(super) fn reconcile(&mut self, container: &gtk::Box, state: &WorkspaceState) {
        self.lanes.retain(|id, layout| {
            let valid = state
                .worklanes()
                .iter()
                .find(|lane| &lane.id == id)
                .is_some_and(|lane| layout.key == LayoutKey::new(lane));
            if !valid {
                detach(container, layout);
                return false;
            }
            for column in layout.columns.values() {
                column.set_visible(id == state.active_worklane_id());
            }
            true
        });
    }

    pub(super) fn columns(&self, id: &str) -> Option<&BTreeMap<String, gtk::Overlay>> {
        self.lanes.get(id).map(|layout| &layout.columns)
    }

    pub(super) fn insert(&mut self, lane: &WorklaneState, columns: BTreeMap<String, gtk::Overlay>) {
        self.lanes.insert(
            lane.id.clone(),
            MountedLayout {
                key: LayoutKey::new(lane),
                columns,
            },
        );
    }
}
