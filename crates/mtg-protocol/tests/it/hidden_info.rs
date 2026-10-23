//! Goal: across seeded bot games, no viewer's snapshot or event stream ever contains an object
//! (by name or id) that the viewer was not allowed to see.
//!
//! "Hidden" is defined here independently of the crate: a card in any library, or in another seat's hand.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::collections::{BTreeMap, BTreeSet};

use common::{Step, is_hidden, revealed_tops, run_game, unique_setup, with_unique_names};
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::Choice;
use mtg_engine::object::GameObject;
use mtg_engine::zones::Zone;
use mtg_engine::{Decision, DecisionKind, Engine, GameState};
use mtg_protocol::{EventBatch, StateView, project_events};
use mtg_testkit::{
    archon_fuzz_setup, attach_fuzz_setup, counters_fuzz_setup, keyword_triggers_fuzz_setup,
    loyalty_fuzz_setup, parts_fuzz_setup, prevent_fuzz_setup, rules_fuzz_setup, spell_fuzz_setup,
    split_fuzz_setup, umbra_fuzz_setup, upkeep_fuzz_setup, zones_fuzz_setup,
};
use rstest::rstest;
use serde_json::Value;

/// Keys whose numbers are counters or seat numbers, never object ids.
const SCALAR_KEYS: &[&str] = &[
    "PayLife",
    "counts",
    "life",
    "life_after",
    "count",
    "number",
    "lands_played",
    "mulligans",
    "bank",
    "damage",
    "power",
    "toughness",
    "amount",
    "left",
    "Option",
    "Replacement",
    "from_seq",
    "ability",
    "counter",
    "counters",
    "delta",
    "Plus",
    "Minus",
    "types",
    "colors",
    "position",
    "modes",
    "x",
    "optional_costs",
    "division",
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
    "Generic",
    "Crew",
    "Hideaway",
    "Saddle",
    "timestamp",
    "turn",
    "rule",
    "index",
    "Effect",
    "PrintedKeyword",
    "max_life",
    "Hand",
    "Library",
    "Graveyard",
    "Command",
    "AnyColor",
    "color_indicator",
    "face",
    "who",
];

/// How often (in answers) a full snapshot is checked; events are checked after every answer.
const SNAPSHOT_EVERY: usize = 20;

/// Games per table size in the prevention campaign.
const PREVENTION_GAMES: u64 = 1;

pub(crate) fn hidden_in(state: &GameState, viewer: PlayerId) -> BTreeMap<ObjectId, String> {
    let tops: Vec<ObjectId> = revealed_tops(state).into_values().collect();
    state
        .objects()
        .filter(|o| is_hidden(o.zone(), viewer) && !tops.contains(&o.id()))
        .map(|o| (o.id(), o.card().name.clone()))
        .collect()
}

/// Objects hidden from `viewer` before a batch that are not visible to them afterwards.
pub(crate) fn hidden_across(
    before: &GameState,
    after: &GameState,
    viewer: PlayerId,
) -> BTreeMap<ObjectId, String> {
    let tops: Vec<ObjectId> = revealed_tops(after).into_values().collect();
    hidden_in(before, viewer)
        .into_iter()
        .filter(|(id, _)| {
            after
                .object(*id)
                .is_none_or(|o| is_hidden(o.zone(), viewer) && !tops.contains(id))
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
            Value::Object(map) => {
                // A continuous effect, and the event that ends one, name it by an effect id, which is no object's.
                let names_an_effect = key == "EffectExpired"
                    || key == "RuleExpired"
                    || key == "ShieldUsed"
                    || (map.contains_key("duration")
                        && (map.contains_key("timestamp") || map.contains_key("turn")));
                map.iter()
                    .filter(|(k, _)| !(names_an_effect && *k == "id"))
                    .for_each(|(k, v)| walk(v, k, hidden, out));
            }
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

/// Spells with targets and abilities on the stack name only things every seat may see: the cast of a spell, the
/// ability that goes on the stack and what the resolution binds and moves are public, and no hand or library card
/// is named by them.
#[rstest]
#[case(2, 21)]
#[case(3, 22)]
#[case(4, 23)]
fn no_hidden_object_reaches_any_viewer_in_games_of_spells_and_abilities(
    #[case] players: usize,
    #[case] seed: u64,
) {
    let viewers: Vec<PlayerId> = (0..players).map(PlayerId::from_index).collect();
    let (mut checked, mut abilities) = (0usize, 0usize);
    let setup = with_unique_names(spell_fuzz_setup(players, seed));
    let engine = run_game(setup, seed, |step: &Step| {
        abilities += step
            .events
            .iter()
            .filter(|event| matches!(event, mtg_engine::Event::AbilityPutOnStack(_)))
            .count();
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
    });
    assert!(engine.result().is_some(), "game should finish");
    assert!(checked > 500 && abilities > 0, "test must not be vacuous");
}

/// The division of a spell, an ability and a trigger names only the targets they have, which every seat sees: the
/// cast, the ability on the stack and the amounts each target gets reach every seat whole, and no hand or library
/// card is named by them.
#[rstest]
#[case(2, 71)]
#[case(3, 72)]
#[case(4, 73)]
fn no_hidden_object_reaches_any_viewer_in_games_of_divided_spells(
    #[case] players: usize,
    #[case] seed: u64,
) {
    let viewers: Vec<PlayerId> = (0..players).map(PlayerId::from_index).collect();
    let (mut checked, mut divisions) = (0usize, 0usize);
    let setup = with_unique_names(split_fuzz_setup(players, seed));
    let engine = run_game(setup, seed, |step: &Step| {
        divisions += step
            .events
            .iter()
            .filter(|event| match event {
                mtg_engine::Event::SpellCast { choices, .. } => choices.division.len() > 1,
                mtg_engine::Event::AbilityPutOnStack(put) => put.choices.division.len() > 1,
                _ => false,
            })
            .count();
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
    });
    assert!(engine.result().is_some(), "game should finish");
    assert!(checked > 500 && divisions > 0, "test must not be vacuous");
}

/// Counters on permanents and players are public: the events that put them on and take them off, and the counters of
/// every permanent and seat in a snapshot, name only things every seat may see, and a hidden card never holds any.
#[rstest]
#[case(2, 61)]
#[case(3, 62)]
#[case(4, 63)]
fn no_hidden_object_reaches_any_viewer_in_games_of_counters(
    #[case] players: usize,
    #[case] seed: u64,
) {
    let viewers: Vec<PlayerId> = (0..players).map(PlayerId::from_index).collect();
    let (mut checked, mut counter_events, mut counted_views) = (0usize, 0usize, 0usize);
    let setup = with_unique_names(counters_fuzz_setup(players, seed).unwrap());
    let engine = run_game(setup, seed, |step: &Step| {
        counter_events += step
            .events
            .iter()
            .filter(|event| matches!(event, mtg_engine::Event::CountersChanged(_)))
            .count();
        let batches = project_events(step.before, step.events, &viewers, step.from_seq).unwrap();
        for (viewer, batch) in viewers.iter().zip(&batches) {
            let hidden = hidden_across(step.before, step.after, *viewer);
            checked += hidden.len();
            for (part, value) in batch_parts(batch) {
                let found = leaks(&without_trigger_orders(value), &hidden);
                assert!(
                    found.is_empty(),
                    "seed {seed} answer {} viewer {viewer:?}: batch {part} leaks {found:?}",
                    step.answers
                );
            }
            // Every counter event reaches every viewer whole: counters are never hidden.
            let seen = batch
                .events
                .iter()
                .filter(|view| {
                    matches!(
                        view,
                        mtg_protocol::EventView::Public(mtg_engine::Event::CountersChanged(_))
                    )
                })
                .count();
            let logged = step
                .events
                .iter()
                .filter(|event| matches!(event, mtg_engine::Event::CountersChanged(_)))
                .count();
            assert_eq!(seen, logged, "seed {seed} viewer {viewer:?}");
        }
        if step.answers.is_multiple_of(SNAPSHOT_EVERY) {
            for viewer in &viewers {
                let hidden = hidden_in(step.after, *viewer);
                let view = StateView::for_player(step.after, *viewer);
                let found = leaks(&serde_json::to_value(&view).unwrap(), &hidden);
                assert!(
                    found.is_empty(),
                    "seed {seed} answer {} viewer {viewer:?}: snapshot leaks {found:?}",
                    step.answers
                );
                // A permanent shows its counters and a seat shows its own to every viewer alike.
                for object in &view.objects {
                    if object.zone != Zone::Battlefield {
                        assert!(
                            object.counters.is_empty(),
                            "{} off the battlefield",
                            object.name
                        );
                    }
                }
                for player in &view.table.players {
                    let state = step.after.player(player.seat).unwrap();
                    let held: Vec<_> = state
                        .counters()
                        .map(|(kind, count)| (kind.clone(), count))
                        .collect();
                    assert_eq!(player.counters, held, "seed {seed} seat {:?}", player.seat);
                }
                counted_views += usize::from(
                    view.objects.iter().any(|o| !o.counters.is_empty())
                        || view.table.players.iter().any(|p| !p.counters.is_empty()),
                );
            }
        }
    });
    assert!(engine.result().is_some(), "game should finish");
    assert!(
        checked > 500 && counter_events > 0 && counted_views > 0,
        "test must not be vacuous: {checked} hidden, {counter_events} counter events, {counted_views} views with counters"
    );
}

/// Planeswalkers are public: their loyalty counters and loyalty abilities, the activations and the attackers aimed at
/// them name only things every seat may see; a loyalty ability of a planeswalker on the battlefield is listed with its
/// cost to every viewer, a card in a hand lists none, and the loyalty counters of a snapshot are those the permanent holds.
#[rstest]
#[case(2, 71)]
#[case(3, 72)]
#[case(4, 73)]
fn no_hidden_object_reaches_any_viewer_in_games_of_planeswalkers(
    #[case] players: usize,
    #[case] seed: u64,
) {
    let viewers: Vec<PlayerId> = (0..players).map(PlayerId::from_index).collect();
    let (mut checked, mut activations, mut walker_views) = (0usize, 0usize, 0usize);
    // A game of random bots may never cast a planeswalker, so a few seeds are played from each case.
    for seed in seed..seed + 2 {
        let setup = with_unique_names(loyalty_fuzz_setup(players, seed).unwrap());
        let mut shown: BTreeMap<PlayerId, std::collections::BTreeSet<ObjectId>> = BTreeMap::new();
        let engine = run_game(setup, seed, |step: &Step| {
            activations += step
                .events
                .iter()
                .filter(|event| matches!(event, mtg_engine::Event::AbilityActivated { .. }))
                .count();
            let batches =
                project_events(step.before, step.events, &viewers, step.from_seq).unwrap();
            for (viewer, batch) in viewers.iter().zip(&batches) {
                let mut hidden = hidden_across(step.before, step.after, *viewer);
                // The seat that looks at a card (Jace's +2) is shown it, and the events say so to that seat alone; it
                // may be named afterwards, when the card is put on the bottom and leaves its sight.
                for event in step.events {
                    if let mtg_engine::Event::LibraryLooked(looked) = event {
                        shown
                            .entry(looked.player)
                            .or_default()
                            .extend(&looked.cards);
                    }
                }
                for card in shown.get(viewer).into_iter().flatten() {
                    hidden.remove(card);
                }
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
                    // What the resolution in progress showed the seat (the card it looked at) is theirs to see.
                    hidden.retain(|id, _| !step.after.is_shown_to(*viewer, *id));
                    let view = StateView::for_player(step.after, *viewer);
                    let found = leaks(&serde_json::to_value(&view).unwrap(), &hidden);
                    assert!(
                        found.is_empty(),
                        "seed {seed} answer {} viewer {viewer:?}: snapshot leaks {found:?}",
                        step.answers
                    );
                    for object in &view.objects {
                        let in_play = object.zone == Zone::Battlefield;
                        assert!(
                            in_play || object.loyalty_abilities.is_empty(),
                            "{} lists loyalty abilities off the battlefield",
                            object.name
                        );
                        if in_play && !object.loyalty_abilities.is_empty() {
                            walker_views += 1;
                            // The loyalty is the counters the permanent holds, whoever looks.
                            let held = step
                                .after
                                .object(object.id)
                                .and_then(|o| o.permanent())
                                .map_or(0, |p| {
                                    p.counters_of(&mtg_cards::value::CounterKind::Loyalty)
                                });
                            let shown = object
                                .counters
                                .iter()
                                .find(|(kind, _)| *kind == mtg_cards::value::CounterKind::Loyalty)
                                .map_or(0, |(_, count)| *count);
                            assert_eq!(shown, held, "seed {seed} {}", object.name);
                        }
                    }
                }
            }
        });
        assert!(engine.result().is_some(), "game should finish");
    }
    assert!(
        checked > 500 && activations > 0 && walker_views > 0,
        "test must not be vacuous: {checked} hidden, {activations} activations, {walker_views} views of planeswalkers"
    );
}

/// Ward, prowess and battle cry trigger on spells and abilities that target and on attackers, and the triggers, the
/// payments of the ward and what the countered spells and abilities were name only things every seat may see.
#[rstest]
#[case(2, 31)]
#[case(3, 32)]
#[case(4, 33)]
fn no_hidden_object_reaches_any_viewer_in_games_of_the_keywords_that_are_triggers(
    #[case] players: usize,
    #[case] seed: u64,
) {
    let viewers: Vec<PlayerId> = (0..players).map(PlayerId::from_index).collect();
    let (mut checked, mut keyword_triggers) = (0usize, 0usize);
    let setup = with_unique_names(keyword_triggers_fuzz_setup(players, seed).unwrap());
    let engine = run_game(setup, seed, |step: &Step| {
        keyword_triggers += step
            .events
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    mtg_engine::Event::AbilityTriggered(trigger)
                        if matches!(
                            trigger.origin,
                            mtg_engine::abilities::AbilitySource::PrintedKeyword(_)
                                | mtg_engine::abilities::AbilitySource::GrantedKeyword(_)
                        )
                )
            })
            .count();
        let batches = project_events(step.before, step.events, &viewers, step.from_seq).unwrap();
        for (viewer, batch) in viewers.iter().zip(&batches) {
            let hidden = hidden_across(step.before, step.after, *viewer);
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
    });
    assert!(engine.result().is_some(), "game should finish");
    assert!(
        checked > 500 && keyword_triggers > 0,
        "test must not be vacuous"
    );
}

