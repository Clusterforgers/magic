//! Goal: a manual action is told to every seat as the event that names the seat and the action, and what it moves is told as any move is: a card that goes to a graveyard, to exile or onto the battlefield is named to everyone, a card that goes between a hand and a library is named to its owner alone and the other seats are told that the seat moved a card there, never which; tokens, counters, life and tapping are public; a card the engine does not run shows the clauses it leaves to the table to every seat that sees the card; and the views the seats build by folding the batches equal the true ones.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_engine::manual::{CommonToken, ManualAction, ManualToken, ManualZone};
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, Event};
use mtg_protocol::{EventBatch, EventView, HiddenEvent, project_events};
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::manual_cards::manual_creature;
use mtg_testkit::{ManualScript, RandomBot, Scenario, manual_fuzz_setup, ready};
use rstest::rstest;
use serde_json::Value;

use crate::common::table::{Run, Table, seats};
use crate::common::with_unique_names;
use crate::hidden_info::{assert_clean, hidden_across, hidden_in};

fn seat(index: usize) -> PlayerId {
    PlayerId::from_index(index)
}

/// Seat 0 has two named cards in hand, a named library and a creature with a clause the engine does not run on the
/// battlefield; every other seat has a hand of its own.
fn table(seat_count: usize) -> Table {
    let mut scenario = Scenario::new(seat_count)
        .hand(
            0,
            [
                free_creature("Secret hand card", 1, 1),
                free_creature("Second hand card", 1, 1),
            ],
        )
        .library(
            0,
            [
                free_creature("Secret top", 1, 1),
                free_creature("Secret next", 1, 1),
                forest(),
                forest(),
            ],
        )
        .battlefield(
            0,
            [
                ready(manual_creature("Open drake")),
                ready(free_creature("Open bear", 2, 2)),
            ],
        );
    for other in 1..seat_count {
        scenario = scenario
            .hand(other, [free_creature(&format!("Foe card {other}"), 1, 1)])
            .library(other, vec![forest(); 4]);
    }
    Table::new(scenario)
}

/// Whether the text of `value` holds `name` anywhere.
fn names(value: &Value, name: &str) -> bool {
    match value {
        Value::String(text) => text == name,
        Value::Array(items) => items.iter().any(|item| names(item, name)),
        Value::Object(map) => map.values().any(|item| names(item, name)),
        _ => false,
    }
}

/// Whether any part of the batch holds the card's name.
fn batch_names(batch: &EventBatch, name: &str) -> bool {
    [
        serde_json::to_value(&batch.events).unwrap(),
        serde_json::to_value(&batch.table).unwrap(),
        serde_json::to_value(&batch.objects).unwrap(),
        serde_json::to_value(&batch.gone).unwrap(),
    ]
    .iter()
    .any(|part| names(part, name))
}

/// The id of the card called `name` in a hand or library of seat 0.
fn card(table: &Table, name: &str) -> ObjectId {
    table
        .driver
        .state()
        .objects()
        .find(|o| o.card().name == name)
        .map(|o| o.id())
        .unwrap_or_else(|| panic!("no {name}"))
}

/// Takes the action as seat 0.
fn act(table: &mut Table, action: ManualAction) -> Run {
    table.run(|driver| driver.manual(action))
}

/// The marker every seat is told first.
fn marker(batch: &EventBatch) -> &EventView {
    &batch.events[0]
}

/// Who reads the name of a card in a batch of its move: a card in a library is named to nobody, even its owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Named {
    All,
    OwnerOnly,
    Nobody,
}

#[rstest]
#[case::hand_to_the_top_of_the_library("Secret hand card", ManualZone::LibraryTop, Named::Nobody, false)]
#[case::hand_to_the_bottom_of_the_library("Secret hand card", ManualZone::LibraryBottom, Named::Nobody, false)]
#[case::hand_to_the_graveyard("Secret hand card", ManualZone::Graveyard, Named::All, true)]
#[case::hand_to_exile("Secret hand card", ManualZone::Exile, Named::All, true)]
#[case::hand_to_the_battlefield("Secret hand card", ManualZone::Battlefield, Named::All, true)]
#[case::the_battlefield_to_the_hand("Open bear", ManualZone::Hand, Named::OwnerOnly, true)]
#[case::the_battlefield_to_the_library("Open bear", ManualZone::LibraryTop, Named::Nobody, true)]
fn a_card_moved_by_hand_is_named_unless_it_goes_between_places_the_others_cannot_see(
    #[values(2, 3, 4)] seat_count: usize,
    #[case] name: &str,
    #[case] to: ManualZone,
    #[case] named: Named,
    #[case] whole_to_all: bool,
) {
    let mut table = table(seat_count);
    let id = card(&table, name);
    let run = act(&mut table, ManualAction::Move { object: id, to });
    let after = table.driver.state().clone();
    for (viewer, batch) in seats(seat_count).into_iter().zip(&run.batches) {
        let owner = viewer == seat(0);
        assert_eq!(
            batch_names(batch, name),
            named == Named::All || (named == Named::OwnerOnly && owner),
            "who is told the name of {name}: {viewer:?}"
        );
        match marker(batch) {
            EventView::Public(Event::ManualAction { player, action }) => {
                assert!(owner || whole_to_all, "{viewer:?} is told the card");
                assert_eq!(*player, seat(0));
                assert_eq!(**action, ManualAction::Move { object: id, to });
            }
            EventView::Hidden(HiddenEvent::ManualMove { player, to: told }) => {
                assert!(!owner && !whole_to_all, "{viewer:?} is not told the card");
                assert_eq!((*player, *told), (seat(0), to));
            }
            other => panic!("the first event is the action, not {other:?}"),
        }
        if !owner && !whole_to_all {
            assert_clean(batch, &hidden_in(&after, viewer));
        }
    }
}

