use zentty_core::{
    GlobalSearchCoordinator, GlobalSearchDirection, GlobalSearchEffect, GlobalSearchState,
    GlobalSearchTarget,
};

fn target(worklane: &str, pane: &str) -> GlobalSearchTarget {
    GlobalSearchTarget::new(worklane, pane)
}

#[test]
fn query_fans_out_in_frozen_order_and_aggregates_global_ordinals() {
    let targets = [target("lane-1", "pane-1"), target("lane-2", "pane-2")];
    let mut search = GlobalSearchCoordinator::default();
    search.show(&targets);
    assert_eq!(
        search.update_query("build", &targets),
        targets.map(|target| {
            GlobalSearchEffect::Start {
                target,
                needle: "build".to_owned(),
            }
        })
    );
    assert!(search.handle_total("pane-1", 2).is_empty());
    assert!(search.handle_total("pane-2", 1).is_empty());
    assert_eq!(search.state().total, 3);
    assert_eq!(search.state().selected, None);

    search.handle_selected("pane-2", Some(0));
    assert_eq!(search.state().selected, Some(2));
}

#[test]
fn short_query_debounces_but_navigation_flushes_and_waits_for_every_total() {
    let targets = [target("lane-1", "pane-1"), target("lane-2", "pane-2")];
    let mut search = GlobalSearchCoordinator::default();
    search.show(&targets);
    assert!(search.update_query("ab", &targets).is_empty());
    assert!(search.has_pending_query());

    assert_eq!(
        search.find_next(None),
        targets.clone().map(|target| {
            GlobalSearchEffect::Start {
                target,
                needle: "ab".to_owned(),
            }
        })
    );
    assert!(search.handle_total("pane-1", 0).is_empty());
    assert_eq!(
        search.handle_total("pane-2", 1),
        [GlobalSearchEffect::Navigate {
            target: targets[1].clone(),
            direction: GlobalSearchDirection::Next,
            selected_index: 0,
        }]
    );
}

#[test]
fn next_and_previous_wrap_across_nonempty_panes_in_source_order() {
    let targets = [
        target("lane-1", "pane-1"),
        target("lane-1", "pane-empty"),
        target("lane-2", "pane-2"),
    ];
    let mut search = GlobalSearchCoordinator::default();
    search.show(&targets);
    let _ = search.update_query("needle", &targets);
    let _ = search.handle_total("pane-1", 2);
    let _ = search.handle_total("pane-empty", 0);
    let _ = search.handle_total("pane-2", 1);

    assert_eq!(
        search.find_next(None),
        [GlobalSearchEffect::Navigate {
            target: targets[0].clone(),
            direction: GlobalSearchDirection::Next,
            selected_index: 0,
        }]
    );
    search.handle_selected("pane-1", Some(0));
    assert_eq!(
        search.find_next(Some(&targets[0])),
        [GlobalSearchEffect::Navigate {
            target: targets[0].clone(),
            direction: GlobalSearchDirection::Next,
            selected_index: 1,
        }]
    );
    search.handle_selected("pane-1", Some(1));
    assert_eq!(
        search.find_next(Some(&targets[0])),
        [
            GlobalSearchEffect::ResetSelection {
                pane_id: "pane-1".to_owned(),
            },
            GlobalSearchEffect::Navigate {
                target: targets[2].clone(),
                direction: GlobalSearchDirection::Next,
                selected_index: 0,
            },
        ]
    );
    search.handle_selected("pane-2", Some(0));
    assert_eq!(
        search.find_next(Some(&targets[2])),
        [
            GlobalSearchEffect::ResetSelection {
                pane_id: "pane-2".to_owned(),
            },
            GlobalSearchEffect::Navigate {
                target: targets[0].clone(),
                direction: GlobalSearchDirection::Next,
                selected_index: 0,
            },
        ]
    );
    search.handle_selected("pane-1", Some(0));
    assert_eq!(
        search.find_previous(Some(&targets[0])),
        [
            GlobalSearchEffect::ResetSelection {
                pane_id: "pane-1".to_owned(),
            },
            GlobalSearchEffect::Navigate {
                target: targets[2].clone(),
                direction: GlobalSearchDirection::Previous,
                selected_index: 0,
            },
        ]
    );
}

