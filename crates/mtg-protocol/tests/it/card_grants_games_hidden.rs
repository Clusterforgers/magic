//! Goal: across whole bot games at two to four seats with the cards that give keywords to the cards of a hand (Rhet-Tomb Mystic, Tectonic Reformation, Jo Grant, Molecule Man), no viewer's events or snapshots name a card the viewer may not see, whatever a card of a hand was given: the card an ability names as its source (a granted cycling, a granted miracle) is shown to the table with the ability on the stack, as for a printed one, and no other hidden card is named; the games did cycle cards for a given cycling.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::collections::BTreeSet;

use common::{Step, run_game, with_unique_names};
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::Event;
use mtg_protocol::{StateView, project_events};
use mtg_testkit::card_grants_fuzz_setup;
use rstest::rstest;

use crate::hidden_info::{batch_parts, hidden_across, hidden_in, leaks, without_trigger_orders};

/// How often (in answers) a full snapshot is checked.
const SNAPSHOT_EVERY: usize = 20;

#[rstest]
#[case(2, 71)]
#[case(3, 72)]
#[case(4, 73)]
fn no_hidden_object_reaches_any_viewer_in_games_of_the_card_grants(
    #[case] players: usize,
    #[case] seed: u64,
) {
    let viewers: Vec<PlayerId> = (0..players).map(PlayerId::from_index).collect();
    let (mut checked, mut cycled) = (0usize, 0usize);
    let mut shown: BTreeSet<ObjectId> = BTreeSet::new();
    let setup = with_unique_names(card_grants_fuzz_setup(players, seed));
    let engine = run_game(setup, seed, |step: &Step| {
        for event in step.events {
            match event {
                Event::Cycled { .. } => cycled += 1,
                Event::AbilityTriggered(trigger) => {
                    shown.insert(trigger.source.id);
                }
                Event::AbilityPutOnStack(put) => {
                    shown.insert(put.source.id);
                }
                _ => {}
            }
        }
        let batches = project_events(step.before, step.events, &viewers, step.from_seq).unwrap();
        for (viewer, batch) in viewers.iter().zip(&batches) {
            let mut hidden = hidden_across(step.before, step.after, *viewer);
            hidden.retain(|id, _| !shown.contains(id));
            checked += hidden.len();
            for (part, value) in batch_parts(batch) {
                let found = leaks(&without_trigger_orders(value), &hidden);
                assert!(
                    found.is_empty(),
                    "seed {seed} answer {} viewer {viewer:?}: batch {part} leaks {found:?}",
                    step.answers
                );
            }
        }
        if step.answers.is_multiple_of(SNAPSHOT_EVERY) {
            for viewer in &viewers {
                let mut hidden = hidden_in(step.after, *viewer);
                hidden.retain(|id, _| !shown.contains(id));
                let view = StateView::for_player(step.after, *viewer);
                let found = leaks(&serde_json::to_value(&view).unwrap(), &hidden);
                assert!(
                    found.is_empty(),
                    "seed {seed} answer {} viewer {viewer:?}: snapshot leaks {found:?}",
                    step.answers
                );
            }
        }
    });
    assert!(engine.result().is_some(), "game should finish");
    assert!(
        checked > 500 && cycled > 0,
        "test must not be vacuous: {checked} hidden, {cycled} cycled"
    );
}