#[rstest]
#[case::to_the_graveyard("Secret top", ManualZone::Graveyard, true)]
#[case::to_exile("Secret top", ManualZone::Exile, true)]
#[case::to_the_hand("Secret top", ManualZone::Hand, false)]
#[case::to_the_bottom("Secret top", ManualZone::LibraryBottom, false)]
fn the_top_of_the_library_moved_by_hand_is_named_as_the_move_tells_it(
    #[values(2, 3, 4)] seat_count: usize,
    #[case] name: &str,
    #[case] to: ManualZone,
    #[case] told_to_all: bool,
) {
    let mut table = table(seat_count);
    let run = act(&mut table, ManualAction::MoveTop { count: 1, to });
    let after = table.driver.state().clone();
    for (viewer, batch) in seats(seat_count).into_iter().zip(&run.batches) {
        let owner = viewer == seat(0);
        let named = batch_names(batch, name);
        // Only a card moved to the hand is named to its owner alone: the owner learns the card on the bottom of a library
        // from no event, and the others are told how many cards went there.
        assert_eq!(
            named,
            told_to_all || (owner && to == ManualZone::Hand),
            "who is told the name of {name}: {viewer:?}"
        );
        assert!(matches!(
            marker(batch),
            EventView::Public(Event::ManualAction { .. })
        ));
        if !owner && !told_to_all {
            assert_clean(batch, &hidden_in(&after, viewer));
        }
    }
}

#[rstest]
fn a_draw_by_hand_is_told_to_the_others_without_the_card(#[values(2, 3, 4)] seat_count: usize) {
    let mut table = table(seat_count);
    let run = act(&mut table, ManualAction::Draw { count: 2 });
    let after = table.driver.state().clone();
    for (viewer, batch) in seats(seat_count).into_iter().zip(&run.batches) {
        let owner = viewer == seat(0);
        assert_eq!(batch_names(batch, "Secret top"), owner, "{viewer:?}");
        assert_eq!(batch_names(batch, "Secret next"), owner, "{viewer:?}");
        let drawn = batch
            .events
            .iter()
            .filter(|event| match event {
                EventView::Public(Event::CardDrawn { .. }) => owner,
                EventView::Hidden(HiddenEvent::CardDrawn { player }) => {
                    !owner && *player == seat(0)
                }
                _ => false,
            })
            .count();
        assert_eq!(drawn, 2, "{viewer:?}");
        if !owner {
            assert_clean(batch, &hidden_in(&after, viewer));
        }
    }
}

#[rstest]
fn a_discard_by_hand_names_the_card_to_everyone_as_it_reaches_the_graveyard(
    #[values(2, 3, 4)] seat_count: usize,
) {
    let mut table = table(seat_count);
    let id = card(&table, "Second hand card");
    let run = act(&mut table, ManualAction::Discard { object: id });
    for batch in &run.batches {
        assert!(batch_names(batch, "Second hand card"));
        assert!(matches!(
            marker(batch),
            EventView::Public(Event::ManualAction { .. })
        ));
    }
}

#[rstest]
#[case::a_token(
    ManualAction::CreateToken { token: ManualToken::Common(CommonToken::Treasure), count: 2 },
    "Treasure")]
#[case::counters(
    ManualAction::Counters { object: ObjectId(0), counter: CounterKind::Charge, delta: 2 },
    "Open bear")]
