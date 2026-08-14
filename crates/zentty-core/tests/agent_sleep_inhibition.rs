use zentty_core::{AgentSleepInhibitionState, SleepInhibitionTransition};

#[test]
fn first_running_source_acquires_once_and_other_sources_share_the_request() {
    let mut state = AgentSleepInhibitionState::default();
    assert_eq!(
        state.update(true, true, 100),
        SleepInhibitionTransition::Acquire
    );
    assert_eq!(
        state.update(true, true, 200),
        SleepInhibitionTransition::None
    );
    assert!(state.lease_requested());
}

#[test]
fn idle_release_is_debounced_and_running_cancels_the_pending_release() {
    let mut state = AgentSleepInhibitionState::new(10_000);
    assert_eq!(
        state.update(true, true, 1_000),
        SleepInhibitionTransition::Acquire
    );
    assert_eq!(
        state.update(true, false, 2_000),
        SleepInhibitionTransition::None
    );
    assert_eq!(state.release_deadline_ms(), Some(12_000));
    assert_eq!(
        state.update(true, true, 11_999),
        SleepInhibitionTransition::None
    );
    assert_eq!(state.release_deadline_ms(), None);
    assert_eq!(
        state.update(true, false, 20_000),
        SleepInhibitionTransition::None
    );
    assert_eq!(
        state.update(true, false, 29_999),
        SleepInhibitionTransition::None
    );
    assert_eq!(
        state.update(true, false, 30_000),
        SleepInhibitionTransition::Release
    );
    assert!(!state.lease_requested());
}

#[test]
fn disabled_never_acquires_and_releases_an_existing_request_immediately() {
    let mut state = AgentSleepInhibitionState::default();
    assert_eq!(
        state.update(false, true, 0),
        SleepInhibitionTransition::None
    );
    assert_eq!(
        state.update(true, true, 1),
        SleepInhibitionTransition::Acquire
    );
    assert_eq!(
        state.update(false, true, 2),
        SleepInhibitionTransition::Release
    );
    assert_eq!(state.release_deadline_ms(), None);
}

#[test]
fn non_running_phases_do_not_start_a_release_timer_without_a_lease() {
    let mut state = AgentSleepInhibitionState::default();
    for now in [0, 10_000, u64::MAX] {
        assert_eq!(
            state.update(true, false, now),
            SleepInhibitionTransition::None
        );
        assert_eq!(state.release_deadline_ms(), None);
    }
}

#[test]
fn backend_failure_does_not_fork_bomb_while_the_same_source_qualifies() {
    let mut state = AgentSleepInhibitionState::default();
    assert_eq!(
        state.update(true, true, 0),
        SleepInhibitionTransition::Acquire
    );
    state.mark_backend_lost(true);
    assert_eq!(state.update(true, true, 1), SleepInhibitionTransition::None);
    assert_eq!(
        state.update(true, false, 2),
        SleepInhibitionTransition::None
    );
    assert_eq!(
        state.update(true, true, 3),
        SleepInhibitionTransition::Acquire
    );
}

#[test]
fn force_release_is_idempotent_and_bypasses_the_debounce() {
    let mut state = AgentSleepInhibitionState::default();
    assert_eq!(
        state.update(true, true, 0),
        SleepInhibitionTransition::Acquire
    );
    assert_eq!(state.force_release(), SleepInhibitionTransition::Release);
    assert_eq!(state.force_release(), SleepInhibitionTransition::None);
}
