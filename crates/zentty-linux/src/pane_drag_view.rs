use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;

use crate::pane_drag_drop::{
    CanvasHit, ColumnInsertionHit, PaneDragPayload, PaneDropInput, PaneDropOutcome,
    SidebarDropTarget, SplitHit, StackGapHit, canvas_hit, resolve,
};

#[derive(Clone, Debug, glib::Boxed)]
#[boxed_type(name = "ZenttyPaneDragTransfer")]
struct PaneDragTransfer(String);

thread_local! {
    // GTK 4.14's DropTarget preload path asserts when one drag crosses several
    // nested targets quickly. Keep only the immutable presentation payload in
    // process-local DnD state; the actual drop still decodes and validates the
    // transferred GValue before any topology mutation.
    static ACTIVE_PANE_DRAG: RefCell<Option<PaneDragPayload>> = const { RefCell::new(None) };
    static DROP_SHIELDS: RefCell<Vec<glib::WeakRef<gtk::Widget>>> = const { RefCell::new(Vec::new()) };
}

fn active_payload() -> Option<PaneDragPayload> {
    ACTIVE_PANE_DRAG.with(|active| active.borrow().clone())
}

fn begin_active_payload(payload: &PaneDragPayload) {
    ACTIVE_PANE_DRAG.with(|active| active.replace(Some(payload.clone())));
    set_drop_shields_enabled(true);
}

fn end_active_payload(pane_id: &str) {
    ACTIVE_PANE_DRAG.with(|active| {
        if active
            .borrow()
            .as_ref()
            .is_some_and(|payload| payload.pane_id == pane_id)
        {
            active.replace(None);
            set_drop_shields_enabled(false);
        }
    });
}

fn register_drop_shield(shield: &gtk::Box) {
    let widget = shield.clone().upcast::<gtk::Widget>();
    widget.set_can_target(active_payload().is_some());
    DROP_SHIELDS.with(|shields| shields.borrow_mut().push(widget.downgrade()));
}

fn set_drop_shields_enabled(enabled: bool) {
    DROP_SHIELDS.with(|shields| {
        shields.borrow_mut().retain(|weak| {
            let Some(widget) = weak.upgrade() else {
                return false;
            };
            widget.set_can_target(enabled);
            true
        });
    });
}

#[derive(Clone)]
pub(crate) struct PaneDragContext {
    pub(crate) window_id: String,
    pub(crate) generation: u64,
    pub(crate) on_drop: Rc<dyn Fn(PaneDropOutcome)>,
    pub(crate) on_cancel: Rc<dyn Fn(String)>,
    pub(crate) source_columns: Rc<BTreeMap<String, String>>,
}

#[derive(Clone, Copy)]
pub(crate) struct CanvasTarget<'a> {
    pub(crate) worklane_id: &'a str,
    pub(crate) column_id: &'a str,
    pub(crate) pane_id: &'a str,
    pub(crate) pane_index: usize,
    pub(crate) column_index: usize,
    pub(crate) column_pane_count: usize,
}

fn payload_from_value(value: &glib::Value) -> Option<PaneDragPayload> {
    value
        .get::<PaneDragTransfer>()
        .ok()
        .and_then(|value| PaneDragPayload::decode(&value.0))
}

fn payload_provider(payload: &PaneDragPayload) -> gtk::gdk::ContentProvider {
    gtk::gdk::ContentProvider::for_value(&PaneDragTransfer(payload.encode()).to_value())
}

pub(crate) fn make_identity_card(payload: &PaneDragPayload, class: &str) -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 2);
    card.add_css_class("pane-drag-card");
    card.add_css_class(class);
    let worklane = gtk::Label::new(Some(&payload.presentation.worklane_title));
    worklane.add_css_class("pane-drag-worklane");
    worklane.set_xalign(0.0);
    worklane.set_ellipsize(gtk::pango::EllipsizeMode::End);
    card.append(&worklane);
    let pane = gtk::Label::new(Some(&payload.presentation.pane_title));
    pane.add_css_class("pane-drag-title");
    pane.set_xalign(0.0);
    pane.set_ellipsize(gtk::pango::EllipsizeMode::End);
    card.append(&pane);
    if !payload.presentation.context.is_empty() {
        let context = gtk::Label::new(Some(&payload.presentation.context));
        context.add_css_class("pane-drag-context");
        context.set_xalign(0.0);
        context.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        card.append(&context);
    }
    if let Some(status) = &payload.presentation.agent_status {
        let status = gtk::Label::new(Some(status));
        status.add_css_class("pane-drag-agent");
        status.set_xalign(0.0);
        status.set_ellipsize(gtk::pango::EllipsizeMode::End);
        card.append(&status);
    }
    card
}