/// Abilities that work from a graveyard, a hand and the stack, and state triggers, name only what every seat may see
/// and the card an ability names as its source: a card in a hand or a library that activates an ability or triggers
/// is shown to the table with the ability on the stack, as it is at a real table, and no other hidden card is named.
#[rstest]
#[case(2, 51)]
#[case(3, 55)]
#[case(4, 53)]
fn no_hidden_object_reaches_any_viewer_in_games_of_abilities_from_other_zones(
    #[case] players: usize,
    #[case] seed: u64,
) {
    let viewers: Vec<PlayerId> = (0..players).map(PlayerId::from_index).collect();
    let (mut checked, mut from_hidden_zones, mut state_triggers) = (0usize, 0usize, 0usize);
    let mut shown: BTreeSet<ObjectId> = BTreeSet::new();
    let setup = with_unique_names(zones_fuzz_setup(players, seed).unwrap());
    let engine = run_game(setup, seed, |step: &Step| {
        for event in step.events {
            let source = match event {
                mtg_engine::Event::AbilityTriggered(trigger) => {
                    state_triggers += usize::from(trigger.cause.subjects.is_empty());
                    Some((trigger.source.id, trigger.snapshot.zone))
                }
                mtg_engine::Event::AbilityPutOnStack(put) => {
                    Some((put.source.id, put.snapshot.zone))
                }
                _ => None,
            };
            if let Some((id, zone)) = source {
                from_hidden_zones += usize::from(matches!(zone, Zone::Hand(_) | Zone::Library(_)));
                shown.insert(id);
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
        checked > 500 && state_triggers > 0,
        "test must not be vacuous: {checked} hidden objects checked, {state_triggers} state triggers"
    );
    assert!(
        from_hidden_zones > 0 || players > 2,
        "a card in a hidden zone used an ability"
    );
}

/// The costs that discard a card from a hand (a rummage, a cycling) or exile one from a graveyard reveal it to every
/// seat as it reaches a public zone, the discarded card in the batch of the other seats and in every snapshot, and
/// nothing else of a hand, a library or the cards picked among reaches a seat that may not see it.
#[rstest]
#[case(2, 61)]
#[case(3, 62)]
#[case(4, 63)]
fn a_card_discarded_or_exiled_to_pay_a_cost_reaches_every_seat_and_no_hidden_object_does(
    #[case] players: usize,
    #[case] seed: u64,
) {
    let viewers: Vec<PlayerId> = (0..players).map(PlayerId::from_index).collect();
    let (mut checked, mut discards, mut exiles, mut picks) = (0usize, 0usize, 0usize, 0usize);
    let mut previous: Option<Decision> = None;
    let setup = with_unique_names(parts_fuzz_setup(players, seed).unwrap());
    let engine = run_game(setup, seed, |step: &Step| {
        if let Some(Decision {
            kind: DecisionKind::ChooseCards(options),
            ..
        }) = step.decision
        {
            picks += usize::from(options.can_cancel);
        }
        let batches = project_events(step.before, step.events, &viewers, step.from_seq).unwrap();
        let revealed: Vec<(ObjectId, bool)> = step
            .events
            .iter()
            .filter_map(|event| match event {
                mtg_engine::Event::CardDiscarded { object, .. } => Some((*object, true)),
                mtg_engine::Event::ZoneChanged {
                    object,
                    from: Zone::Graveyard(_),
                    to: Zone::Exile,
                    ..
                } => Some((*object, false)),
                _ => None,
            })
            .collect();
        for (viewer, batch) in viewers.iter().zip(&batches) {
            let mut hidden = hidden_across(step.before, step.after, *viewer);
            for (object, _) in &revealed {
                hidden.remove(object);
            }
            // The seat asked to choose among cards of its library, to search, sees them.
            hidden.retain(|id, _| {
                !offered_to(step.decision, *viewer).contains(id)
                    && !offered_to(previous.as_ref(), *viewer).contains(id)
            });
            checked += hidden.len();
            for (part, value) in batch_parts(batch) {
                let found = leaks(&without_trigger_orders(value), &hidden);
                assert!(
                    found.is_empty(),
                    "seed {seed} answer {} viewer {viewer:?}: batch {part} leaks {found:?}",
                    step.answers
                );
            }
            let view = StateView::for_player(step.after, *viewer);
            for (object, discarded) in &revealed {
                // Whatever zone it is in by the end of the batch, a card in a public zone is in every snapshot.
                if step
                    .after
                    .object(*object)
                    .is_some_and(|o| !is_hidden(o.zone(), *viewer))
                {
                    assert!(
                        view.objects.iter().any(|o| o.id == *object),
                        "seed {seed} answer {} viewer {viewer:?}: {object:?} is missing from the snapshot",
                        step.answers
                    );
                }
                if viewer.index() == 0 {
                    discards += usize::from(*discarded);
                    exiles += usize::from(!*discarded);
                }
            }
        }
        previous = step.decision.cloned();
    });
    assert!(engine.result().is_some(), "game should finish");
    assert!(
        checked > 500 && discards > 0 && picks > 0,
        "test must not be vacuous: {checked} hidden objects, {discards} discards, {exiles} exiles, {picks} picks"
    );
}

/// The order of triggers that go on the stack is a list of positions among them, not of objects.
fn without_trigger_orders(events: Value) -> Value {
    match events {
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .filter(|event| event.pointer("/Public/TriggersOrdered").is_none())
                .collect(),
        ),
        other => other,
    }
}

/// The cards the pending decision puts before `viewer` to choose among or to put in an order.
fn offered_to(decision: Option<&Decision>, viewer: PlayerId) -> Vec<ObjectId> {
    match decision {
        Some(Decision {
            player,
            kind: DecisionKind::ChooseCards(options),
        }) if *player == viewer => options.from.clone(),
        Some(Decision {
            player,
            kind: DecisionKind::OrderCards(options),
        }) if *player == viewer => options.cards.clone(),
        _ => Vec::new(),
    }
}

/// Games of the rules of the game include a Goblin Spy that reveals the top card of its controller's library: every
/// seat's view lists that card and names it as the library's top, the view of the seat that is not shown it as well,
/// and no card below it, in a hand or in another library is named by any batch or snapshot.
#[rstest]
#[case(2, 41)]
#[case(3, 42)]
#[case(4, 43)]
fn a_revealed_library_top_reaches_every_viewer_and_nothing_below_it_does(
    #[case] players: usize,
    #[case] seed: u64,
) {
    let viewers: Vec<PlayerId> = (0..players).map(PlayerId::from_index).collect();
    let (mut checked, mut tops_seen, mut tops_gone) = (0usize, 0usize, 0usize);
    let mut previous: Option<Decision> = None;
    let setup = with_unique_names(rules_fuzz_setup(players, seed).unwrap());
    let engine = run_game(setup, seed, |step: &Step| {
        let batches = project_events(step.before, step.events, &viewers, step.from_seq).unwrap();
        for (viewer, batch) in viewers.iter().zip(&batches) {
            let mut hidden = hidden_across(step.before, step.after, *viewer);
            // The seat asked to choose among cards of its library sees them.
            hidden.retain(|id, _| {
                !offered_to(step.decision, *viewer).contains(id)
                    && !offered_to(previous.as_ref(), *viewer).contains(id)
            });
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
        for viewer in &viewers {
            let view = StateView::for_player(step.after, *viewer);
            let tops = revealed_tops(step.after);
            for player in &view.table.players {
                let top = tops.get(&player.seat).copied();
                assert_eq!(
                    player.revealed_top, top,
                    "seed {seed} answer {} viewer {viewer:?}: the top of {:?}'s library",
                    step.answers, player.seat
                );
                if let Some(top) = top {
                    tops_seen += 1;
                    assert!(
                        view.objects.iter().any(|o| o.id == top),
                        "seed {seed} answer {}: {viewer:?} is not shown the revealed top",
                        step.answers
                    );
                    assert_eq!(
                        player.library.cards, None,
                        "the rest of the library stays hidden"
                    );
                }
            }
            let mut hidden = hidden_in(step.after, *viewer);
            hidden.retain(|id, _| !offered_to(step.decision, *viewer).contains(id));
            let found = leaks(&serde_json::to_value(&view).unwrap(), &hidden);
            assert!(
                found.is_empty(),
                "seed {seed} answer {} viewer {viewer:?}: snapshot leaks {found:?}",
                step.answers
            );
        }
        // A card that stops being the top (drawn, shuffled away) disappears from the views that were shown it.
        for viewer in &viewers {
            let was = revealed_tops(step.before);
            let now = revealed_tops(step.after);
            for (seat, top) in &was {
                if now.get(seat) != Some(top) {
                    tops_gone += 1;
                    let view = StateView::for_player(step.after, *viewer);
                    let seen = view.objects.iter().any(|o| o.id == *top);
                    let still_visible =
                        !is_hidden(step.after.object(*top).unwrap().zone(), *viewer);
                    assert_eq!(seen, still_visible, "seed {seed} answer {}", step.answers);
                }
            }
        }
        previous = step.decision.cloned();
    });
    assert!(engine.result().is_some(), "game should finish");
    assert!(
        checked > 500 && tops_seen > 0 && tops_gone > 0,
        "test must not be vacuous: {checked} hidden objects, {tops_seen} tops seen, {tops_gone} tops gone"
    );
}

/// The choice of cards: the player asked sees every card they choose among (their own hand to discard, the
/// battlefield to sacrifice), no other seat sees a card of that hand, and nothing in the batches that follow names
/// a hidden card, the answer about a hand included.
#[rstest]
#[case(2, 31)]
#[case(3, 32)]
#[case(4, 33)]
fn the_chooser_sees_the_cards_to_choose_among_and_no_other_seat_learns_a_hand(
    #[case] players: usize,
    #[case] seed: u64,
) {
    let viewers: Vec<PlayerId> = (0..players).map(PlayerId::from_index).collect();
    let (mut asked, mut from_hands, mut answers_about_hands) = (0usize, 0usize, 0usize);
    let setup = with_unique_names(archon_fuzz_setup(players, seed).unwrap());
    let engine = run_game(setup, seed, |step: &Step| {
        answers_about_hands += step
            .events
            .iter()
            .filter(|event| matches!(event, mtg_engine::Event::ChoiceMade { choice: Choice::Cards(cards), .. } if cards.iter().any(|c| step.before.object(*c).is_some_and(|o| matches!(o.zone(), Zone::Hand(_))))))
            .count();
        if let Some(Decision {
            player,
            kind: DecisionKind::ChooseCards(options),
        }) = step.decision
        {
            asked += 1;
            for viewer in &viewers {
                let view = StateView::for_player(step.after, *viewer);
                for card in &options.from {
                    let in_hand = step
                        .after
                        .object(*card)
                        .is_some_and(|o| matches!(o.zone(), Zone::Hand(_)));
                    from_hands += usize::from(in_hand);
                    let shown = view.objects.iter().any(|o| o.id == *card);
                    assert_eq!(
                        shown,
                        !in_hand || viewer == player,
                        "seed {seed} viewer {viewer:?}: {card:?} of the choice of {player:?}"
                    );
                }
            }
        }
        let batches = project_events(step.before, step.events, &viewers, step.from_seq).unwrap();
        for (viewer, batch) in viewers.iter().zip(&batches) {
            let mut hidden = hidden_across(step.before, step.after, *viewer);
            // A discarded card is shown to the table as it reaches the graveyard, even when its owner leaves the
            // game later in the same batch.
            for event in step.events {
                if let mtg_engine::Event::CardDiscarded { object, .. } = event {
                    hidden.remove(object);
                }
            }
            for (part, value) in batch_parts(batch) {
                let found = leaks(&without_trigger_orders(value), &hidden);
                assert!(
                    found.is_empty(),
                    "seed {seed} answer {} viewer {viewer:?}: batch {part} leaks {found:?}",
                    step.answers
                );
            }
        }
    });
    assert!(engine.result().is_some(), "game should finish");
    assert!(
        asked > 0 && from_hands > 0 && answers_about_hands > 0,
        "test must not be vacuous: {asked} questions, {from_hands} cards from hands, {answers_about_hands} answers about hands"
    );
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

/// Seat 0's library, top first, in the tables below.
const LIBRARY: [&str; 6] = [
    "Grizzly Bears",
    "Serra Angel",
    "Lightning Bolt",
    "Giant Growth",
    "Counterspell",
    "Wrath of God",
];

/// Seat 0 holds `spell` and then the cards named in `hand`, controls an Island, and every seat has the library above.
fn library_table(
    seats: usize,
    spell: std::sync::Arc<mtg_cards::CardDefinition>,
    hand: &[&str],
) -> mtg_testkit::Scenario {
    let mut cards = vec![spell];
    cards.extend(
        hand.iter()
            .map(|name| mtg_testkit::corpus::card(name).unwrap()),
    );
    let mut scenario = mtg_testkit::Scenario::new(seats)
        .hand(0, cards)
        .battlefield(0, [mtg_testkit::ready(mtg_testkit::cards::island())]);
    for seat in 0..seats {
        scenario = scenario.library_named(seat, LIBRARY);
    }
    scenario
}

/// Casts `spell` from seat 0 and plays the resolution out, `answer` giving each question; returns the state at the end,
/// the log, and the whole log projected for each seat from the start.
fn play_spell(
    scenario: mtg_testkit::Scenario,
    seats: usize,
    spell: &str,
    answer: impl Fn(&Decision) -> mtg_engine::decision::Answer,
) -> (GameState, Vec<mtg_engine::Event>, Vec<EventBatch>) {
    let before = GameState::from_setup(&scenario.clone().setup().unwrap()).unwrap();
    let mut driver = mtg_testkit::Driver::from_scenario(scenario).unwrap();
    driver.cast(spell).unwrap();
    for _ in 0..100 {
        let decision = driver.decision().unwrap().clone();
        match decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::Priority(_) => driver.pass().map(|_| ()).unwrap(),
            _ => driver.answer(answer(&decision)).map(|_| ()).unwrap(),
        }
    }
    let viewers: Vec<PlayerId> = (0..seats).map(PlayerId::from_index).collect();
    let events = driver.engine().events().to_vec();
    let batches = project_events(&before, &events, &viewers, 0).unwrap();
    (driver.state().clone(), events, batches)
}

/// The batch has none of the hidden cards in any part.
pub(crate) fn assert_clean(batch: &EventBatch, hidden: &BTreeMap<ObjectId, String>) {
    for (part, value) in batch_parts(batch) {
        assert!(
            leaks(&value, hidden).is_empty(),
            "{part} leaks {:?}",
            leaks(&value, hidden)
        );
    }
}

#[rstest]
fn the_seat_that_scries_sees_the_cards_and_no_other_seat_does(#[values(2, 3, 4)] seats: usize) {
    let scenario = library_table(seats, mtg_testkit::corpus::card("Opt").unwrap(), &[]);
    let keep = |_: &Decision| {
        mtg_engine::decision::Answer::ChooseCards(mtg_engine::choose::CardsChoice(Vec::new()))
    };
    let (after, events, batches) = play_spell(scenario, seats, "Opt", keep);
    let scried = events
        .iter()
        .find_map(|event| match event {
            mtg_engine::Event::Scried(scried) => Some(scried.clone()),
            _ => None,
        })
        .expect("Opt scried");
    let top = scried.top[0];
    let hidden = BTreeMap::from([(top, after.object(top).unwrap().card().name.clone())]);
    assert!(
        batches[0].events.iter().any(|event| matches!(
            event,
            mtg_protocol::EventView::Public(mtg_engine::Event::Scried(s)) if *s == scried
        )),
        "the seat that scried is told which card it looked at"
    );
    for batch in &batches[1..] {
        assert!(
            batch.events.iter().any(|event| matches!(
                event,
                mtg_protocol::EventView::Hidden(mtg_protocol::HiddenEvent::Scried {
                    player: PlayerId(0),
                    top: 1,
                    bottom: 0
                })
            )),
            "the others are told that one card stayed on top"
        );
        assert!(!batch.events.iter().any(|event| matches!(
            event,
            mtg_protocol::EventView::Public(mtg_engine::Event::Scried(_))
        )));
        assert_clean(batch, &hidden);
    }
}

#[rstest]
fn the_seat_that_surveils_sees_the_cards_and_the_others_see_only_what_goes_into_the_graveyard(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] graveyard: bool,
) {
    let scenario = library_table(seats, mtg_testkit::corpus::card("Consider").unwrap(), &[]);
    let answer = move |decision: &Decision| match &decision.kind {
        DecisionKind::ChooseCards(options) if graveyard => {
            mtg_engine::decision::Answer::ChooseCards(mtg_engine::choose::CardsChoice(
                options.from.clone(),
            ))
        }
        DecisionKind::ChooseCards(_) => {
            mtg_engine::decision::Answer::ChooseCards(mtg_engine::choose::CardsChoice(Vec::new()))
        }
        other => panic!("unexpected {}", other.name()),
    };
    let (after, events, batches) = play_spell(scenario, seats, "Consider", answer);
    let surveilled = events
        .iter()
        .find_map(|event| match event {
            mtg_engine::Event::Surveilled(surveilled) => Some(surveilled.clone()),
            _ => None,
        })
        .expect("Consider surveilled");
    assert!(
        batches[0].events.iter().any(|event| matches!(
            event,
            mtg_protocol::EventView::Public(mtg_engine::Event::Surveilled(s)) if *s == surveilled
        )),
        "the seat that surveilled is told which card it looked at"
    );
    let looked = surveilled
        .top
        .iter()
        .chain(&surveilled.graveyard)
        .copied()
        .next()
        .expect("one card was looked at");
    let name = after.object(looked).unwrap().card().name.clone();
    for batch in &batches[1..] {
        assert!(
            batch.events.iter().any(|event| matches!(
                event,
                mtg_protocol::EventView::Hidden(mtg_protocol::HiddenEvent::Surveilled {
                    player: PlayerId(0),
                    top,
                    graveyard: sent
                }) if *top == u32::from(!graveyard) && *sent == u32::from(graveyard)
            )),
            "the others are told how many cards stayed and how many went"
        );
        assert!(!batch.events.iter().any(|event| matches!(
            event,
            mtg_protocol::EventView::Public(mtg_engine::Event::Surveilled(_))
        )));
        if graveyard {
            // A graveyard is open: the move names the card where it lands.
            assert!(
                batch.events.iter().any(|event| matches!(
                    event,
                    mtg_protocol::EventView::Public(mtg_engine::Event::ZoneChanged {
                        object,
                        to: Zone::Graveyard(PlayerId(0)),
                        ..
                    }) if *object == looked
                )),
                "the card put into the graveyard is named to every seat"
            );
        } else {
            assert_clean(batch, &BTreeMap::from([(looked, name.clone())]));
        }
    }
}

#[rstest]
fn a_proliferate_and_the_counters_it_gives_are_public_and_its_choices_name_only_permanents_on_the_battlefield(
    #[values(2, 3, 4)] seats: usize,
) {
    let wolf = mtg_testkit::with_counters(
        mtg_testkit::ready(mtg_testkit::cards::free_creature("Wolf", 2, 2)),
        mtg_cards::value::CounterKind::PlusOnePlusOne,
        1,
    );
    let mut scenario = mtg_testkit::Scenario::new(seats)
        .hand(0, [mtg_testkit::corpus::card("Contentious Plan").unwrap()])
        .battlefield(
            0,
            [
                mtg_testkit::ready(mtg_testkit::cards::island()),
                mtg_testkit::ready(mtg_testkit::cards::island()),
                wolf,
            ],
        );
    for seat in 0..seats {
        scenario = scenario.library_named(seat, LIBRARY);
    }
    let asked = std::cell::RefCell::new(Vec::new());
    let answer = |decision: &Decision| match &decision.kind {
        DecisionKind::ChooseCards(options) => mtg_engine::decision::Answer::ChooseCards(
            mtg_engine::choose::CardsChoice(options.from.clone()),
        ),
        other => panic!("unexpected {}", other.name()),
    };
    let (after, events, batches) = play_spell(scenario, seats, "Contentious Plan", |decision| {
        asked.borrow_mut().push(decision.player);
        answer(decision)
    });
    assert_eq!(
        asked.into_inner(),
        [PlayerId(0)],
        "only the proliferating seat is asked"
    );
    let wolf = after
        .objects()
        .find(|o| o.card().name == "Wolf")
        .map(GameObject::id)
        .unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        mtg_engine::Event::Proliferated {
            player: PlayerId(0)
        }
    )));
    let drawn = events
        .iter()
        .find_map(|event| match event {
            mtg_engine::Event::CardDrawn { object, .. } => Some(*object),
            _ => None,
        })
        .expect("Contentious Plan draws");
    let name = after.object(drawn).unwrap().card().name.clone();
    for (seat, batch) in batches.iter().enumerate() {
        assert!(
            batch.events.iter().any(|event| matches!(
                event,
                mtg_protocol::EventView::Public(mtg_engine::Event::Proliferated {
                    player: PlayerId(0)
                })
            )),
            "seat {seat} is told who proliferated"
        );
        assert!(
            batch.events.iter().any(|event| matches!(
                event,
                mtg_protocol::EventView::Public(mtg_engine::Event::CountersChanged(changed))
                    if changed.on.object().is_some_and(|o| o.id == wolf) && changed.delta == 1
            )),
            "seat {seat} sees the counter the Wolf got"
        );
        if seat != 0 {
            assert_clean(batch, &BTreeMap::from([(drawn, name.clone())]));
        }
    }
}

