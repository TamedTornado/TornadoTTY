use std::collections::BTreeSet;
use zentty_core::WindowRecipe;

/// Recover snapshots written by the full-height reflow identity collision.
/// Only column identities change: panes, drafts, geometry and order survive.
pub(super) fn repair(window: &mut WindowRecipe) -> usize {
    let mut reserved = window
        .worklanes
        .iter()
        .flat_map(|lane| &lane.columns)
        .map(|column| column.id.clone())
        .collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    let mut repaired = 0;
    for lane in &mut window.worklanes {
        // A duplicate within one lane makes saved focus ambiguous. Preserve
        // the first-match interpretation rather than inventing another pane.
        let focused_index = lane
            .columns
            .iter()
            .position(|column| Some(&column.id) == lane.focused_column_id.as_ref());
        for (index, column) in lane.columns.iter_mut().enumerate() {
            if seen.insert(column.id.clone()) {
                continue;
            }
            let base = column.id.clone();
            let mut suffix = 2;
            loop {
                let candidate = format!("{base}-{suffix}");
                if reserved.insert(candidate.clone()) {
                    column.id = candidate;
                    break;
                }
                suffix += 1;
            }
            seen.insert(column.id.clone());
            if focused_index == Some(index) {
                lane.focused_column_id = Some(column.id.clone());
            }
            repaired += 1;
        }
    }
    repaired
}