pub(crate) fn sidebar_source(
    select: &gtk::Button,
    row: &gtk::Box,
    payload: PaneDragPayload,
    on_cancel: Rc<dyn Fn(String)>,
) -> gtk::DragSource {
    let motion = gtk::EventControllerMotion::new();
    let motion_pane = payload.pane_id.clone();
    motion.connect_enter(move |_, _, _| {
        eprintln!("zentty-linux: pane-drag-zone pane={motion_pane} source=sidebar pointer=entered");
    });
    // Install the gesture on the actual pointer target. The pane row contains
    // a GtkButton and a menu button; a source installed on their parent loses
    // the gesture sequence to the child button before GTK crosses its drag
    // threshold. Keeping the source on the select button preserves normal
    // click-to-focus while making a held movement a real GTK drag.
    select.add_controller(motion);
    let source = gtk::DragSource::new();
    source.set_actions(gtk::gdk::DragAction::MOVE);
    source.set_propagation_phase(gtk::PropagationPhase::Capture);
    let prepare = payload.clone();
    source.connect_prepare(move |_, _, _| {
        eprintln!("zentty-linux: pane-drag=prepare pane={}", prepare.pane_id);
        Some(payload_provider(&prepare))
    });
    let begin_payload = payload.clone();
    let begin_row = row.clone();
    source.connect_drag_begin(move |source, _| {
        begin_active_payload(&begin_payload);
        let preview = make_identity_card(&begin_payload, "pane-drag-floating");
        let paintable = gtk::WidgetPaintable::new(Some(&preview));
        source.set_icon(Some(&paintable), 20, 18);
        begin_row.add_css_class("pane-dragged");
        eprintln!(
            "zentty-linux: pane-drag=begin window={} worklane={} pane={} visual=full-card",
            begin_payload.source_window_id, begin_payload.source_worklane_id, begin_payload.pane_id
        );
    });
    let end_row = row.clone();
    let end_pane = payload.pane_id.clone();
    source.connect_drag_end(move |_, _, _| {
        end_active_payload(&end_pane);
        end_row.remove_css_class("pane-dragged");
        eprintln!("zentty-linux: pane-drag=end pane={end_pane} visual=cleared");
    });
    let cancel_pane = payload.pane_id;
    source.connect_drag_cancel(move |_, _, reason| {
        eprintln!("zentty-linux: pane-drag=cancel pane={cancel_pane} reason={reason:?}");
        on_cancel(cancel_pane.clone());
        false
    });
    select.set_cursor_from_name(Some("grab"));
    select.update_property(&[gtk::accessible::Property::Description(
        "Draggable pane. Drop on a pane edge, column boundary, or worklane.",
    )]);
    select.add_controller(source.clone());
    source
}

pub(crate) fn terminal_source(
    drag_zone: &gtk::Label,
    payload: &PaneDragPayload,
) -> gtk::DragSource {
    drag_zone.set_can_target(true);
    let source = gtk::DragSource::new();
    source.set_actions(gtk::gdk::DragAction::MOVE);
    source.set_propagation_phase(gtk::PropagationPhase::Capture);
    let prepare = payload.clone();
    source.connect_prepare(move |_, _, _| Some(payload_provider(&prepare)));
    let begin_payload = payload.clone();
    source.connect_drag_begin(move |source, _| {
        begin_active_payload(&begin_payload);
        let preview = make_identity_card(&begin_payload, "pane-drag-floating");
        let paintable = gtk::WidgetPaintable::new(Some(&preview));
        source.set_icon(Some(&paintable), 20, 18);
        eprintln!(
            "zentty-linux: pane-drag=begin window={} worklane={} pane={} source=terminal-strip visual=full-card",
            begin_payload.source_window_id,
            begin_payload.source_worklane_id,
            begin_payload.pane_id
        );
    });
    let end_pane = payload.pane_id.clone();
    source.connect_drag_end(move |_, _, _| {
        end_active_payload(&end_pane);
        eprintln!("zentty-linux: pane-drag=end pane={end_pane} source=terminal-strip");
    });
    drag_zone.add_controller(source.clone());
    source
}