#[rstest]
fn an_explore_reveals_its_card_to_every_seat_asks_only_its_controller_and_shows_nothing_below_it(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] land: bool,
    #[values(true, false)] graveyard: bool,
) {
    let mut cards: Vec<_> = LIBRARY
        .iter()
        .map(|name| mtg_testkit::corpus::card(name).unwrap())
        .collect();
    if land {
        cards.insert(0, mtg_testkit::cards::island());
    }
    let below = cards[1].name.clone();
    let mut scenario = mtg_testkit::Scenario::new(seats)
        .hand(0, [mtg_testkit::corpus::card("Siren Lookout").unwrap()])
        .battlefield(0, vec![mtg_testkit::ready(mtg_testkit::cards::island()); 3]);
    for seat in 0..seats {
        scenario = scenario.library(seat, cards.clone());
    }
    let asked = std::cell::RefCell::new(Vec::new());
    let answer = |decision: &Decision| match &decision.kind {
        DecisionKind::ChooseCards(options) => {
            asked
                .borrow_mut()
                .push((decision.player, options.from.clone()));
            mtg_engine::decision::Answer::ChooseCards(mtg_engine::choose::CardsChoice(
                if graveyard {
                    options.from.clone()
                } else {
                    Vec::new()
                },
            ))
        }
        other => panic!("unexpected {}", other.name()),
    };
    let (after, events, batches) = play_spell(scenario, seats, "Siren Lookout", answer);
    let revealed = events
        .iter()
        .find_map(|event| match event {
            mtg_engine::Event::Revealed(revealed) => Some(revealed.clone()),
            _ => None,
        })
        .expect("the explore reveals a card");
    assert_eq!(revealed.player, PlayerId(0));
    let asked = asked.into_inner();
    if land {
        assert!(asked.is_empty(), "a land is not offered to the graveyard");
    } else {
        assert_eq!(asked, [(PlayerId(0), revealed.cards.clone())]);
    }
    let second = after
        .objects()
        .find(|o| o.card().name == below && o.owner() == PlayerId(0))
        .map(GameObject::id)
        .unwrap();
    for (seat, batch) in batches.iter().enumerate() {
        assert!(
            batch.events.iter().any(|event| matches!(
                event,
                mtg_protocol::EventView::Public(mtg_engine::Event::Explored {
                    player: PlayerId(0),
                    ..
                })
            )),
            "seat {seat} is told who explored"
        );
        assert!(
            batch.events.iter().any(|event| matches!(
                event,
                mtg_protocol::EventView::Public(mtg_engine::Event::Revealed(r)) if *r == revealed
            )),
            "seat {seat} sees the revealed card"
        );
        assert_clean(batch, &BTreeMap::from([(second, below.clone())]));
    }
}

