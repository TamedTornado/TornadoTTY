use std::collections::HashSet;

use zentty_core::{Pane, StableId, Workspace};

struct Generator {
    state: u64,
    next_id: u64,
}

impl Generator {
    fn new(seed: u64) -> Self {
        Self {
            state: seed,
            next_id: 10,
        }
    }

    fn number(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.state
    }

    fn index(&mut self, len: usize) -> usize {
        usize::try_from(self.number() % u64::try_from(len).unwrap()).unwrap()
    }

    fn id(&mut self) -> StableId {
        let value = self.next_id;
        self.next_id += 1;
        StableId::parse(format!("00000000-0000-4000-8000-{value:012x}")).unwrap()
    }

    fn pane(&mut self) -> Pane {
        Pane::new(self.id(), "/tmp", "default").unwrap()
    }
}

fn initial_workspace(generator: &mut Generator) -> Workspace {
    let workspace_id = generator.id();
    let window_id = generator.id();
    let worklane_id = generator.id();
    let pane = generator.pane();
    Workspace::new(workspace_id, window_id, worklane_id, pane).unwrap()
}

#[test]
fn deterministic_mutation_sequences_preserve_every_topology_invariant() {
    for seed in 1..=32 {
        let mut generator = Generator::new(seed);
        let mut workspace = initial_workspace(&mut generator);
        for step in 0..500 {
            mutate(&mut workspace, &mut generator);
            assert_invariants(&workspace, seed, step);
            let encoded = workspace.to_json().unwrap();
            assert_eq!(
                Workspace::from_json(&encoded),
                Ok(workspace.clone()),
                "round-trip failed for seed {seed} at step {step}"
            );
        }
    }
}

fn mutate(workspace: &mut Workspace, generator: &mut Generator) {
    let window_id = workspace.active_window_id().clone();
    let operation = generator.index(9);
    match operation {
        0 if workspace.windows()[0].worklanes().len() < 6 => {
            let lane_id = generator.id();
            let pane = generator.pane();
            workspace
                .add_worklane(
                    &window_id,
                    lane_id,
                    Some(format!("lane-{}", generator.number() % 100)),
                    pane,
                )
                .unwrap();
        }
        1 => {
            let lane_id = random_lane(workspace, generator);
            workspace
                .rename_worklane(
                    &window_id,
                    &lane_id,
                    Some(format!("renamed-{}", generator.number() % 100)),
                )
                .unwrap();
        }
        2 => {
            let lane_id = random_lane(workspace, generator);
            let destination = generator.index(workspace.windows()[0].worklanes().len());
            workspace
                .move_worklane(&window_id, &lane_id, destination)
                .unwrap();
        }
        3 => {
            let lane_id = random_lane(workspace, generator);
            workspace.select_worklane(&window_id, &lane_id).unwrap();
        }
        4 if workspace.windows()[0].worklanes().len() > 1 => {
            let lane_id = random_lane(workspace, generator);
            workspace.remove_worklane(&window_id, &lane_id).unwrap();
        }
        5 => {
            let lane_id = random_lane(workspace, generator);
            let pane_count = lane(workspace, &lane_id).panes().len();
            if pane_count < 8 {
                let pane = generator.pane();
                workspace.add_pane(&window_id, &lane_id, pane).unwrap();
            }
        }
        6 => {
            let lane_id = random_lane(workspace, generator);
            let pane_id = random_pane(workspace, &lane_id, generator);
            workspace
                .select_pane(&window_id, &lane_id, &pane_id)
                .unwrap();
        }
        7 => {
            let lane_id = random_lane(workspace, generator);
            let pane_id = random_pane(workspace, &lane_id, generator);
            let destination = generator.index(lane(workspace, &lane_id).panes().len());
            workspace
                .move_pane(&window_id, &lane_id, &pane_id, destination)
                .unwrap();
        }
        8 => {
            let lane_id = random_lane(workspace, generator);
            if lane(workspace, &lane_id).panes().len() > 1 {
                let pane_id = random_pane(workspace, &lane_id, generator);
                workspace
                    .remove_pane(&window_id, &lane_id, &pane_id)
                    .unwrap();
            }
        }
        _ => {}
    }
}

fn random_lane(workspace: &Workspace, generator: &mut Generator) -> StableId {
    let lanes = workspace.windows()[0].worklanes();
    lanes[generator.index(lanes.len())].id().clone()
}

fn random_pane(workspace: &Workspace, lane_id: &StableId, generator: &mut Generator) -> StableId {
    let panes = lane(workspace, lane_id).panes();
    panes[generator.index(panes.len())].id().clone()
}

fn lane<'a>(workspace: &'a Workspace, lane_id: &StableId) -> &'a zentty_core::Worklane {
    workspace.windows()[0]
        .worklanes()
        .iter()
        .find(|candidate| candidate.id() == lane_id)
        .unwrap()
}

fn assert_invariants(workspace: &Workspace, seed: u64, step: usize) {
    let mut ids = HashSet::new();
    let active_window = workspace.active_window().unwrap();
    assert!(
        ids.insert(active_window.id().as_str()),
        "seed {seed}, step {step}"
    );
    assert!(
        active_window.active_worklane().is_some(),
        "seed {seed}, step {step}"
    );
    for worklane in active_window.worklanes() {
        assert!(
            ids.insert(worklane.id().as_str()),
            "seed {seed}, step {step}"
        );
        assert!(!worklane.panes().is_empty(), "seed {seed}, step {step}");
        assert!(
            worklane
                .panes()
                .iter()
                .any(|pane| pane.id() == worklane.active_pane_id()),
            "seed {seed}, step {step}"
        );
        for (row, pane) in worklane.panes().iter().enumerate() {
            assert!(ids.insert(pane.id().as_str()), "seed {seed}, step {step}");
            assert_eq!(pane.layout().column(), 0, "seed {seed}, step {step}");
            assert_eq!(pane.layout().row(), row, "seed {seed}, step {step}");
        }
    }
}
