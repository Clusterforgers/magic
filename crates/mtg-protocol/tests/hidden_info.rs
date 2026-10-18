//! Goal: across seeded bot games, no viewer's snapshot or event stream ever contains an object
//! (by name or id) that the viewer was not allowed to see.
//!
//! "Hidden" is defined here independently of the crate: a card in any library, or in another seat's hand.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::collections::{BTreeMap, BTreeSet};

use common::{Step, is_hidden, run_game, unique_setup};
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::object::GameObject;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, GameState};
use mtg_protocol::{EventBatch, StateView, project_events};
use rstest::rstest;
use serde_json::Value;

/// Keys whose numbers are counters or seat numbers, never object ids.
const SCALAR_KEYS: &[&str] = &[
    "counts",
    "life",
    "life_after",
    "count",
    "number",
    "lands_played",
    "mulligans",
    "damage",
    "power",
    "toughness",
    "amount",
    "from_seq",
    "ability",
    "Bottoming",
    "protocol_version",
    "player",
    "owner",
    "controller",
    "seat",
    "active",
    "holder",
    "viewer",
    "passed",
    "first_player",
    "blockers_declared",
    "Player",
    "Winner",
    "colored",
    "colorless",
    "generic",
    "Hand",
    "Library",
    "Graveyard",
    "Command",
];

/// How often (in answers) a full snapshot is checked; events are checked after every answer.
const SNAPSHOT_EVERY: usize = 20;

fn hidden_in(state: &GameState, viewer: PlayerId) -> BTreeMap<ObjectId, String> {
    state
        .objects()
        .filter(|o| is_hidden(o.zone(), viewer))
        .map(|o| (o.id(), o.card().name.clone()))
        .collect()
}

/// Objects hidden from `viewer` before a batch that are not visible to them afterwards.
fn hidden_across(
    before: &GameState,
    after: &GameState,
    viewer: PlayerId,
) -> BTreeMap<ObjectId, String> {
    hidden_in(before, viewer)
        .into_iter()
        .filter(|(id, _)| {
            after
                .object(*id)
                .is_none_or(|o| is_hidden(o.zone(), viewer))
        })
        .collect()
}

/// Describes every place `value` mentions one of the hidden objects.
fn leaks(value: &Value, hidden: &BTreeMap<ObjectId, String>) -> Vec<String> {
    fn walk(value: &Value, key: &str, hidden: &BTreeMap<ObjectId, String>, out: &mut Vec<String>) {
        match value {
            Value::String(text) => {
                if let Some((id, _)) = hidden.iter().find(|(_, name)| *name == text) {
                    out.push(format!("name of {id:?} under {key}"));
                }
            }
            Value::Number(n) => {
                let id = n.as_u64().map(ObjectId);
                let hidden_id = id.filter(|id| hidden.contains_key(id));
                if let (Some(id), false) = (hidden_id, SCALAR_KEYS.contains(&key)) {
                    out.push(format!("{id:?} under {key}"));
                }
            }
            // Damage assignments are (object, amount) pairs; only the object is an id.
            Value::Array(items) if key == "assignment" => {
                for pair in items {
                    if let Some(object) = pair.get(0) {
                        walk(object, key, hidden, out);
                    }
                }
            }
            Value::Array(items) => items.iter().for_each(|v| walk(v, key, hidden, out)),
            Value::Object(map) => map.iter().for_each(|(k, v)| walk(v, k, hidden, out)),
            Value::Null | Value::Bool(_) => {}
        }
    }
    let mut out = Vec::new();
    walk(value, "", hidden, &mut out);
    out
}