#[rstest]
fn a_connive_asks_only_its_controller_and_the_others_do_not_see_the_card_drawn_unless_it_is_discarded(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] discard_the_drawn_card: bool,
) {
    let mut scenario = mtg_testkit::Scenario::new(seats)
        .hand(
            0,
            [
                mtg_testkit::corpus::card("Echo Inspector").unwrap(),
                mtg_testkit::corpus::card("Giant Growth").unwrap(),
            ],
        )
        .battlefield(0, vec![mtg_testkit::ready(mtg_testkit::cards::island()); 4]);
    for seat in 0..seats {
        scenario = scenario.library_named(seat, LIBRARY);
    }
    let asked = std::cell::RefCell::new(Vec::new());
    let answer = |decision: &Decision| match &decision.kind {
        DecisionKind::ChooseCards(options) => {
            asked.borrow_mut().push(decision.player);
            let pick = usize::from(discard_the_drawn_card);
            mtg_engine::decision::Answer::ChooseCards(mtg_engine::choose::CardsChoice(vec![
                options.from[pick],
            ]))
        }
        other => panic!("unexpected {}", other.name()),
    };
    let (after, events, batches) = play_spell(scenario, seats, "Echo Inspector", answer);
    assert_eq!(
        asked.into_inner(),
        [PlayerId(0)],
        "only the controller is asked"
    );
    let drawn = events
        .iter()
        .find_map(|event| match event {
            mtg_engine::Event::CardDrawn { object, .. } => Some(*object),
            _ => None,
        })
        .expect("the connive draws");
    let name = after.object(drawn).unwrap().card().name.clone();
    for (seat, batch) in batches.iter().enumerate() {
        assert!(
            batch.events.iter().any(|event| matches!(
                event,
                mtg_protocol::EventView::Public(mtg_engine::Event::Connived {
                    player: PlayerId(0),
                    ..
                })
            )),
            "seat {seat} is told who connived"
        );
        if seat != 0 && !discard_the_drawn_card {
            assert_clean(batch, &BTreeMap::from([(drawn, name.clone())]));
        }
    }
}