pub(crate) fn install_worklane_target(
    card: &gtk::Box,
    worklane_id: &str,
    context: &PaneDragContext,
) {
    card.set_accessible_role(gtk::AccessibleRole::Group);
    let target = gtk::DropTarget::new(PaneDragTransfer::static_type(), gtk::gdk::DragAction::MOVE);
    let motion_card = card.clone();
    let motion_worklane = worklane_id.to_owned();
    target.connect_motion(move |_, _, y| {
        let Some(payload) = active_payload() else {
            return gtk::gdk::DragAction::empty();
        };
        let pane_index = pane_insertion_index(&motion_card, y);
        show_worklane_slot(&motion_card, &payload, pane_index);
        eprintln!(
            "zentty-linux: pane-drag=preview-slot target-worklane={motion_worklane} pane-index={pane_index} reflow=live"
        );
        gtk::gdk::DragAction::MOVE
    });
    let leave_card = card.clone();
    target.connect_leave(move |_| remove_worklane_slot(&leave_card));
    let drop_card = card.clone();
    let drop_worklane = worklane_id.to_owned();
    let drop_context = context.clone();
    target.connect_drop(move |_, value, _, y| {
        let Some(payload) = payload_from_value(value) else {
            return false;
        };
        let pane_index = pane_insertion_index(&drop_card, y);
        remove_worklane_slot(&drop_card);
        let outcome = resolve(PaneDropInput {
            payload,
            sidebar_target: Some(SidebarDropTarget::Worklane {
                window_id: drop_context.window_id.clone(),
                worklane_id: drop_worklane.clone(),
                pane_index: Some(pane_index),
                generation: drop_context.generation,
            }),
            stack_gap_hit: None,
            split_hit: None,
            column_insertion_hit: None,
            is_duplicate: false,
        });
        let Some(outcome) = outcome else {
            return false;
        };
        (drop_context.on_drop)(outcome);
        true
    });
    card.update_property(&[gtk::accessible::Property::Description(&format!(
        "Pane drop destination: worklane {worklane_id}"
    ))]);
    card.add_controller(target);
}