#[case::tapping(ManualAction::Tap { object: ObjectId(0) }, "Open bear")]
fn tokens_counters_and_tapping_by_hand_are_public(
    #[values(2, 3, 4)] seat_count: usize,
    #[case] action: ManualAction,
    #[case] name: &str,
) {
    let mut table = table(seat_count);
    let action = match action {
        ManualAction::Counters {
            counter, delta, ..
        } => ManualAction::Counters {
            object: card(&table, "Open bear"),
            counter,
            delta,
        },
        ManualAction::Tap { .. } => ManualAction::Tap {
            object: card(&table, "Open bear"),
        },
        other => other,
    };
    let run = act(&mut table, action.clone());
    for batch in &run.batches {
        assert!(matches!(
            marker(batch),
            EventView::Public(Event::ManualAction { action: logged, .. }) if **logged == action
        ));
        assert!(batch_names(batch, name));
    }
    // Every seat's folded view shows the change.
    for view in &table.views {
        match &action {
            ManualAction::CreateToken { .. } => {
                let tokens = view.objects.iter().filter(|o| o.name == "Treasure").count();
                assert_eq!(tokens, 2);
            }
            ManualAction::Counters { object, .. } => {
                let shown = view.objects.iter().find(|o| o.id == *object).unwrap();
                assert_eq!(shown.counters, [(CounterKind::Charge, 2)]);
            }
            ManualAction::Tap { object } => {
                let shown = view.objects.iter().find(|o| o.id == *object).unwrap();
                assert!(shown.permanent.as_ref().unwrap().tapped);
            }
            _ => {}
        }
    }
}

#[rstest]
fn life_and_a_seats_own_counters_changed_by_hand_are_public(#[values(2, 3, 4)] seat_count: usize) {
    let mut table = table(seat_count);
    act(&mut table, ManualAction::Life { player: seat(0), delta: -3 });
    act(
        &mut table,
        ManualAction::PlayerCounters {
            player: seat(0),
            counter: CounterKind::Poison,
            delta: 2,
        },
    );
    for view in &table.views {
        let me = &view.table.players[0];
        assert_eq!(me.life, 17);
        assert_eq!(me.counters, [(CounterKind::Poison, 2)]);
        assert!(view.table.players[1..].iter().all(|p| p.life == 20));
    }
}

#[rstest]
fn a_card_the_engine_does_not_run_shows_its_clauses_to_every_seat_that_sees_the_card(
    #[values(2, 3, 4)] seat_count: usize,
) {
    let table = table(seat_count);
    let drake = card(&table, "Open drake");
    assert_eq!(
        table.driver.state().object(drake).unwrap().zone(),
        Zone::Battlefield
    );
    for view in &table.views {
        let shown = view.objects.iter().find(|o| o.id == drake).unwrap();
        assert_eq!(shown.manual.len(), 1);
        assert_eq!(
            shown.manual[0].text,
            "Whenever this creature dies, each opponent loses 1 life."
        );
        assert!(!shown.keywords.is_empty(), "the engine's flying is listed");
        let bear = view.objects.iter().find(|o| o.name == "Open bear").unwrap();
        assert!(bear.manual.is_empty());
        let json = serde_json::to_value(bear).unwrap();
        assert!(json.get("manual").is_none(), "left out while empty");
    }
}

/// Games with cards the engine does not run, under unique names and with manual actions taken at random times: no part of
/// what a seat is told names a card or an object that is hidden from it after the answer.
#[rstest]
#[case(2, 41)]
#[case(3, 42)]
#[case(4, 43)]
fn no_hidden_object_reaches_any_viewer_in_games_with_manual_actions(
    #[case] players: usize,
    #[case] seed: u64,
) {
    let setup = with_unique_names(manual_fuzz_setup(players, seed).unwrap());
    let viewers = seats(players);
    let mut engine = Engine::new(setup).unwrap();
    let mut bots: Vec<RandomBot> = (0u64..)
        .take(players)
        .map(|seat| RandomBot::new(seed, seat))
        .collect();
    let mut script = ManualScript::new(seed, 4);
    let (mut taken, mut checked) = (0usize, 0usize);
    while let Some(decision) = engine.decision().cloned() {
        let scripted = (matches!(decision.kind, DecisionKind::Priority(_)) && script.due())
            .then(|| script.action(engine.state(), decision.player))
            .flatten();
        let answer = match scripted {
            Some(action) => {
                taken += 1;
                Answer::Priority(PriorityAction::Manual(action))
            }
            None => bots[decision.player.index()].choose(&decision),
        };
        let before = engine.state().clone();
        let from_seq = engine.events().len() as u64;
        let events = engine.apply(answer).unwrap();
        let batches = project_events(&before, &events, &viewers, from_seq).unwrap();
        for (viewer, batch) in viewers.iter().zip(&batches) {
            let hidden = hidden_across(&before, engine.state(), *viewer);
            checked += hidden.len();
            assert_clean(batch, &hidden);
        }
    }
    assert!(engine.result().is_some(), "the game finishes");
    assert!(taken > 0 && checked > 500, "not vacuous: {taken} {checked}");
}