#[rstest]
fn the_seat_that_looks_at_libraries_sees_the_cards_and_no_other_seat_does_not_even_the_owner(
    #[values(2, 3, 4)] seats: usize,
) {
    let scenario = library_table(seats, mtg_testkit::cards::bottomer("Sink"), &[]);
    let reversed = |decision: &Decision| match &decision.kind {
        DecisionKind::OrderCards(options) => mtg_engine::decision::Answer::OrderCards(
            mtg_engine::choose::OrderChoice(options.cards.iter().rev().copied().collect()),
        ),
        other => panic!("unexpected {}", other.name()),
    };
    let (after, events, batches) = play_spell(scenario, seats, "Sink", reversed);
    // The two top cards of each opponent's library are what was looked at.
    let mut hidden = BTreeMap::new();
    let looked: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            mtg_engine::Event::LibraryLooked(looked) => Some(looked.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(looked.len(), seats - 1);
    for each in &looked {
        assert_eq!((each.player, each.cards.len()), (PlayerId(0), 2));
        for card in &each.cards {
            hidden.insert(*card, after.object(*card).unwrap().card().name.clone());
        }
    }
    for (seat, batch) in batches.iter().enumerate() {
        let count = |wanted: fn(&mtg_protocol::EventView) -> bool| {
            batch.events.iter().filter(|event| wanted(event)).count()
        };
        let public = count(|event| {
            matches!(
                event,
                mtg_protocol::EventView::Public(mtg_engine::Event::LibraryLooked(_))
            )
        });
        let counted = count(|event| {
            matches!(
                event,
                mtg_protocol::EventView::Hidden(mtg_protocol::HiddenEvent::LibraryLooked {
                    player: PlayerId(0),
                    count: 2
                })
            )
        });
        if seat == 0 {
            assert_eq!(
                (public, counted),
                (seats - 1, 0),
                "the looker sees every card"
            );
        } else {
            assert_eq!(
                (public, counted),
                (0, seats - 1),
                "seat {seat} learns only how many"
            );
            assert_clean(batch, &hidden);
        }
        // The order the cards were put in is told to the one who chose it and to nobody else.
        let ordered = count(|event| {
            matches!(
                event,
                mtg_protocol::EventView::Public(mtg_engine::Event::ChoiceMade {
                    choice: mtg_engine::choose::Choice::Order(_),
                    ..
                })
            )
        });
        assert_eq!(
            ordered,
            if seat == 0 { seats - 1 } else { 0 },
            "seat {seat}"
        );
        let bottomed = count(|event| {
            matches!(
                event,
                mtg_protocol::EventView::Public(mtg_engine::Event::LibraryBottomed { .. })
            )
        });
        assert_eq!(bottomed, 0, "no seat is told which cards went down");
    }
}

#[rstest]
fn a_reveal_shows_the_cards_to_every_seat(#[values(2, 3, 4)] seats: usize) {
    let scenario = library_table(
        seats,
        mtg_testkit::cards::revealer("Show"),
        &["Lightning Bolt", "Giant Growth"],
    );
    let (after, events, batches) = play_spell(scenario, seats, "Show", |decision| {
        panic!("a reveal asks nothing, not {}", decision.kind.name())
    });
    let revealed = events
        .iter()
        .find_map(|event| match event {
            mtg_engine::Event::Revealed(revealed) => Some(revealed.clone()),
            _ => None,
        })
        .expect("the hand was revealed");
    assert_eq!(revealed.cards.len(), 2);
    for (seat, batch) in batches.iter().enumerate() {
        assert!(
            batch.events.iter().any(|event| matches!(
                event,
                mtg_protocol::EventView::Public(mtg_engine::Event::Revealed(r)) if *r == revealed
            )),
            "seat {seat} sees what was revealed"
        );
    }
    // The card looked at afterwards is the controller's alone.
    let top = events
        .iter()
        .find_map(|event| match event {
            mtg_engine::Event::LibraryLooked(looked) => Some(looked.cards[0]),
            _ => None,
        })
        .expect("the top card was looked at");
    let hidden = BTreeMap::from([(top, after.object(top).unwrap().card().name.clone())]);
    for batch in &batches[1..] {
        assert_clean(batch, &hidden);
    }
}

/// Seat 0 holds `spell` and the Forests that pay for it; its library is `library` (top first), the others' are the same.
fn search_table(seats: usize, spell: &str, library: &[&str]) -> mtg_testkit::Scenario {
    let pick = |name: &str| match name {
        "Plains" => mtg_testkit::cards::plains(),
        "Swamp" => mtg_testkit::cards::swamp(),
        "Mountain" => mtg_testkit::cards::mountain(),
        other => mtg_testkit::corpus::card(other).unwrap(),
    };
    let forests = if spell == "Cultivate" { 3 } else { 2 };
    let mut scenario = mtg_testkit::Scenario::new(seats)
        .hand(0, [mtg_testkit::corpus::card(spell).unwrap()])
        .battlefield(
            0,
            (0..forests).map(|_| mtg_testkit::ready(mtg_testkit::cards::forest())),
        );
    for seat in 0..seats {
        scenario = scenario.library(seat, library.iter().map(|name| pick(name)));
    }
    scenario
}

/// Casts the spell of seat 0 and passes until a decision other than priority waits. Returns the driver and each seat's
/// batch of everything since the cast.
fn cast_until_asked(
    scenario: mtg_testkit::Scenario,
    seats: usize,
    spell: &str,
) -> (mtg_testkit::Driver, Vec<EventBatch>) {
    let mut driver = mtg_testkit::Driver::from_scenario(scenario).unwrap();
    let (before, from) = (driver.state().clone(), driver.engine().events().len());
    driver.cast(spell).unwrap();
    while matches!(driver.decision().unwrap().kind, DecisionKind::Priority(_))
        && !driver.state().stack().is_empty()
    {
        driver.pass().unwrap();
    }
    let viewers: Vec<PlayerId> = (0..seats).map(PlayerId::from_index).collect();
    let events = driver.engine().events()[from..].to_vec();
    let batches = project_events(&before, &events, &viewers, from as u64).unwrap();
    (driver, batches)
}

/// Whether the view of `seat` lists each of `cards`.
fn listed(driver: &mtg_testkit::Driver, seat: usize, cards: &[ObjectId]) -> Vec<bool> {
    let view = StateView::for_player(driver.state(), PlayerId::from_index(seat));
    cards
        .iter()
        .map(|card| view.objects.iter().any(|o| o.id == *card))
        .collect()
}

fn name(driver: &mtg_testkit::Driver, card: ObjectId) -> String {
    driver.state().object(card).unwrap().card().name.clone()
}

/// The ids and names of `cards`, for the scan of a batch.
fn named_hidden(driver: &mtg_testkit::Driver, cards: &[ObjectId]) -> BTreeMap<ObjectId, String> {
    cards.iter().map(|c| (*c, name(driver, *c))).collect()
}

/// Seat 0's library from the top down: three basic lands that are not Forests, so no name is also in play.
const SEARCHED: [&str; 5] = [
    "Grizzly Bears",
    "Plains",
    "Serra Angel",
    "Swamp",
    "Mountain",
];

fn choose_answer(cards: &[ObjectId]) -> mtg_engine::decision::Answer {
    mtg_engine::decision::Answer::ChooseCards(mtg_engine::choose::CardsChoice(cards.to_vec()))
}

fn batches_of(
    before: &GameState,
    driver: &mtg_testkit::Driver,
    from: usize,
    seats: usize,
) -> Vec<EventBatch> {
    let viewers: Vec<PlayerId> = (0..seats).map(PlayerId::from_index).collect();
    project_events(
        before,
        &driver.engine().events()[from..],
        &viewers,
        from as u64,
    )
    .unwrap()
}

#[rstest]
fn the_seat_that_searches_sees_the_cards_it_chooses_among_and_no_other_seat_learns_what_the_library_holds(
    #[values(2, 3, 4)] seats: usize,
) {
    let scenario = search_table(seats, "Rampant Growth", &SEARCHED);
    let (mut driver, batches) = cast_until_asked(scenario, seats, "Rampant Growth");
    let Some(Decision {
        player,
        kind: DecisionKind::ChooseCards(options),
    }) = driver.decision().ok().cloned()
    else {
        panic!("the search asks for cards");
    };
    assert_eq!(player, PlayerId(0));
    assert_eq!(options.from.len(), 3, "the three basic lands");
    let library: Vec<ObjectId> = driver.state().zone(Zone::Library(PlayerId(0))).to_vec();
    for seat in 0..seats {
        let expected: Vec<bool> = library
            .iter()
            .map(|card| seat == 0 && options.from.contains(card))
            .collect();
        assert_eq!(
            listed(&driver, seat, &library),
            expected,
            "seat {seat} lists exactly the cards it is choosing among"
        );
    }
    // The batch that ended in the question says the same, and nothing else reaches the others.
    let in_library = |batch: &EventBatch| {
        batch
            .objects
            .iter()
            .filter(|o| o.zone == Zone::Library(PlayerId(0)))
            .count()
    };
    assert_eq!(
        in_library(&batches[0]),
        3,
        "the searcher's batch lists the candidates"
    );
    let hidden = named_hidden(&driver, &library);
    for batch in &batches[1..] {
        assert_eq!(in_library(batch), 0);
        assert_clean(batch, &hidden);
    }
    // Choosing one: it enters the battlefield for everyone and the others learn nothing of the rest.
    let swamp = *options
        .from
        .iter()
        .find(|c| name(&driver, **c) == "Swamp")
        .unwrap();
    let (before, from) = (driver.state().clone(), driver.engine().events().len());
    driver.answer(choose_answer(&[swamp])).unwrap();
    let after = batches_of(&before, &driver, from, seats);
    let rest: Vec<ObjectId> = library.iter().copied().filter(|c| *c != swamp).collect();
    let hidden = named_hidden(&driver, &rest);
    for (seat, batch) in after.iter().enumerate() {
        let found = batch.events.iter().find_map(|event| match event {
            mtg_protocol::EventView::Public(mtg_engine::Event::CardsFound(f)) => {
                Some(f.cards.clone())
            }
            _ => None,
        });
        let counted = batch.events.iter().any(|event| {
            matches!(
                event,
                mtg_protocol::EventView::Hidden(mtg_protocol::HiddenEvent::CardsFound {
                    player: PlayerId(0),
                    count: 1
                })
            )
        });
        if seat == 0 {
            assert_eq!(
                found,
                Some(vec![swamp]),
                "the searcher is told what it found"
            );
        } else {
            assert!(
                found.is_none() && counted,
                "seat {seat} learns that a card was found"
            );
            assert_clean(batch, &hidden);
        }
        assert!(
            !batch.events.iter().any(|event| matches!(
                event,
                mtg_protocol::EventView::Public(mtg_engine::Event::Revealed(_))
            )),
            "Rampant Growth reveals nothing"
        );
        assert!(
            batch.objects.iter().any(|o| o.id == swamp),
            "seat {seat} sees the Swamp enter"
        );
    }
    for seat in 0..seats {
        let expected: Vec<bool> = library.iter().map(|card| *card == swamp).collect();
        assert_eq!(
            listed(&driver, seat, &library),
            expected,
            "after the search only the land that entered is listed"
        );
    }
}

#[rstest]
fn cultivate_shows_every_seat_the_lands_it_reveals_while_they_matter_and_not_the_rest(
    #[values(2, 3, 4)] seats: usize,
) {
    let scenario = search_table(seats, "Cultivate", &SEARCHED);
    let (mut driver, _) = cast_until_asked(scenario, seats, "Cultivate");
    let Some(DecisionKind::ChooseCards(options)) = driver.decision().ok().map(|d| d.kind.clone())
    else {
        panic!("the search asks for cards");
    };
    let find = |driver: &mtg_testkit::Driver, wanted: &str| {
        *options
            .from
            .iter()
            .find(|card| name(driver, **card) == wanted)
            .unwrap()
    };
    let (plains, mountain) = (find(&driver, "Plains"), find(&driver, "Mountain"));
    let (before, from) = (driver.state().clone(), driver.engine().events().len());
    driver.answer(choose_answer(&[plains, mountain])).unwrap();
    // Which of the two enters is asked now; both were revealed.
    let Some(DecisionKind::ChooseCards(second)) = driver.decision().ok().map(|d| d.kind.clone())
    else {
        panic!("the player chooses which land enters");
    };
    assert_eq!(second.from.len(), 2);
    let library: Vec<ObjectId> = driver.state().zone(Zone::Library(PlayerId(0))).to_vec();
    let revealed = vec![plains, mountain];
    let batches = batches_of(&before, &driver, from, seats);
    for (seat, batch) in batches.iter().enumerate() {
        for (card, shown) in library.iter().zip(listed(&driver, seat, &library)) {
            assert_eq!(
                shown,
                revealed.contains(card),
                "seat {seat}, {}",
                name(&driver, *card)
            );
        }
        assert!(
            batch.events.iter().any(|event| matches!(
                event,
                mtg_protocol::EventView::Public(mtg_engine::Event::Revealed(r)) if r.cards == revealed
            )),
            "seat {seat} sees what was revealed"
        );
    }
    let rest: Vec<ObjectId> = library
        .iter()
        .copied()
        .filter(|c| !revealed.contains(c))
        .collect();
    let hidden = named_hidden(&driver, &rest);
    for batch in &batches[1..] {
        assert_clean(batch, &hidden);
    }
    // One enters the battlefield and the other goes to the hand: the others now see only the one in play.
    let (before, from) = (driver.state().clone(), driver.engine().events().len());
    driver.answer(choose_answer(&[mountain])).unwrap();
    let after = batches_of(&before, &driver, from, seats);
    for (seat, batch) in after.iter().enumerate() {
        let in_view = |card: ObjectId| batch.objects.iter().any(|o| o.id == card);
        assert!(in_view(mountain), "seat {seat} sees the Mountain enter");
        assert_eq!(
            in_view(plains),
            seat == 0,
            "only its owner sees the Plains in hand"
        );
        if seat != 0 {
            assert!(
                batch.gone.contains(&plains),
                "the others lose sight of the Plains"
            );
        }
    }
    for seat in 1..seats {
        assert_eq!(listed(&driver, seat, &[plains, mountain]), [false, true]);
    }
}

#[rstest]
fn the_seat_that_looked_at_libraries_lists_the_cards_while_it_orders_them_and_their_owners_do_not(
    #[values(2, 3, 4)] seats: usize,
) {
    let scenario = library_table(seats, mtg_testkit::cards::bottomer("Sink"), &[]);
    let (driver, batches) = cast_until_asked(scenario, seats, "Sink");
    let Some(DecisionKind::OrderCards(options)) = driver.decision().ok().map(|d| d.kind.clone())
    else {
        panic!("the cards are put in order");
    };
    // The cards of one library are ordered at a time; every library was looked at before the first question.
    assert_eq!(options.cards.len(), 2);
    let looked: Vec<ObjectId> = driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            mtg_engine::Event::LibraryLooked(looked) => Some(looked.cards.clone()),
            _ => None,
        })
        .flatten()
        .collect();
    assert_eq!(looked.len(), 2 * (seats - 1));
    for seat in 0..seats {
        assert_eq!(
            listed(&driver, seat, &looked),
            vec![seat == 0; looked.len()],
            "seat {seat}: no owner sees the cards of their library that another looked at"
        );
    }
    let hidden = named_hidden(&driver, &looked);
    for batch in &batches[1..] {
        assert_clean(batch, &hidden);
    }
}

