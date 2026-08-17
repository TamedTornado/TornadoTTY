use std::rc::Rc;

use gtk::prelude::*;
use zentty_core::{PaneReference, WorklaneState};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Direction {
    Forward,
    Backward,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SpatialDirection {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) enum Phase {
    #[default]
    Idle,
    Armed {
        generation: u64,
        pending: Direction,
    },
    Peeking {
        original: PaneReference,
        current: PaneReference,
        traversal: Vec<PaneReference>,
    },
}

impl Phase {
    pub(crate) fn is_active(&self) -> bool {
        !matches!(self, Self::Idle)
    }

    pub(crate) fn selected(&self) -> Option<&PaneReference> {
        match self {
            Self::Peeking { current, .. } => Some(current),
            Self::Idle | Self::Armed { .. } => None,
        }
    }
}

pub(crate) fn step(
    traversal: &[PaneReference],
    current: &PaneReference,
    direction: Direction,
) -> Option<PaneReference> {
    if traversal.len() < 2 {
        return None;
    }
    let index = traversal
        .iter()
        .position(|candidate| candidate == current)?;
    let target = match direction {
        Direction::Forward => (index + 1) % traversal.len(),
        Direction::Backward => index.checked_sub(1).unwrap_or(traversal.len() - 1),
    };
    Some(traversal[target].clone())
}

pub(crate) fn spatial_target(
    worklanes: &[WorklaneState],
    current: &PaneReference,
    direction: SpatialDirection,
) -> Option<PaneReference> {
    let worklane_index = worklanes
        .iter()
        .position(|worklane| worklane.id == current.worklane_id)?;
    let worklane = &worklanes[worklane_index];
    let column_index = worklane
        .columns
        .iter()
        .position(|column| column.panes.iter().any(|pane| pane.id == current.pane_id))?;

    match direction {
        SpatialDirection::Left | SpatialDirection::Right => {
            let target_index = if direction == SpatialDirection::Right {
                column_index.checked_add(1)?
            } else {
                column_index.checked_sub(1)?
            };
            let column = worklane.columns.get(target_index)?;
            let pane_id = if column
                .panes
                .iter()
                .any(|pane| pane.id == column.last_focused_pane_id)
            {
                &column.last_focused_pane_id
            } else {
                &column.focused_pane_id
            };
            Some(PaneReference::new(&worklane.id, pane_id))
        }
        SpatialDirection::Up | SpatialDirection::Down => {
            let column = &worklane.columns[column_index];
            let pane_index = column
                .panes
                .iter()
                .position(|pane| pane.id == current.pane_id)?;
            let local_index = if direction == SpatialDirection::Down {
                pane_index.checked_add(1)
            } else {
                pane_index.checked_sub(1)
            };
            if let Some(pane) = local_index.and_then(|index| column.panes.get(index)) {
                return Some(PaneReference::new(&worklane.id, &pane.id));
            }

            let target_worklane_index = if direction == SpatialDirection::Down {
                worklane_index.checked_add(1)?
            } else {
                worklane_index.checked_sub(1)?
            };
            let target = worklanes.get(target_worklane_index)?;
            let focused_column = target
                .columns
                .iter()
                .find(|column| column.id == target.focused_column_id)
                .or_else(|| target.columns.first())?;
            Some(PaneReference::new(
                &target.id,
                &focused_column.focused_pane_id,
            ))
        }
    }
}

pub(crate) struct PanePreview {
    pub(crate) reference: PaneReference,
    pub(crate) worklane_title: String,
    pub(crate) pane_title: String,
    pub(crate) terminal: gtk::Widget,
    pub(crate) project_icon_path: Option<std::path::PathBuf>,
}

pub(crate) struct WorklanePeekView {
    root: gtk::Box,
    content: gtk::Box,
    hud: gtk::Label,
}

impl WorklanePeekView {
    pub(crate) fn new() -> Self {
        install_styles();
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("zentty-peek-shield");
        root.set_hexpand(true);
        root.set_vexpand(true);
        root.set_halign(gtk::Align::Fill);
        root.set_valign(gtk::Align::Fill);
        root.set_visible(false);
        root.connect_map(|_| eprintln!("zentty-linux: worklane-peek=mapped"));

        let content = gtk::Box::new(gtk::Orientation::Horizontal, 18);
        content.add_css_class("zentty-peek-content");
        content.set_halign(gtk::Align::Center);
        content.set_valign(gtk::Align::Center);
        content.set_vexpand(true);
        root.append(&content);

        let hud = gtk::Label::new(None);
        hud.add_css_class("zentty-peek-hud");
        hud.set_halign(gtk::Align::Center);
        hud.set_margin_bottom(30);
        root.append(&hud);

        // Make the backdrop an input target. Pane buttons remain the more
        // specific targets, while clicks elsewhere cannot reach a terminal.
        root.add_controller(gtk::GestureClick::new());
        Self { root, content, hud }
    }

    pub(crate) fn widget(&self) -> &gtk::Widget {
        self.root.upcast_ref()
    }

    pub(crate) fn hide(&self) {
        self.root.set_visible(false);
        clear_box(&self.content);
        self.hud.set_text("");
    }

    pub(crate) fn render(
        &self,
        previews: Vec<PanePreview>,
        selected: &PaneReference,
        on_select: impl Fn(PaneReference) + 'static,
    ) {
        clear_box(&self.content);
        let on_select: Rc<dyn Fn(PaneReference)> = Rc::new(on_select);
        let mut current_worklane = String::new();
        let mut lane: Option<gtk::Box> = None;
        let mut selected_hud = String::new();

        for preview in previews {
            if preview.reference == *selected {
                selected_hud = format!("{}  •  {}", preview.pane_title, preview.worklane_title);
            }
            if preview.reference.worklane_id != current_worklane {
                current_worklane.clone_from(&preview.reference.worklane_id);
                let group = gtk::Box::new(gtk::Orientation::Vertical, 8);
                group.add_css_class("zentty-peek-worklane");
                let heading = gtk::Label::new(Some(&preview.worklane_title));
                heading.add_css_class("zentty-peek-heading");
                heading.set_xalign(0.0);
                group.append(&heading);
                self.content.append(&group);
                lane = Some(group);
            }

            let button = gtk::Button::new();
            button.add_css_class("zentty-peek-pane");
            if preview.reference == *selected {
                button.add_css_class("selected");
            }
            button.set_tooltip_text(Some(&format!(
                "Switch to {} — {}",
                preview.worklane_title, preview.pane_title
            )));

            let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
            let paintable = gtk::WidgetPaintable::new(Some(&preview.terminal));
            let picture = gtk::Picture::for_paintable(&paintable);
            picture.set_size_request(240, 140);
            picture.set_can_shrink(true);
            picture.set_content_fit(gtk::ContentFit::Contain);
            card.append(&picture);
            let title_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            title_row.set_halign(gtk::Align::Center);
            let project_icon = crate::project_icon_view::picture(
                &format!("zentty-peek-project-icon-{}", preview.reference.pane_id),
                18,
            );
            crate::project_icon_view::configure(
                &project_icon,
                preview.project_icon_path.as_deref(),
                &format!("worklane-peek:{}", preview.reference.pane_id),
            );
            title_row.append(&project_icon);
            let title = gtk::Label::new(Some(&preview.pane_title));
            title.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
            title.set_max_width_chars(28);
            title_row.append(&title);
            card.append(&title_row);
            button.set_child(Some(&card));

            let reference = preview.reference;
            let callback = Rc::clone(&on_select);
            button.connect_clicked(move |_| callback(reference.clone()));
            lane.as_ref()
                .expect("a pane preview always follows a worklane")
                .append(&button);
        }
        self.hud.set_text(&selected_hud);
        eprintln!("zentty-linux: worklane-peek-hud value={selected_hud:?}");
        self.root.set_visible(true);
    }
}

fn clear_box(container: &gtk::Box) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}