#[test]
fn selected_result_remains_navigation_anchor_after_ambient_focus_changes() {
    let targets = [target("lane", "pane-1"), target("lane", "pane-2")];
    let mut search = GlobalSearchCoordinator::default();
    search.show(&targets);
    let _ = search.update_query("needle", &targets);
    let _ = search.handle_total("pane-1", 2);
    let _ = search.handle_total("pane-2", 0);
    search.handle_selected("pane-1", Some(0));

    assert_eq!(
        search.find_next(Some(&targets[1])),
        [GlobalSearchEffect::Navigate {
            target: targets[0].clone(),
            direction: GlobalSearchDirection::Next,
            selected_index: 1,
        }]
    );
}

#[test]
fn reconciliation_removes_stale_targets_without_admitting_new_panes() {
    let original = [target("lane-1", "pane-1"), target("lane-2", "pane-2")];
    let mut search = GlobalSearchCoordinator::default();
    search.show(&original);
    let _ = search.update_query("needle", &original);
    let _ = search.handle_total("pane-1", 1);
    let _ = search.handle_total("pane-2", 2);
    search.handle_selected("pane-2", Some(1));

    assert_eq!(
        search.reconcile_live_panes(["pane-1", "pane-3"]),
        [GlobalSearchEffect::End {
            pane_id: "pane-2".to_owned(),
        },]
    );
    assert_eq!(search.frozen_targets(), &[original[0].clone()]);
    assert_eq!(search.state().total, 1);
    assert_eq!(search.state().selected, None);
}

#[test]
fn clear_and_end_have_distinct_hud_and_cleanup_contracts() {
    let targets = [target("lane", "pane-1"), target("lane", "pane-2")];
    let mut search = GlobalSearchCoordinator::default();
    search.show(&targets);
    let _ = search.update_query("needle", &targets);

    assert_eq!(
        search.update_query("", &targets),
        targets.clone().map(|target| {
            GlobalSearchEffect::End {
                pane_id: target.pane_id,
            }
        })
    );
    assert!(search.state().visible);
    assert!(!search.state().has_remembered_search);
    assert_eq!(search.frozen_targets(), &targets);

    let _ = search.update_query("again", &targets);
    assert_eq!(
        search.end(),
        targets.map(|target| GlobalSearchEffect::End {
            pane_id: target.pane_id,
        })
    );
    assert_eq!(search.state(), &GlobalSearchState::default());
    assert!(search.frozen_targets().is_empty());
}

#[test]
fn show_and_implicit_show_capture_only_fresh_sessions() {
    let first = [target("lane", "pane-1")];
    let second = [target("lane", "pane-2")];
    let mut search = GlobalSearchCoordinator::default();
    assert!(!search.state().visible);
    search.show(&first);
    assert!(search.state().visible);
    assert_eq!(search.frozen_targets(), &first);

    let _ = search.update_query("remembered", &first);
    search.show(&second);
    assert_eq!(
        search.frozen_targets(),
        &first,
        "showing a remembered search must not silently replace its targets"
    );

    let mut implicit = GlobalSearchCoordinator::default();
    assert_eq!(
        implicit.update_query("needle", &second),
        [GlobalSearchEffect::Start {
            target: second[0].clone(),
            needle: "needle".to_owned(),
        }]
    );
    assert!(implicit.state().visible);
    assert_eq!(implicit.frozen_targets(), &second);
}

#[test]
fn pending_query_flag_clears_after_dispatch_and_a_new_query_resets_results() {
    let targets = [target("lane", "pane-1")];
    let mut search = GlobalSearchCoordinator::default();
    search.show(&targets);
    assert!(search.update_query("a", &targets).is_empty());
    assert!(search.has_pending_query());
    assert_eq!(search.dispatch_pending_query().len(), 1);
    assert!(!search.has_pending_query());
    let _ = search.handle_total("pane-1", 4);
    search.handle_selected("pane-1", Some(3));
    assert_eq!(search.state().selected, Some(3));

    let _ = search.update_query("different", &targets);
    assert_eq!(search.state().total, 0);
    assert_eq!(search.state().selected, None);
}

#[test]
fn shrinking_totals_and_selected_clear_events_obey_exact_pane_boundaries() {
    let targets = [target("lane", "pane-1"), target("lane", "pane-2")];
    let mut search = GlobalSearchCoordinator::default();
    search.show(&targets);
    let _ = search.update_query("needle", &targets);
    let _ = search.handle_total("pane-1", 3);
    let _ = search.handle_total("pane-2", 2);
    search.handle_selected("pane-1", Some(2));
    assert_eq!(search.state().selected, Some(2));

    let _ = search.handle_total("pane-2", 1);
    assert_eq!(search.state().selected, Some(2));
    search.handle_selected("pane-2", None);
    assert_eq!(search.state().selected, Some(2));
    let _ = search.handle_total("pane-1", 3);
    assert_eq!(search.state().selected, Some(2));
    let _ = search.handle_total("pane-1", 2);
    assert_eq!(search.state().selected, None);
}