// GTK requires the enter/motion/leave/drop closures to share weak widget and
// hit-state ownership. Keeping that controller lifecycle in one constructor is
// safer than splitting it across helpers whose captures can silently diverge.
#[allow(clippy::too_many_lines)]
pub(crate) fn wrap_canvas_target(
    frame: &gtk::Overlay,
    target_spec: CanvasTarget<'_>,
    context: &PaneDragContext,
) -> gtk::Overlay {
    let wrapper = gtk::Overlay::new();
    wrapper.set_accessible_role(gtk::AccessibleRole::Group);
    wrapper.set_hexpand(true);
    wrapper.set_vexpand(true);
    wrapper.set_child(Some(frame));
    let drop_shield = gtk::Box::new(gtk::Orientation::Vertical, 0);
    drop_shield.set_hexpand(true);
    drop_shield.set_vexpand(true);
    drop_shield.add_css_class("pane-drop-shield");
    register_drop_shield(&drop_shield);
    wrapper.add_overlay(&drop_shield);
    let target = gtk::DropTarget::new(PaneDragTransfer::static_type(), gtk::gdk::DragAction::MOVE);
    let worklane_id = target_spec.worklane_id.to_owned();
    let column_id = target_spec.column_id.to_owned();
    let pane_id = target_spec.pane_id.to_owned();
    let pane_index = target_spec.pane_index;
    let column_index = target_spec.column_index;
    let pane_count = target_spec.column_pane_count;
    let enter_pane_id = pane_id.clone();
    target.connect_enter(move |_, x, y| {
        let accepted = active_payload().is_some();
        eprintln!(
            "zentty-linux: pane-drag=target-enter pane={enter_pane_id} accepted={accepted} x={x} y={y}"
        );
        if accepted {
            gtk::gdk::DragAction::MOVE
        } else {
            gtk::gdk::DragAction::empty()
        }
    });
    let last_hit = Rc::new(Cell::new(None));
    let motion_hit = Rc::clone(&last_hit);
    let motion_pane_id = pane_id.clone();
    let motion_wrapper = wrapper.downgrade();
    let motion_window_id = context.window_id.clone();
    let motion_worklane_id = worklane_id.clone();
    let motion_column_id = column_id.clone();
    target.connect_motion(move |_, x, y| {
        let Some(motion_wrapper) = motion_wrapper.upgrade() else {
            return gtk::gdk::DragAction::empty();
        };
        let Some(payload) = active_payload() else {
            return gtk::gdk::DragAction::empty();
        };
        let effective_count = pane_count.saturating_sub(usize::from(
            payload.source_window_id == motion_window_id
                && payload.source_worklane_id == motion_worklane_id
                && payload.source_column_id == motion_column_id,
        ));
        let Some(hit) = canvas_hit(
            x,
            y,
            f64::from(motion_wrapper.width()),
            f64::from(motion_wrapper.height()),
            effective_count.max(1),
        ) else {
            remove_canvas_preview(&motion_wrapper);
            motion_hit.set(None);
            return gtk::gdk::DragAction::empty();
        };
        if motion_hit.get() != Some(hit) {
            eprintln!(
                "zentty-linux: pane-drag=preview target-pane={motion_pane_id} hit={hit:?} reflow=live"
            );
            motion_hit.set(Some(hit));
            show_canvas_preview(&motion_wrapper, &payload, hit);
        }
        gtk::gdk::DragAction::MOVE
    });
    let leave_wrapper = wrapper.downgrade();
    let leave_hit = Rc::clone(&last_hit);
    target.connect_leave(move |_| {
        leave_hit.set(None);
        if let Some(leave_wrapper) = leave_wrapper.upgrade() {
            remove_canvas_preview(&leave_wrapper);
        }
    });
    let drop_wrapper = wrapper.downgrade();
    let drop_context = context.clone();
    target.connect_drop(move |_, value, x, y| {
        let Some(drop_wrapper) = drop_wrapper.upgrade() else {
            return false;
        };
        let Some(payload) = payload_from_value(value) else {
            return false;
        };
        let effective_count = pane_count.saturating_sub(usize::from(
            payload.source_window_id == drop_context.window_id
                && payload.source_worklane_id == worklane_id
                && payload.source_column_id == column_id,
        ));
        let Some(canvas_hit) = canvas_hit(
            x,
            y,
            f64::from(drop_wrapper.width()),
            f64::from(drop_wrapper.height()),
            effective_count.max(1),
        ) else {
            return false;
        };
        remove_canvas_preview(&drop_wrapper);
        let (stack_gap_hit, split_hit, column_insertion_hit) = match canvas_hit {
            CanvasHit::StackGap { after } => (
                Some(StackGapHit {
                    window_id: drop_context.window_id.clone(),
                    worklane_id: worklane_id.clone(),
                    column_id: column_id.clone(),
                    pane_index: pane_index + usize::from(after),
                    generation: drop_context.generation,
                }),
                None,
                None,
            ),
            CanvasHit::Split { axis, leading } => (
                None,
                Some(SplitHit {
                    window_id: drop_context.window_id.clone(),
                    worklane_id: worklane_id.clone(),
                    target_pane_id: pane_id.clone(),
                    axis,
                    leading,
                    generation: drop_context.generation,
                }),
                None,
            ),
            CanvasHit::ColumnGap { after } => (
                None,
                None,
                Some(ColumnInsertionHit {
                    window_id: drop_context.window_id.clone(),
                    worklane_id: worklane_id.clone(),
                    column_index: column_index + usize::from(after),
                    generation: drop_context.generation,
                }),
            ),
        };
        let Some(outcome) = resolve(PaneDropInput {
            payload,
            sidebar_target: None,
            stack_gap_hit,
            split_hit,
            column_insertion_hit,
            is_duplicate: false,
        }) else {
            return false;
        };
        eprintln!(
            "zentty-linux: pane-drag=drop pane={} target={canvas_hit:?}",
            outcome.payload().pane_id
        );
        (drop_context.on_drop)(outcome);
        true
    });
    wrapper.update_property(&[gtk::accessible::Property::Description(&format!(
        "Pane drop destination: pane {} in worklane {}",
        target_spec.pane_id, target_spec.worklane_id
    ))]);
    drop_shield.add_controller(target);
    wrapper
}

fn show_canvas_preview(wrapper: &gtk::Overlay, payload: &PaneDragPayload, hit: CanvasHit) {
    remove_canvas_preview(wrapper);
    let preview = make_identity_card(payload, "pane-canvas-drop-preview");
    preview.set_widget_name("zentty-pane-canvas-drop-preview");
    preview.set_can_target(false);
    preview.set_halign(match hit {
        CanvasHit::Split {
            axis: crate::pane_drag_drop::SplitAxis::Horizontal,
            leading: true,
        }
        | CanvasHit::ColumnGap { after: false } => gtk::Align::Start,
        CanvasHit::Split {
            axis: crate::pane_drag_drop::SplitAxis::Horizontal,
            leading: false,
        }
        | CanvasHit::ColumnGap { after: true } => gtk::Align::End,
        _ => gtk::Align::Center,
    });
    preview.set_valign(match hit {
        CanvasHit::StackGap { after: false }
        | CanvasHit::Split {
            axis: crate::pane_drag_drop::SplitAxis::Vertical,
            leading: true,
        } => gtk::Align::Start,
        CanvasHit::StackGap { after: true }
        | CanvasHit::Split {
            axis: crate::pane_drag_drop::SplitAxis::Vertical,
            leading: false,
        } => gtk::Align::End,
        _ => gtk::Align::Center,
    });
    wrapper.add_overlay(&preview);
}