fn install_styles() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(
        ".zentty-peek-shield { background: alpha(#080b10, 0.84); padding: 36px; }\n\
         .zentty-peek-content { padding: 18px; }\n\
         .zentty-peek-worklane { background: #171b21; border-radius: 12px; padding: 12px; }\n\
         .zentty-peek-heading { color: #d7dce5; font-weight: 700; font-size: 15px; }\n\
         .zentty-peek-hud { background: alpha(#000000, 0.58); color: #ffffff; border-radius: 10px; padding: 8px 14px; font-weight: 600; }\n\
         .zentty-peek-pane { background: #252b34; border: 2px solid transparent; border-radius: 9px; padding: 6px; }\n\
         .zentty-peek-pane:hover { background: #303845; }\n\
         .zentty-peek-pane.selected { border-color: #65a7ff; background: #303845; }",
    );
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{Direction, SpatialDirection, spatial_target, step};
    use zentty_core::{
        ColumnRecipe, PaneRecipe, PaneReference, WindowRecipe, WorklaneRecipe, WorkspaceState,
    };

    const CONTROLLER_SOURCE: &str =
        include_str!("../../../Zentty/UI/WorklanePeek/WorklanePeekController.swift");
    const SELECTION_SOURCE: &str =
        include_str!("../../../Zentty/UI/WorklanePeek/WorklanePeekSelectionState.swift");

    fn state() -> WorkspaceState {
        fn pane(id: &str) -> PaneRecipe {
            PaneRecipe {
                id: id.to_owned(),
                custom_title: None,
                title_seed: None,
                working_directory: None,
                last_activity_title: None,
                last_run_command: None,
            }
        }
        fn column(id: &str, panes: Vec<PaneRecipe>) -> ColumnRecipe {
            ColumnRecipe {
                id: id.to_owned(),
                width: 1.0,
                focused_pane_id: panes.first().map(|pane| pane.id.clone()),
                last_focused_pane_id: panes.first().map(|pane| pane.id.clone()),
                pane_heights: vec![1.0; panes.len()],
                panes,
            }
        }
        fn worklane(id: &str, columns: Vec<ColumnRecipe>) -> WorklaneRecipe {
            WorklaneRecipe {
                id: id.to_owned(),
                title: None,
                next_pane_number: 1,
                focused_column_id: columns.first().map(|column| column.id.clone()),
                columns,
                color: None,
                bookmark_origin_id: None,
            }
        }
        WorkspaceState::from_window_recipe(&WindowRecipe {
            id: "window".to_owned(),
            frame: None,
            worklanes: vec![
                worklane(
                    "one",
                    vec![
                        column("one-a", vec![pane("a"), pane("b")]),
                        column("one-b", vec![pane("c")]),
                    ],
                ),
                worklane("two", vec![column("two-a", vec![pane("d")])]),
            ],
            active_worklane_id: Some("one".to_owned()),
        })
        .expect("fixture is valid")
    }

    #[test]
    fn traversal_wraps_in_source_sidebar_order() {
        let traversal = vec![
            PaneReference::new("one", "a"),
            PaneReference::new("one", "b"),
            PaneReference::new("two", "d"),
        ];
        assert_eq!(
            step(&traversal, &traversal[2], Direction::Forward),
            Some(traversal[0].clone())
        );
        assert_eq!(
            step(&traversal, &traversal[0], Direction::Backward),
            Some(traversal[2].clone())
        );
        assert!(SELECTION_SOURCE.contains("cycles to the first, and vice versa"));
    }

    #[test]
    fn spatial_navigation_uses_columns_splits_and_adjacent_worklanes() {
        let state = state();
        let lanes = state.worklanes();
        assert_eq!(
            spatial_target(
                lanes,
                &PaneReference::new("one", "a"),
                SpatialDirection::Down
            ),
            Some(PaneReference::new("one", "b"))
        );
        assert_eq!(
            spatial_target(
                lanes,
                &PaneReference::new("one", "b"),
                SpatialDirection::Down
            ),
            Some(PaneReference::new("two", "d"))
        );
        assert_eq!(
            spatial_target(
                lanes,
                &PaneReference::new("one", "a"),
                SpatialDirection::Right
            ),
            Some(PaneReference::new("one", "c"))
        );
        assert_eq!(
            spatial_target(
                lanes,
                &PaneReference::new("one", "c"),
                SpatialDirection::Left
            ),
            Some(PaneReference::new("one", "a"))
        );
        assert!(CONTROLLER_SOURCE.contains("Tap-versus-hold disambiguation window"));
        assert!(SELECTION_SOURCE.contains("left/right move between pane columns"));
    }
}
