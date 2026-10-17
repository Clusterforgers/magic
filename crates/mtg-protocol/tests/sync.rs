//! Goal: in seeded 2-, 3- and 4-seat bot games, folding each event batch into a seat's previous view
//! gives exactly the view built from the true state after it, at every batch and for every seat,
//! and each batch lists exactly the objects the seat gained, saw change or lost.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::collections::{BTreeMap, BTreeSet};

use common::{Step, run_game, unique_setup, visible_ids};
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_protocol::{EventBatch, ObjectView, StateView, project_events};
use rstest::rstest;

/// How often (in answers) a snapshot is also checked to survive its JSON form.
const SNAPSHOT_EVERY: usize = 25;

/// The batch as the client receives it: after a trip through JSON.
fn over_the_wire(batch: &EventBatch) -> EventBatch {
    serde_json::from_str(&serde_json::to_string(batch).unwrap()).unwrap()
}

fn by_id(objects: &[ObjectView]) -> BTreeMap<ObjectId, &ObjectView> {
    objects.iter().map(|o| (o.id, o)).collect()
}

/// Fails with the ids that differ rather than a dump of two whole views.
fn assert_same(view: &StateView, truth: &StateView, context: &str) {
    assert!(view.table == truth.table, "{context}: tables differ");
    let (have, want) = (by_id(&view.objects), by_id(&truth.objects));
    let missing: Vec<_> = want.keys().filter(|id| !have.contains_key(id)).collect();
    let extra: Vec<_> = have.keys().filter(|id| !want.contains_key(id)).collect();
    let stale: Vec<_> = want
        .iter()
        .filter(|(id, object)| have.get(id).is_some_and(|mine| mine != *object))
        .map(|(id, _)| id)
        .collect();
    assert!(
        missing.is_empty() && extra.is_empty() && stale.is_empty(),
        "{context}: missing {missing:?}, extra {extra:?}, out of date {stale:?}"
    );
    assert!(view == truth, "{context}: objects are not in id order");
}

/// What a batch has to say about objects, judged by the independent visibility rule in `common`.
fn assert_lists_what_changed(step: &Step, viewer: PlayerId, batch: &EventBatch, context: &str) {
    let before = visible_ids(step.before, viewer);
    let after = visible_ids(step.after, viewer);
    let listed: BTreeSet<ObjectId> = batch.objects.iter().map(|o| o.id).collect();
    let lost: Vec<ObjectId> = before.difference(&after).copied().collect();
    assert_eq!(batch.gone, lost, "{context}: gone");
    assert!(listed.is_subset(&after), "{context}: lists a hidden object");
    assert!(
        after.difference(&before).all(|id| listed.contains(id)),
        "{context}: omits an object that is new to the viewer"
    );
    if step.from_seq == 0 {
        assert_eq!(listed, after, "{context}: the first batch lists everything");
    }
}

/// Every listed object is new to the viewer or differs from the one they had.
fn assert_lists_only_changes(previous: &StateView, batch: &EventBatch, context: &str) {
    let known = by_id(&previous.objects);
    for object in &batch.objects {
        assert!(
            known.get(&object.id).copied() != Some(object),
            "{context}: lists unchanged {:?}",
            object.id
        );
    }
}

#[derive(Default)]
struct Tally {
    batches: usize,
    upserted: usize,
    gone: usize,
    snapshots: usize,
}

#[rstest]
#[case(2, 61)]
#[case(2, 62)]
#[case(3, 63)]
#[case(3, 64)]
#[case(4, 65)]
#[case(4, 66)]
fn folding_batches_reproduces_the_true_view_for_every_seat(
    #[case] players: usize,
    #[case] seed: u64,
) {
    let viewers: Vec<PlayerId> = (0..players).map(PlayerId::from_index).collect();
    let mut views: Vec<Option<StateView>> = vec![None; players];
    let mut tally = Tally::default();
    let engine = run_game(unique_setup(players, seed), seed, |step: &Step| {
        let batches = project_events(step.before, step.events, &viewers, step.from_seq).unwrap();
        for (viewer, batch) in viewers.iter().zip(&batches) {
            let context = format!("seed {seed} answer {} viewer {viewer:?}", step.answers);
            let batch = over_the_wire(batch);
            let truth = StateView::for_player(step.after, *viewer);
            assert_lists_what_changed(step, *viewer, &batch, &context);
            tally.batches += 1;
            tally.upserted += batch.objects.len();
            tally.gone += batch.gone.len();
            match &mut views[viewer.index()] {
                slot @ None => {
                    // A client that joined the lobby has no snapshot and starts from the first batch.
                    let started = StateView::from_first_batch(&batch).unwrap();
                    assert_same(&started, &truth, &context);
                    // One that was sent a snapshot of the table before the game starts folds it like any other.
                    let mut snapped = StateView::for_player(step.before, *viewer);
                    snapped.apply(&batch);
                    assert_same(&snapped, &truth, &context);
                    *slot = Some(started);
                }
                Some(view) => {
                    assert_lists_only_changes(view, &batch, &context);
                    view.apply(&batch);
                    assert_same(view, &truth, &context);
                }
            }
            if step.answers.is_multiple_of(SNAPSHOT_EVERY) {
                let text = serde_json::to_string(&truth).unwrap();
                assert!(
                    serde_json::from_str::<StateView>(&text).unwrap() == truth,
                    "{context}: snapshot changed in JSON"
                );
                tally.snapshots += 1;
            }
        }
    });
    assert!(engine.result().is_some(), "game should finish");
    assert!(
        tally.batches > 500 && tally.upserted > 300 && tally.gone > 0 && tally.snapshots > 10,
        "test must not be vacuous: {} batches, {} upserted, {} gone, {} snapshots",
        tally.batches,
        tally.upserted,
        tally.gone,
        tally.snapshots
    );
}