#[test]
fn previous_navigation_uses_exact_last_and_within_pane_indices() {
    let targets = [target("lane", "pane-1"), target("lane", "pane-2")];
    let mut search = GlobalSearchCoordinator::default();
    search.show(&targets);
    let _ = search.update_query("needle", &targets);
    let _ = search.handle_total("pane-1", 3);
    let _ = search.handle_total("pane-2", 4);

    assert_eq!(
        search.find_previous(None),
        [GlobalSearchEffect::Navigate {
            target: targets[1].clone(),
            direction: GlobalSearchDirection::Previous,
            selected_index: 3,
        }]
    );
    search.handle_selected("pane-2", Some(3));
    assert_eq!(
        search.find_previous(Some(&targets[1])),
        [GlobalSearchEffect::Navigate {
            target: targets[1].clone(),
            direction: GlobalSearchDirection::Previous,
            selected_index: 2,
        }]
    );
    search.handle_selected("pane-2", Some(0));
    assert_eq!(
        search.find_previous(Some(&targets[1])),
        [
            GlobalSearchEffect::ResetSelection {
                pane_id: "pane-2".to_owned(),
            },
            GlobalSearchEffect::Navigate {
                target: targets[0].clone(),
                direction: GlobalSearchDirection::Previous,
                selected_index: 2,
            },
        ]
    );
}

#[test]
fn previous_navigation_moves_to_the_immediately_preceding_of_three_panes() {
    let targets = [
        target("lane", "pane-1"),
        target("lane", "pane-2"),
        target("lane", "pane-3"),
    ];
    let mut search = GlobalSearchCoordinator::default();
    search.show(&targets);
    let _ = search.update_query("needle", &targets);
    for pane_id in ["pane-1", "pane-2", "pane-3"] {
        let _ = search.handle_total(pane_id, 2);
    }
    search.handle_selected("pane-2", Some(0));

    assert_eq!(
        search.find_previous(Some(&targets[1])),
        [
            GlobalSearchEffect::ResetSelection {
                pane_id: "pane-2".to_owned(),
            },
            GlobalSearchEffect::Navigate {
                target: targets[0].clone(),
                direction: GlobalSearchDirection::Previous,
                selected_index: 1,
            },
        ]
    );
}

#[test]
fn pending_navigation_survives_an_initial_zero_total_until_a_match_arrives() {
    let targets = [target("lane", "pane-1")];
    let mut search = GlobalSearchCoordinator::default();
    search.show(&targets);
    assert!(search.update_query("ab", &targets).is_empty());
    assert_eq!(search.find_next(None).len(), 1);
    assert!(search.handle_total("pane-1", 0).is_empty());
    assert_eq!(
        search.handle_total("pane-1", 1),
        [GlobalSearchEffect::Navigate {
            target: targets[0].clone(),
            direction: GlobalSearchDirection::Next,
            selected_index: 0,
        }]
    );
}

#[test]
fn reconciliation_preserves_a_live_selection_when_an_unselected_pane_closes() {
    let targets = [target("lane", "pane-1"), target("lane", "pane-2")];
    let mut search = GlobalSearchCoordinator::default();
    search.show(&targets);
    let _ = search.update_query("needle", &targets);
    let _ = search.handle_total("pane-1", 2);
    let _ = search.handle_total("pane-2", 3);
    search.handle_selected("pane-1", Some(1));

    assert_eq!(
        search.reconcile_live_panes(["pane-1"]),
        [GlobalSearchEffect::End {
            pane_id: "pane-2".to_owned(),
        }]
    );
    assert_eq!(search.state().total, 2);
    assert_eq!(search.state().selected, Some(1));
}

#[test]
fn reconciliation_is_inert_without_a_remembered_query_and_zero_totals_hold_navigation() {
    let targets = [target("lane", "pane-1")];
    let mut search = GlobalSearchCoordinator::default();
    search.show(&targets);
    assert!(search.reconcile_live_panes(std::iter::empty()).is_empty());
    assert_eq!(search.frozen_targets(), &targets);

    assert!(search.update_query("ab", &targets).is_empty());
    assert_eq!(search.find_next(None).len(), 1);
    assert!(search.handle_total("pane-1", 0).is_empty());
    assert!(search.find_next(None).is_empty());
    assert_eq!(search.state().total, 0);
    assert_eq!(search.state().selected, None);
}