fn remove_canvas_preview(wrapper: &gtk::Overlay) {
    let mut child = wrapper.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        if widget.widget_name() == "zentty-pane-canvas-drop-preview" {
            wrapper.remove_overlay(&widget);
        }
    }
}

fn pane_rows(card: &gtk::Box) -> Vec<gtk::Widget> {
    let mut rows = Vec::new();
    let mut child = card.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        if widget.has_css_class("pane-row") && !widget.has_css_class("pane-drop-slot") {
            rows.push(widget);
        }
    }
    rows
}

fn pane_insertion_index(card: &gtk::Box, y: f64) -> usize {
    let rows = pane_rows(card);
    rows.iter()
        .position(|row| {
            row.compute_bounds(card)
                .is_some_and(|bounds| y < f64::from(bounds.y() + bounds.height() / 2.0))
        })
        .unwrap_or(rows.len())
}

fn show_worklane_slot(card: &gtk::Box, payload: &PaneDragPayload, pane_index: usize) {
    remove_worklane_slot(card);
    let slot = make_identity_card(payload, "pane-drop-slot");
    slot.set_widget_name("zentty-pane-drop-slot");
    slot.set_can_target(false);
    let rows = pane_rows(card);
    if let Some(target) = rows.get(pane_index) {
        card.insert_child_after(&slot, target.prev_sibling().as_ref());
    } else if let Some(last) = rows.last() {
        card.insert_child_after(&slot, Some(last));
    } else {
        card.append(&slot);
    }
}

fn remove_worklane_slot(card: &gtk::Box) {
    let mut child = card.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        if widget.widget_name() == "zentty-pane-drop-slot" {
            card.remove(&widget);
        }
    }
}

pub(crate) fn install_styles() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(
        ".pane-drag-card { min-width: 190px; padding: 7px 9px; border-radius: 8px; background: #29303a; border: 1px solid #657185; color: #f3f5f7; }\n\
         .pane-drag-floating { box-shadow: 0 10px 26px alpha(black, 0.58); border-color: #65a7ff; }\n\
         .pane-drop-slot { margin: 3px 0; background: #24364c; border: 2px solid #65a7ff; box-shadow: inset 0 0 0 1px alpha(#65a7ff, 0.35); }\n\
         .pane-canvas-drop-preview { margin: 8px; background: #24364c; border: 2px solid #65a7ff; box-shadow: 0 8px 20px alpha(black, 0.55); }\n\
         .pane-dragged { opacity: 0.48; }\n\
         .pane-drag-worklane { color: #aab4c2; font-size: 11px; font-weight: 700; }\n\
         .pane-drag-title { color: #ffffff; font-weight: 700; }\n\
         .pane-drag-context { color: #b4bdc9; font-size: 11px; }\n\
         .pane-drag-agent { color: #8fc0ff; font-size: 11px; font-weight: 600; }\n\
         .zentty-pane-drag-zone { min-height: 15px; background: transparent; }\n\
         .zentty-pane-drag-zone:hover { background: alpha(white, 0.055); }",
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
    use super::*;
    use crate::pane_drag_drop::PaneDragPresentation;

    #[test]
    fn pane_payload_uses_a_distinct_boxed_type_so_worklane_string_drops_cannot_collide() {
        assert_ne!(PaneDragTransfer::static_type(), String::static_type());
        let payload = PaneDragPayload {
            source_window_id: "window-a".to_owned(),
            source_worklane_id: "lane-a".to_owned(),
            source_column_id: "column-a".to_owned(),
            pane_id: "pane-a".to_owned(),
            source_generation: 4,
            presentation: PaneDragPresentation {
                worklane_title: "Frontend".to_owned(),
                pane_title: "pnpm dev".to_owned(),
                context: "frontend • main".to_owned(),
                agent_status: None,
            },
        };
        let provider = payload_provider(&payload);
        assert!(
            provider
                .formats()
                .contains_type(PaneDragTransfer::static_type())
        );
        assert!(!provider.formats().contains_type(String::static_type()));
    }
}
