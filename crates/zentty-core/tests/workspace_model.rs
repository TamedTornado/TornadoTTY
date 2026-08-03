use zentty_core::{Pane, StableId, Workspace, WorkspaceError};

const WORKSPACE: &str = "00000000-0000-4000-8000-000000000001";
const WINDOW: &str = "00000000-0000-4000-8000-000000000002";
const LANE_A: &str = "00000000-0000-4000-8000-000000000003";
const LANE_B: &str = "00000000-0000-4000-8000-000000000004";
const LANE_C: &str = "00000000-0000-4000-8000-000000000005";
const PANE_A: &str = "00000000-0000-4000-8000-000000000006";
const PANE_B: &str = "00000000-0000-4000-8000-000000000007";
const PANE_C: &str = "00000000-0000-4000-8000-000000000008";

fn id(value: &str) -> StableId {
    StableId::parse(value).expect("test ID must be valid")
}

fn pane(value: &str) -> Pane {
    Pane::new(id(value), "/tmp", "default").expect("test pane must be valid")
}

fn workspace() -> Workspace {
    Workspace::new(id(WORKSPACE), id(WINDOW), id(LANE_A), pane(PANE_A))
        .expect("initial topology must be valid")
}

#[test]
fn worklane_mutations_preserve_order_selection_and_revision() {
    let mut workspace = workspace();

    workspace
        .add_worklane(&id(WINDOW), id(LANE_B), Some("Build".into()), pane(PANE_B))
        .unwrap();
    workspace
        .add_worklane(&id(WINDOW), id(LANE_C), Some("Review".into()), pane(PANE_C))
        .unwrap();
    workspace.select_worklane(&id(WINDOW), &id(LANE_B)).unwrap();
    workspace
        .move_worklane(&id(WINDOW), &id(LANE_C), 0)
        .unwrap();
    workspace
        .rename_worklane(&id(WINDOW), &id(LANE_B), Some("Ship".into()))
        .unwrap();

    let window = &workspace.windows()[0];
    let order: Vec<_> = window
        .worklanes()
        .iter()
        .map(|lane| lane.id().as_str())
        .collect();
    assert_eq!(order, [LANE_C, LANE_A, LANE_B]);
    assert_eq!(window.active_worklane_id().as_str(), LANE_B);
    assert_eq!(window.worklanes()[2].title(), Some("Ship"));
    assert_eq!(workspace.revision(), 5);

    workspace.remove_worklane(&id(WINDOW), &id(LANE_B)).unwrap();
    assert_eq!(workspace.windows()[0].active_worklane_id().as_str(), LANE_A);
}

#[test]
fn pane_mutations_repair_active_selection_deterministically() {
    let mut workspace = workspace();
    workspace
        .add_pane(&id(WINDOW), &id(LANE_A), pane(PANE_B))
        .unwrap();
    workspace
        .add_pane(&id(WINDOW), &id(LANE_A), pane(PANE_C))
        .unwrap();
    workspace
        .select_pane(&id(WINDOW), &id(LANE_A), &id(PANE_B))
        .unwrap();
    workspace
        .move_pane(&id(WINDOW), &id(LANE_A), &id(PANE_C), 0)
        .unwrap();

    let lane = &workspace.windows()[0].worklanes()[0];
    let order: Vec<_> = lane.panes().iter().map(|pane| pane.id().as_str()).collect();
    assert_eq!(order, [PANE_C, PANE_A, PANE_B]);

    workspace
        .remove_pane(&id(WINDOW), &id(LANE_A), &id(PANE_B))
        .unwrap();
    assert_eq!(
        workspace.windows()[0].worklanes()[0]
            .active_pane_id()
            .as_str(),
        PANE_A
    );
}

#[test]
fn invalid_and_destructive_mutations_fail_without_changing_state() {
    let mut workspace = workspace();
    let original = workspace.clone();

    assert_eq!(
        workspace.remove_pane(&id(WINDOW), &id(LANE_A), &id(PANE_A)),
        Err(WorkspaceError::CannotRemoveFinalPane)
    );
    assert_eq!(
        workspace.remove_worklane(&id(WINDOW), &id(LANE_A)),
        Err(WorkspaceError::CannotRemoveFinalWorklane)
    );
    assert_eq!(workspace, original);

    assert!(matches!(
        StableId::parse("not-an-id"),
        Err(WorkspaceError::InvalidStableId(_))
    ));
    assert!(matches!(
        Pane::new(id(PANE_B), "relative/path", "default"),
        Err(WorkspaceError::InvalidCwd(_))
    ));
    assert!(matches!(
        Pane::new(id(PANE_B), "/tmp", "UPPERCASE"),
        Err(WorkspaceError::InvalidLaunchProfileId(_))
    ));

    assert_eq!(
        workspace.move_worklane(&id(WINDOW), &id(LANE_B), 0),
        Err(WorkspaceError::WorklaneNotFound(id(LANE_B)))
    );
    assert_eq!(
        workspace.move_pane(&id(WINDOW), &id(LANE_A), &id(PANE_B), 0),
        Err(WorkspaceError::PaneNotFound(id(PANE_B)))
    );
}

#[test]
fn entity_ids_are_globally_unique() {
    let mut workspace = workspace();
    let duplicate_pane = Pane::new(id(LANE_A), "/tmp", "default").unwrap();
    assert_eq!(
        workspace.add_pane(&id(WINDOW), &id(LANE_A), duplicate_pane),
        Err(WorkspaceError::DuplicateId(id(LANE_A)))
    );

    assert_eq!(
        Workspace::new(id(WORKSPACE), id(WINDOW), id(WINDOW), pane(PANE_A)),
        Err(WorkspaceError::DuplicateId(id(WINDOW)))
    );

    assert_eq!(
        workspace.add_worklane(&id(WINDOW), id(PANE_B), None, pane(PANE_B)),
        Err(WorkspaceError::DuplicateId(id(PANE_B)))
    );
}
