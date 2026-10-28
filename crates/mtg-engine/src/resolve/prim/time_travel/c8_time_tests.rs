//! Goal: a time travel offers exactly the permanents its player controls with a time counter and the suspended cards they own (not another seat's, not a card in exile without suspend), adds one time counter to each chosen first and removes one from each chosen of the rest, asks nothing when nothing holds a time counter, at two to four seats, and the same answers replayed give the same state.
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use rstest::rstest;

use super::*;
use crate::choose::Choice;
use crate::decision::DecisionKind;
use crate::events::ZoneCause;
use crate::fixtures::{creature, setup};
use crate::fold::apply_event;
use crate::resolve::{Resolving, Scope};

const ME: PlayerId = PlayerId(0);

fn c8_time_suspender(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Sorcery)
            .with_rules(
                serde_json::from_str(r#"{"Keyword":{"Suspend":{"time":3,"cost":"{R}"}}}"#).unwrap(),
            ),
    )
}

fn c8_time_named(game: &GameState, name: &str) -> ObjectId {
    game.objects()
        .find(|o| o.card().name == name)
        .map(crate::object::GameObject::id)
        .unwrap()
}

/// Puts `counters` time counters on the object named `name`, after exiling it from its hand when `exile`.
fn c8_time_give(game: &mut GameState, name: &str, counters: i32, exile: bool) {
    let id = c8_time_named(game, name);
    if exile {
        let object = game.object(id).unwrap();
        let event = Event::ZoneChanged {
            object: id,
            from: object.zone(),
            to: Zone::Exile,
            cause: ZoneCause::Exiled,
        };
        apply_event(game, &event).unwrap();
    }
    let on = Target::Object(game.object(id).unwrap().reference());
    let changed = CountersChanged {
        on,
        counter: time_counter(),
        delta: counters,
    };
    apply_event(game, &Event::CountersChanged(changed)).unwrap();
}

/// Seat 0 controls the Source and a Clock with two time counters and has a Rift suspended with three and a Plain card
/// without suspend exiled with one; seat 1 controls a Bear with a time counter and has a Comet suspended.
fn c8_time_game(seats: usize) -> GameState {
    let mut hands = vec![Vec::new(); seats];
    hands[0] = vec![c8_time_suspender("Rift"), creature("Plain", 1, 1)];
    hands[1] = vec![c8_time_suspender("Comet")];
    let mut boards = vec![Vec::new(); seats];
    boards[0] = vec![creature("Source", 1, 1), creature("Clock", 1, 1)];
    boards[1] = vec![creature("Bear", 2, 2)];
    let mut game = GameState::from_setup(&setup(hands, boards)).unwrap();
    c8_time_give(&mut game, "Clock", 2, false);
    c8_time_give(&mut game, "Bear", 1, false);
    c8_time_give(&mut game, "Rift", 3, true);
    c8_time_give(&mut game, "Plain", 1, true);
    c8_time_give(&mut game, "Comet", 3, true);
    game
}

fn c8_time_run(game: &GameState, journal: Vec<(PlayerId, Choice)>) -> Run {
    let source = game.object(c8_time_named(game, "Source")).unwrap();
    let resolving = Resolving {
        journal,
        ..Resolving::begin(source.id())
    };
    let cx = Cx {
        scope: Scope::announcing(game, ME, source.reference()),
        resolving: &resolving,
    };
    run(&cx, &serde_json::from_str(r#"{"TimeTravel":{}}"#).unwrap()).unwrap()
}

fn c8_time_offered(run: &Run) -> Vec<ObjectId> {
    let Run::Ask(decision) = run else {
        panic!("the time travel asks");
    };
    let DecisionKind::ChooseCards(options) = &decision.kind else {
        panic!("a choice of cards");
    };
    options.from.clone()
}

#[rstest]
#[case::two_seats_add_both(2, &["Clock", "Rift"], &[], 3, 4)]
#[case::three_seats_remove_both(3, &[], &["Clock", "Rift"], 1, 2)]
#[case::four_seats_add_one_remove_other(4, &["Rift"], &["Clock"], 1, 4)]
#[case::two_seats_neither(2, &[], &[], 2, 3)]
fn the_player_adds_or_removes_a_time_counter_on_each_chosen(
    #[case] seats: usize,
    #[case] add: &[&str],
    #[case] remove: &[&str],
    #[case] clock: u32,
    #[case] rift: u32,
) {
    let game = c8_time_game(seats);
    let ids = |names: &[&str]| -> Vec<ObjectId> {
        names.iter().map(|n| c8_time_named(&game, n)).collect()
    };
    let first = c8_time_run(&game, Vec::new());
    assert_eq!(c8_time_offered(&first), ids(&["Clock", "Rift"]));
    let mut journal = vec![(ME, Choice::Cards(ids(add)))];
    if add.len() < 2 {
        let second = c8_time_run(&game, journal.clone());
        let rest: Vec<&str> = ["Clock", "Rift"]
            .into_iter()
            .filter(|n| !add.contains(n))
            .collect();
        assert_eq!(c8_time_offered(&second), ids(&rest));
        journal.push((ME, Choice::Cards(ids(remove))));
    }
    let Run::Done { events, .. } = c8_time_run(&game, journal) else {
        panic!("the time travel is done");
    };
    assert_eq!(events.len(), add.len() + remove.len());
    let mut after = game.clone();
    let mut replay = game.clone();
    for event in &events {
        apply_event(&mut after, event).unwrap();
        apply_event(&mut replay, event).unwrap();
    }
    let time = time_counter();
    let held = |name: &str| {
        let object = after.object(c8_time_named(&after, name)).unwrap();
        object
            .permanent()
            .map_or_else(|| object.card_counters_of(&time), |p| p.counters_of(&time))
    };
    assert_eq!((held("Clock"), held("Rift")), (clock, rift));
    assert_eq!((held("Bear"), held("Comet"), held("Plain")), (1, 3, 1));
    assert_eq!(after.state_hash(), replay.state_hash());
}

#[test]
fn nothing_with_a_time_counter_asks_nothing() {
    let game = GameState::from_setup(&setup(
        vec![Vec::new(), Vec::new()],
        vec![vec![creature("Source", 1, 1)], Vec::new()],
    ))
    .unwrap();
    let Run::Done { events, .. } = c8_time_run(&game, Vec::new()) else {
        panic!("nothing to ask");
    };
    assert_eq!(events, Vec::new());
}
