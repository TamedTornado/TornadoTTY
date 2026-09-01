use std::cell::Cell;

use zentty_linux::pane_divider_model::{
    DividerAxis, DividerKey, KEYBOARD_RESIZE_STEP, PaneDivider, adjusted_vertical_margin,
};

fn column(after: &str) -> PaneDivider {
    PaneDivider::Column {
        after_column_id: after.to_owned(),
    }
}

fn pane(column: &str, after: &str) -> PaneDivider {
    PaneDivider::Pane {
        column_id: column.to_owned(),
        after_pane_id: after.to_owned(),
    }
}

#[test]
fn column_boundaries_use_horizontal_motion_and_only_horizontal_keys() {
    let divider = column("column-left");
    assert_eq!(divider.axis(), DividerAxis::Horizontal);
    assert_eq!(divider.pointer_offset(37.0, 91.0), 37.0);
    assert_eq!(
        divider.keyboard_request(DividerKey::Left).unwrap().delta,
        -KEYBOARD_RESIZE_STEP
    );
    assert_eq!(
        divider.keyboard_request(DividerKey::Right).unwrap().delta,
        KEYBOARD_RESIZE_STEP
    );
    assert!(divider.keyboard_request(DividerKey::Up).is_none());
    assert!(divider.keyboard_request(DividerKey::Down).is_none());
}

#[test]
fn pane_boundaries_use_vertical_motion_and_only_vertical_keys() {
    let divider = pane("column-left", "pane-top");
    assert_eq!(divider.axis(), DividerAxis::Vertical);
    assert_eq!(divider.pointer_offset(37.0, 91.0), 91.0);
    assert_eq!(
        divider.keyboard_request(DividerKey::Up).unwrap().delta,
        -KEYBOARD_RESIZE_STEP
    );
    assert_eq!(
        divider.keyboard_request(DividerKey::Down).unwrap().delta,
        KEYBOARD_RESIZE_STEP
    );
    assert!(divider.keyboard_request(DividerKey::Left).is_none());
    assert!(divider.keyboard_request(DividerKey::Right).is_none());
}

#[test]
fn callback_receives_typed_unswapped_target_and_signed_delta() {
    let divider = pane("column::after-pane", "pane::after-column");
    let request = divider.resize_request(-23.5).unwrap();
    let calls = Cell::new(0);
    let applied = request.apply(|payload| {
        calls.set(calls.get() + 1);
        assert_eq!(payload.axis, DividerAxis::Vertical);
        assert_eq!(payload.delta, -23.5);
        assert_eq!(payload.target, divider);
        assert!(matches!(
            &payload.target,
            PaneDivider::Pane {
                column_id,
                after_pane_id
            } if column_id == "column::after-pane" && after_pane_id == "pane::after-column"
        ));
        -7.0
    });
    assert_eq!(applied, -7.0);
    assert_eq!(calls.get(), 1);
}

#[test]
fn zero_motion_suppresses_the_resize_callback() {
    let divider = column("column-left");
    assert!(divider.resize_request(0.0).is_none());
    assert!(divider.resize_request(f64::EPSILON).is_none());
    assert!(divider.resize_request(-f64::EPSILON).is_none());
}

#[test]
fn visible_vertical_boundary_is_bounded_at_both_integer_extremes() {
    assert_eq!(adjusted_vertical_margin(8, -9.0), 0);
    assert_eq!(adjusted_vertical_margin(8, 0.0), 8);
    assert_eq!(adjusted_vertical_margin(8, 2.6), 11);
    assert_eq!(adjusted_vertical_margin(i32::MAX - 1, 20.0), i32::MAX);
    assert_eq!(adjusted_vertical_margin(8, f64::MAX), i32::MAX);
    assert_eq!(adjusted_vertical_margin(8, -f64::MAX), 0);
}

#[test]
fn presentation_strings_do_not_replace_the_typed_topology_identity() {
    let divider = pane("column-after-pane-x", "pane-after-column-y");
    assert_eq!(
        divider,
        PaneDivider::Pane {
            column_id: "column-after-pane-x".to_owned(),
            after_pane_id: "pane-after-column-y".to_owned(),
        }
    );
    assert!(divider.widget_name().contains("column-after-pane-x"));
    assert!(divider.accessible_label().contains("pane-after-column-y"));
}