/// Attachments and the legend rule are public: the events that attach and unattach, the cards the rule puts into a graveyard
/// and the question it asks name only things every seat may see; an Aura on the battlefield shows its host to every viewer.
#[rstest]
#[case(2, 81)]
#[case(3, 82)]
#[case(4, 83)]
fn no_hidden_object_reaches_any_viewer_in_games_of_auras_and_legends(
    #[case] players: usize,
    #[case] seed: u64,
) {
    let viewers: Vec<PlayerId> = (0..players).map(PlayerId::from_index).collect();
    let (mut checked, mut attach_events, mut attached_views, mut questions) = (0usize, 0, 0, 0);
    for seed in seed..seed + 2 {
        let setup = with_unique_names(attach_fuzz_setup(players, seed).unwrap());
        let engine = run_game(setup, seed, |step: &Step| {
            let logged = step
                .events
                .iter()
                .filter(|event| {
                    matches!(
                        event,
                        mtg_engine::Event::Attached { .. } | mtg_engine::Event::Unattached { .. }
                    )
                })
                .count();
            attach_events += logged;
            let batches =
                project_events(step.before, step.events, &viewers, step.from_seq).unwrap();
            for (viewer, batch) in viewers.iter().zip(&batches) {
                let hidden = hidden_across(step.before, step.after, *viewer);
                checked += hidden.len();
                for (part, value) in batch_parts(batch) {
                    let found = leaks(&without_trigger_orders(value), &hidden);
                    assert!(
                        found.is_empty(),
                        "seed {seed} answer {} viewer {viewer:?}: batch {part} leaks {found:?}",
                        step.answers
                    );
                }
                // Every attach and unattach reaches every viewer whole.
                let seen = batch
                    .events
                    .iter()
                    .filter(|view| {
                        matches!(
                            view,
                            mtg_protocol::EventView::Public(
                                mtg_engine::Event::Attached { .. }
                                    | mtg_engine::Event::Unattached { .. }
                            )
                        )
                    })
                    .count();
                assert_eq!(seen, logged, "seed {seed} viewer {viewer:?}");
            }
            if let Some(decision) = step.decision
                && let DecisionKind::ChooseCards(options) = &decision.kind
                && step.after.resolving().is_none()
            {
                // The legend rule names permanents on the battlefield: every seat may see them.
                questions += 1;
                for id in &options.from {
                    assert_eq!(
                        step.after.object(*id).map(GameObject::zone),
                        Some(Zone::Battlefield)
                    );
                }
            }
            if step.answers.is_multiple_of(SNAPSHOT_EVERY) {
                for viewer in &viewers {
                    let hidden = hidden_in(step.after, *viewer);
                    let view = StateView::for_player(step.after, *viewer);
                    let found = leaks(&serde_json::to_value(&view).unwrap(), &hidden);
                    assert!(
                        found.is_empty(),
                        "seed {seed} answer {} viewer {viewer:?}: snapshot leaks {found:?}",
                        step.answers
                    );
                    for object in &view.objects {
                        let host = step.after.host_of(object.id);
                        assert_eq!(object.attached_to, host, "seed {seed} {}", object.name);
                        attached_views += usize::from(host.is_some());
                    }
                }
            }
        });
        assert!(engine.result().is_some(), "game should finish");
    }
    assert!(
        checked > 500 && attach_events > 0 && attached_views > 0,
        "test must not be vacuous: {checked} hidden, {attach_events} attach events, {attached_views} views with a host, {questions} legend questions"
    );
}