/// The parts of a batch that can mention an object, named so a failure says where the leak is.
fn batch_parts(batch: &EventBatch) -> [(&'static str, Value); 4] {
    [
        ("events", serde_json::to_value(&batch.events).unwrap()),
        ("table", serde_json::to_value(&batch.table).unwrap()),
        ("objects", serde_json::to_value(&batch.objects).unwrap()),
        ("gone", serde_json::to_value(&batch.gone).unwrap()),
    ]
}

#[rstest]
#[case(2, 1)]
#[case(2, 2)]
#[case(3, 3)]
#[case(4, 4)]
#[case(4, 5)]
fn no_hidden_object_reaches_any_viewer(#[case] players: usize, #[case] seed: u64) {
    let viewers: Vec<PlayerId> = (0..players).map(PlayerId::from_index).collect();
    let mut checked = 0usize;
    let mut snapshots = 0usize;
    let engine = run_game(unique_setup(players, seed), seed, |step: &Step| {
        let batches = project_events(step.before, step.events, &viewers, step.from_seq).unwrap();
        for (viewer, batch) in viewers.iter().zip(&batches) {
            let hidden = hidden_across(step.before, step.after, *viewer);
            checked += hidden.len();
            for (part, value) in batch_parts(batch) {
                let found = leaks(&value, &hidden);
                assert!(
                    found.is_empty(),
                    "seed {seed} answer {} viewer {viewer:?}: batch {part} leaks {found:?}",
                    step.answers
                );
            }
        }
        if step.answers.is_multiple_of(SNAPSHOT_EVERY) {
            snapshots += 1;
            for viewer in &viewers {
                let hidden = hidden_in(step.after, *viewer);
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
    assert!(checked > 500 && snapshots > 5, "test must not be vacuous");
}

#[rstest]
#[case(2, 11)]
#[case(4, 12)]
fn snapshots_show_exactly_what_the_viewer_may_know(#[case] players: usize, #[case] seed: u64) {
    let viewers: Vec<PlayerId> = (0..players).map(PlayerId::from_index).collect();
    run_game(unique_setup(players, seed), seed, |step: &Step| {
        if !step.answers.is_multiple_of(SNAPSHOT_EVERY) {
            return;
        }
        for viewer in &viewers {
            let view = StateView::for_player(step.after, *viewer);
            let visible: BTreeSet<ObjectId> = view.objects.iter().map(|o| o.id).collect();
            let expected: BTreeSet<ObjectId> = step
                .after
                .objects()
                .filter(|o| !is_hidden(o.zone(), *viewer))
                .map(GameObject::id)
                .collect();
            assert_eq!(
                visible, expected,
                "viewer {viewer:?} sees the wrong objects"
            );
            for seat in &view.table.players {
                let hand = step.after.zone(Zone::Hand(seat.seat));
                assert_eq!(seat.hand.count as usize, hand.len());
                assert_eq!(
                    seat.hand.cards.is_some(),
                    seat.seat == *viewer,
                    "only the owner sees a hand"
                );
                assert_eq!(
                    seat.library.count as usize,
                    step.after.zone(Zone::Library(seat.seat)).len()
                );
                assert!(seat.library.cards.is_none(), "libraries are never listed");
            }
        }
    });
}

#[test]
fn the_scan_flags_a_hidden_card_in_each_part_of_a_batch() {
    let setup = unique_setup(2, 7);
    let before = GameState::from_setup(&setup).unwrap();
    let engine = Engine::new(setup).unwrap();
    let viewer = PlayerId(0);
    let clean = project_events(&before, engine.events(), &[viewer], 0)
        .unwrap()
        .remove(0);
    let hidden = hidden_in(engine.state(), viewer);
    assert!(hidden.len() > 40, "most of the game is hidden at the start");
    for (part, value) in batch_parts(&clean) {
        assert!(
            leaks(&value, &hidden).is_empty(),
            "control: {part} is clean"
        );
    }

    let (id, name) = hidden
        .iter()
        .next()
        .map(|(id, name)| (*id, name.clone()))
        .unwrap();
    let mut in_gone = clean.clone();
    in_gone.gone.push(id);
    let mut in_table = clean.clone();
    in_table.table.players[1].hand.cards = Some(vec![id]);
    let mut in_objects = clean.clone();
    let mut card = clean.objects[0].clone();
    card.id = id;
    card.name = name;
    in_objects.objects.push(card);
    for (part, batch) in [
        ("gone", in_gone),
        ("table", in_table),
        ("objects", in_objects),
    ] {
        let flagged = batch_parts(&batch)
            .into_iter()
            .any(|(name, value)| name == part && !leaks(&value, &hidden).is_empty());
        assert!(flagged, "a hidden card in {part} went unnoticed");
    }
}