#[rstest]
#[case(2, 311)]
#[case(3, 308)]
#[case(4, 320)]
fn no_hidden_object_reaches_any_viewer_in_games_of_prevention_and_regeneration(
    #[case] players: usize,
    #[case] seed: u64,
) {
    let viewers: Vec<PlayerId> = (0..players).map(PlayerId::from_index).collect();
    let (mut checked, mut regenerated, mut shields_used) = (0usize, 0usize, 0usize);
    for seed in seed..seed + PREVENTION_GAMES {
        let setup = with_unique_names(prevent_fuzz_setup(players, seed).unwrap());
        let engine = run_game(setup, seed, |step: &Step| {
            // The only option the prevention decks ask for is the damage a shield takes first.
            if let Some(decision) = step.decision
                && matches!(decision.kind, DecisionKind::ChooseOption(_))
            {
                let hidden = hidden_in(step.after, decision.player);
                let found = leaks(&serde_json::to_value(decision).unwrap(), &hidden);
                assert!(
                    found.is_empty(),
                    "seed {seed} answer {}: the question leaks {found:?}",
                    step.answers
                );
            }
            let count = |wanted: fn(&mtg_engine::Event) -> bool| {
                step.events.iter().filter(|event| wanted(event)).count()
            };
            let regenerations =
                count(|event| matches!(event, mtg_engine::Event::Regenerated { .. }));
            let uses = count(|event| matches!(event, mtg_engine::Event::ShieldUsed { .. }));
            regenerated += regenerations;
            shields_used += uses;
            let batches =
                project_events(step.before, step.events, &viewers, step.from_seq).unwrap();
            for (viewer, batch) in viewers.iter().zip(&batches) {
                let hidden = hidden_across(step.before, step.after, *viewer);
                checked += hidden.len();
                for (part, value) in batch_parts(batch) {
                    let found = leaks(&without_trigger_orders(value), &hidden);
                    assert!(
                        found.is_empty(),
                        "seed {seed} answer {} viewer {viewer:?}: batch {part} leaks {found:?}",
                        step.answers
                    );
                }
                // A regeneration and the amount a shield kept are public: every viewer gets each one whole.
                let public = |wanted: fn(&mtg_engine::Event) -> bool| {
                    batch
                        .events
                        .iter()
                        .filter(|view| {
                            matches!(view, mtg_protocol::EventView::Public(event) if wanted(event))
                        })
                        .count()
                };
                assert_eq!(
                    public(|event| matches!(event, mtg_engine::Event::Regenerated { .. })),
                    regenerations,
                    "seed {seed} viewer {viewer:?}"
                );
                assert_eq!(
                    public(|event| matches!(event, mtg_engine::Event::ShieldUsed { .. })),
                    uses,
                    "seed {seed} viewer {viewer:?}"
                );
            }
            if step.answers.is_multiple_of(SNAPSHOT_EVERY) {
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
    }
    assert!(
        checked > 500 && regenerated > 0,
        "test must not be vacuous: {checked} hidden, {regenerated} regenerations, {shields_used} shields that kept damage"
    );
}

#[rstest]
fn a_shield_that_kept_damage_and_a_regeneration_reach_every_seat_whole_and_nothing_hidden_with_them(
    #[values(2, 3, 4)] seats: usize,
) {
    use mtg_engine::decision::{Answer, PayChoice};
    use mtg_testkit::cards::{burn, mountain, plains, swamp};
    use mtg_testkit::{Driver, Scenario, ready};
    let mut scenario = Scenario::new(seats)
        .hand_named(0, ["Mending Hands"])
        .battlefield(
            0,
            [
                ready(plains()),
                ready(swamp()),
                ready(mtg_testkit::corpus::card("Drudge Skeletons").unwrap()),
            ],
        )
        .hand(1, vec![burn("Spark", 1), burn("Bolt", 3)])
        .battlefield(1, [ready(mountain()), ready(mountain())]);
    for seat in 0..seats {
        scenario = scenario.library_named(seat, LIBRARY);
    }
    let before = GameState::from_setup(&scenario.clone().setup().unwrap()).unwrap();
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let settle = |driver: &mut Driver| {
        while !driver.state().stack().is_empty() {
            driver.pass().unwrap();
        }
    };
    let me = driver.target_player(0);
    driver.cast_at("Mending Hands", &[me]).unwrap();
    settle(&mut driver);
    driver.begin_activation("Drudge Skeletons").unwrap();
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    settle(&mut driver);
    driver.pass().unwrap();
    driver.cast_at("Spark", &[me]).unwrap();
    settle(&mut driver);
    driver.pass().unwrap();
    let skeletons = driver.target_named("Drudge Skeletons").unwrap();
    driver.cast_at("Bolt", &[skeletons]).unwrap();
    settle(&mut driver);
    let events = driver.engine().events().to_vec();
    let after = driver.state().clone();
    let viewers: Vec<PlayerId> = (0..seats).map(PlayerId::from_index).collect();
    let batches = project_events(&before, &events, &viewers, 0).unwrap();
    for (viewer, batch) in viewers.iter().zip(&batches) {
        let public = |wanted: fn(&mtg_engine::Event) -> bool| {
            batch
                .events
                .iter()
                .filter(
                    |view| matches!(view, mtg_protocol::EventView::Public(event) if wanted(event)),
                )
                .count()
        };
        let kept = public(|event| matches!(event, mtg_engine::Event::ShieldUsed { left: 3, .. }));
        let regenerated = public(|event| matches!(event, mtg_engine::Event::Regenerated { .. }));
        assert_eq!((kept, regenerated), (1, 1), "viewer {viewer:?}");
        assert_clean(batch, &hidden_in(&after, *viewer));
    }
}

#[rstest]
fn the_seat_hurt_picks_the_damage_a_shield_takes_first_and_the_question_shows_nothing_hidden(
    #[values(2, 3, 4)] seats: usize,
) {
    use mtg_engine::choose::{ChoiceOption, OptionChoice};
    use mtg_engine::decision::Answer;
    use mtg_testkit::cards::{free_creature, keyworded, plains};
    use mtg_testkit::{Driver, Scenario, ready};
    let mut scenario = Scenario::new(seats)
        .at(2, 0, mtg_engine::turn::Step::DeclareAttackers)
        .hand_named(1, ["Mending Hands"])
        .battlefield(
            0,
            [
                ready(keyworded(
                    "Linker",
                    3,
                    3,
                    &[mtg_cards::keyword::Keyword::Lifelink],
                )),
                ready(free_creature("Grunt", 2, 2)),
            ],
        )
        .battlefield(1, [ready(plains())]);
    for seat in 0..seats {
        scenario = scenario.library_named(seat, LIBRARY);
    }
    let before = GameState::from_setup(&scenario.clone().setup().unwrap()).unwrap();
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.attack(&[("Linker", 1), ("Grunt", 1)]).unwrap();
    driver.pass().unwrap();
    let me = driver.target_player(1);
    driver.cast_at("Mending Hands", &[me]).unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    let mut asked = None;
    for _ in 0..200 {
        let decision = driver.decision().unwrap().clone();
        if matches!(decision.kind, DecisionKind::ChooseOption(_)) {
            asked = Some(decision);
            break;
        }
        driver.answer_passively().unwrap();
    }
    let decision = asked.expect("the damage of two attackers is more than the shield stops");
    // Only the seat that is hurt is asked, and the question names the damage and nothing a seat has not seen.
    assert_eq!(decision.player, PlayerId::from_index(1));
    let DecisionKind::ChooseOption(options) = &decision.kind else {
        unreachable!();
    };
    assert_eq!(options.options.len(), 2);
    assert!(
        options
            .options
            .iter()
            .all(|option| matches!(option, ChoiceOption::Label(_)))
    );
    let hidden = hidden_in(driver.state(), decision.player);
    assert!(!hidden.is_empty());
    let found = leaks(&serde_json::to_value(&decision).unwrap(), &hidden);
    assert!(found.is_empty(), "the question leaks {found:?}");
    driver
        .answer(Answer::ChooseOption(OptionChoice(1)))
        .unwrap();
    let events = driver.engine().events().to_vec();
    let after = driver.state().clone();
    let viewers: Vec<PlayerId> = (0..seats).map(PlayerId::from_index).collect();
    let batches = project_events(&before, &events, &viewers, 0).unwrap();
    for (viewer, batch) in viewers.iter().zip(&batches) {
        assert_clean(batch, &hidden_in(&after, *viewer));
    }
}

/// Umbra armor removes the damage of a creature and destroys its Aura in place of the creature's destruction; both are
/// public, and the question of which of two replacements applies names no card a viewer may not see.
#[rstest]
#[case(2, 411)]
#[case(3, 412)]
#[case(4, 413)]
fn no_hidden_object_reaches_any_viewer_in_games_of_umbra_armor(
    #[case] players: usize,
    #[case] seed: u64,
) {
    let viewers: Vec<PlayerId> = (0..players).map(PlayerId::from_index).collect();
    let (mut checked, mut saved, mut asked) = (0usize, 0usize, 0usize);
    for seed in seed..seed + PREVENTION_GAMES * 4 {
        let setup = with_unique_names(umbra_fuzz_setup(players, seed).unwrap());
        let engine = run_game(setup, seed, |step: &Step| {
            if let Some(decision) = step.decision
                && matches!(decision.kind, DecisionKind::ChooseReplacement(_))
            {
                asked += 1;
                let hidden = hidden_in(step.after, decision.player);
                let found = leaks(&serde_json::to_value(decision).unwrap(), &hidden);
                assert!(
                    found.is_empty(),
                    "seed {seed} answer {}: the question leaks {found:?}",
                    step.answers
                );
            }
            let removals = step
                .events
                .iter()
                .filter(|event| matches!(event, mtg_engine::Event::DamageRemoved { .. }))
                .count();
            saved += removals;
            let batches =
                project_events(step.before, step.events, &viewers, step.from_seq).unwrap();
            for (viewer, batch) in viewers.iter().zip(&batches) {
                let hidden = hidden_across(step.before, step.after, *viewer);
                checked += hidden.len();
                for (part, value) in batch_parts(batch) {
                    let found = leaks(&without_trigger_orders(value), &hidden);
                    assert!(
                        found.is_empty(),
                        "seed {seed} answer {} viewer {viewer:?}: batch {part} leaks {found:?}",
                        step.answers
                    );
                }
                // The removal of the damage is public: every viewer gets each one whole.
                let public = batch
                    .events
                    .iter()
                    .filter(|view| {
                        matches!(
                            view,
                            mtg_protocol::EventView::Public(
                                mtg_engine::Event::DamageRemoved { .. }
                            )
                        )
                    })
                    .count();
                assert_eq!(public, removals, "seed {seed} viewer {viewer:?}");
            }
            if step.answers.is_multiple_of(SNAPSHOT_EVERY) {
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
    }
    assert!(
        checked > 500 && saved > 0,
        "test must not be vacuous: {checked} hidden, {saved} creatures saved by the armor, {asked} questions"
    );
}

#[rstest]
fn the_armor_that_saves_a_creature_reaches_every_seat_whole_and_nothing_hidden_with_it(
    #[values(2, 3, 4)] seats: usize,
) {
    use mtg_testkit::cards::{free_creature, mountain};
    use mtg_testkit::prevent_cards::smite;
    use mtg_testkit::{Driver, Scenario, attached_to, ready};
    let mut scenario = Scenario::new(seats)
        .battlefield(
            0,
            [
                ready(free_creature("Subject", 2, 2)),
                attached_to(ready(mtg_testkit::corpus::card("Eel Umbra").unwrap()), 0, 0),
            ],
        )
        .hand(1, vec![smite("Smite").unwrap()])
        .battlefield(1, [ready(mountain())]);
    for seat in 0..seats {
        scenario = scenario.library_named(seat, LIBRARY);
    }
    let before = GameState::from_setup(&scenario.clone().setup().unwrap()).unwrap();
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.pass().unwrap();
    let subject = driver.target_named("Subject").unwrap();
    driver.cast_at("Smite", &[subject]).unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    let events = driver.engine().events().to_vec();
    let after = driver.state().clone();
    let viewers: Vec<PlayerId> = (0..seats).map(PlayerId::from_index).collect();
    let batches = project_events(&before, &events, &viewers, 0).unwrap();
    for (viewer, batch) in viewers.iter().zip(&batches) {
        let removed = batch
            .events
            .iter()
            .filter(|view| {
                matches!(
                    view,
                    mtg_protocol::EventView::Public(mtg_engine::Event::DamageRemoved { .. })
                )
            })
            .count();
        assert_eq!(removed, 1, "viewer {viewer:?}");
        assert_clean(batch, &hidden_in(&after, *viewer));
    }
}

/// Echo, cumulative upkeep, undying and persist ask their controller to pay (a card discarded from the hand, creatures
/// sacrificed, life), put counters on permanents and return cards from graveyards; none of it names a card a viewer
/// may not see.
#[rstest]
#[case(2, 61)]
#[case(3, 62)]
#[case(4, 63)]
fn no_hidden_object_reaches_any_viewer_in_games_of_the_upkeep_keywords(
    #[case] players: usize,
    #[case] seed: u64,
) {
    let viewers: Vec<PlayerId> = (0..players).map(PlayerId::from_index).collect();
    let (mut checked, mut counters) = (0usize, 0usize);
    let setup = with_unique_names(upkeep_fuzz_setup(players, seed).unwrap());
    let engine = run_game(setup, seed, |step: &Step| {
        counters += step
            .events
            .iter()
            .filter(|event| {
                matches!(event, mtg_engine::Event::CountersChanged(change) if change.delta > 0)
            })
            .count();
        let batches = project_events(step.before, step.events, &viewers, step.from_seq).unwrap();
        for (viewer, batch) in viewers.iter().zip(&batches) {
            let hidden = hidden_across(step.before, step.after, *viewer);
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
    });
    assert!(engine.result().is_some(), "game should finish");
    assert!(
        checked > 500 && counters > 0,
        "test must not be vacuous: {checked} hidden, {counters} counters put"
    );
}

#[rstest]
fn an_amass_is_public_and_leaks_no_card_of_a_hand_or_a_library(#[values(2, 3, 4)] seats: usize) {
    let mut scenario = mtg_testkit::Scenario::new(seats)
        .hand(
            0,
            [
                mtg_testkit::corpus::card("Relentless Advance").unwrap(),
                mtg_testkit::corpus::card("Giant Growth").unwrap(),
            ],
        )
        .battlefield(0, vec![mtg_testkit::ready(mtg_testkit::cards::island()); 4]);
    for seat in 0..seats {
        scenario = scenario.library_named(seat, LIBRARY);
    }
    let answer = |decision: &Decision| panic!("unexpected {}", decision.kind.name());
    let (after, events, batches) = play_spell(scenario, seats, "Relentless Advance", answer);
    let army = events
        .iter()
        .find_map(|event| match event {
            mtg_engine::Event::TokenCreated(token) => Some(token.object),
            _ => None,
        })
        .expect("an Army token is created");
    assert_eq!(after.object(army).unwrap().card().name, "Zombie Army");
    let held: BTreeMap<ObjectId, String> = after
        .zone(Zone::Hand(PlayerId(0)))
        .iter()
        .chain(after.zone(Zone::Library(PlayerId(0))).iter())
        .map(|id| (*id, after.object(*id).unwrap().card().name.clone()))
        .collect();
    for (seat, batch) in batches.iter().enumerate() {
        assert!(
            batch.events.iter().any(|event| matches!(
                event,
                mtg_protocol::EventView::Public(mtg_engine::Event::Amassed {
                    player: PlayerId(0),
                    ..
                })
            )),
            "seat {seat} is told who amassed"
        );
        if seat != 0 {
            assert_clean(batch, &held);
        }
    }
}

#[rstest]
fn a_bolster_is_public_and_leaks_no_card_of_a_hand_or_a_library(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] pick: usize,
) {
    let creature = |name: &str| mtg_testkit::ready(mtg_testkit::cards::free_creature(name, 1, 2));
    let mut board = vec![mtg_testkit::ready(mtg_testkit::cards::forest()); 3];
    board.extend([creature("First Twin"), creature("Second Twin")]);
    let mut scenario = mtg_testkit::Scenario::new(seats)
        .hand(
            0,
            [
                mtg_testkit::corpus::card("Cached Defenses").unwrap(),
                mtg_testkit::corpus::card("Giant Growth").unwrap(),
            ],
        )
        .battlefield(0, board);
    for seat in 0..seats {
        scenario = scenario.library_named(seat, LIBRARY);
    }
    // The two creatures tie for the least toughness: the player is asked to choose one of them.
    let answer = |decision: &Decision| match &decision.kind {
        DecisionKind::ChooseCards(options) => {
            mtg_engine::decision::Answer::ChooseCards(mtg_engine::choose::CardsChoice(vec![
                options.from[pick],
            ]))
        }
        other => panic!("unexpected {}", other.name()),
    };
    let (after, _, batches) = play_spell(scenario, seats, "Cached Defenses", answer);
    let held: BTreeMap<ObjectId, String> = after
        .zone(Zone::Hand(PlayerId(0)))
        .iter()
        .chain(after.zone(Zone::Library(PlayerId(0))).iter())
        .map(|id| (*id, after.object(*id).unwrap().card().name.clone()))
        .collect();
    for (seat, batch) in batches.iter().enumerate() {
        assert!(
            batch.events.iter().any(|event| matches!(
                event,
                mtg_protocol::EventView::Public(mtg_engine::Event::Bolstered {
                    player: PlayerId(0),
                    ..
                })
            )),
            "seat {seat} is told who bolstered"
        );
        if seat != 0 {
            assert_clean(batch, &held);
        }
    }
}

/// The cards seat 0 holds in its hand and library when `after` is reached.
fn held_by_seat_zero(after: &GameState) -> BTreeMap<ObjectId, String> {
    after
        .zone(Zone::Hand(PlayerId(0)))
        .iter()
        .chain(after.zone(Zone::Library(PlayerId(0))).iter())
        .map(|id| (*id, after.object(*id).unwrap().card().name.clone()))
        .collect()
}

#[rstest]
fn a_learn_is_public_and_leaks_neither_the_card_drawn_nor_the_rest_of_the_hand(
    #[values(2, 3, 4)] seats: usize,
) {
    let mut scenario = mtg_testkit::Scenario::new(seats)
        .hand(
            0,
            [
                mtg_testkit::corpus::card("Pop Quiz").unwrap(),
                mtg_testkit::corpus::card("Consider").unwrap(),
                mtg_testkit::corpus::card("Opt").unwrap(),
            ],
        )
        .battlefield(0, vec![mtg_testkit::ready(mtg_testkit::cards::island()); 3]);
    for seat in 0..seats {
        scenario = scenario.library_named(seat, LIBRARY);
    }
    let answer = |decision: &Decision| match &decision.kind {
        DecisionKind::ChooseCards(options) => {
            mtg_engine::decision::Answer::ChooseCards(mtg_engine::choose::CardsChoice(vec![
                options.from[0],
            ]))
        }
        other => panic!("unexpected {}", other.name()),
    };
    let (after, _, batches) = play_spell(scenario, seats, "Pop Quiz", answer);
    let held = held_by_seat_zero(&after);
    for (seat, batch) in batches.iter().enumerate() {
        assert!(
            batch.events.iter().any(|event| matches!(
                event,
                mtg_protocol::EventView::Public(mtg_engine::Event::Learned {
                    player: PlayerId(0)
                })
            )),
            "seat {seat} is told who learned"
        );
        if seat != 0 {
            assert_clean(batch, &held);
        }
    }
}

#[rstest]
fn a_fabricate_is_public_and_the_blessing_it_brings_is_told_to_every_seat(
    #[values(2, 3, 4)] seats: usize,
) {
    // The Aspirant, seven Swamps and the Squad are nine permanents; the Servo makes ten.
    let mut board = vec![mtg_testkit::ready(mtg_testkit::cards::swamp()); 7];
    board.push(mtg_testkit::ready(
        mtg_testkit::corpus::card("Skymarcher Aspirant").unwrap(),
    ));
    let mut scenario = mtg_testkit::Scenario::new(seats)
        .hand(
            0,
            [
                mtg_testkit::corpus::card("Maulfist Squad").unwrap(),
                mtg_testkit::corpus::card("Consider").unwrap(),
            ],
        )
        .battlefield(0, board);
    for seat in 0..seats {
        scenario = scenario.library_named(seat, LIBRARY);
    }
    let answer = |decision: &Decision| match &decision.kind {
        DecisionKind::YesNo(_) => {
            mtg_engine::decision::Answer::YesNo(mtg_engine::choose::YesNoChoice(false))
        }
        other => panic!("unexpected {}", other.name()),
    };
    let (after, events, batches) = play_spell(scenario, seats, "Maulfist Squad", answer);
    let servos = events
        .iter()
        .filter(
            |event| matches!(event, mtg_engine::Event::TokenCreated(token) if token.name == "Servo"),
        )
        .count();
    assert_eq!(servos, 1);
    assert!(after.has_citys_blessing(PlayerId(0)));
    // The number in the Fabricate ability is no card: an id that equals it is not looked for.
    let held: BTreeMap<_, _> = held_by_seat_zero(&after)
        .into_iter()
        .filter(|(id, _)| id.get() > 1)
        .collect();
    for (seat, batch) in batches.iter().enumerate() {
        assert!(
            batch.events.iter().any(|event| matches!(
                event,
                mtg_protocol::EventView::Public(mtg_engine::Event::CitysBlessingGained {
                    player: PlayerId(0)
                })
            )),
            "seat {seat} is told who got the city's blessing"
        );
        if seat != 0 {
            assert_clean(batch, &held);
        }
    }
}
