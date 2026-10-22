//! Goal: a spell with a required additional cost that discards a card ("As an additional cost to cast this spell, discard a card") keeps the rest of the hand hidden: the pick is asked of the casting seat alone and logs nothing until the spell is paid for, the card discarded is named to every seat once it is in the graveyard, and no other card of any hand is named to a seat that may not see it, in the events, the table, the objects or the snapshot.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameState};
use mtg_protocol::{EventBatch, EventView, StateView, project_events};
use mtg_testkit::cards::{forest, mountain};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;
use serde_json::Value;

use crate::hidden_info::{assert_clean, hidden_across};

const SPELL: &str = "Thrill of Possibility";

fn seat(index: usize) -> PlayerId {
    PlayerId::from_index(index)
}

/// Seat 0 holds the spell and two cards that stay hidden unless one is discarded; every other seat has a hand of its own.
fn table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .library(0, vec![forest(); 8])
        .hand_named(0, [SPELL, "Counterspell", "Wrath of God"])
        .battlefield(0, [ready(mountain()), ready(mountain())]);
    let others = [
        ["Doom Blade", "Fireball"],
        ["Giant Growth", "Opt"],
        ["Brainstorm", "Lightning Bolt"],
    ];
    for other in 1..seats {
        scenario = scenario
            .library(other, vec![forest(); 8])
            .hand_named(other, others[other - 1]);
    }
    scenario
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

fn card_in(state: &GameState, zone: Zone, name: &str) -> ObjectId {
    state
        .zone(zone)
        .iter()
        .copied()
        .find(|id| state.object(*id).unwrap().card().name == name)
        .unwrap()
}

#[rstest]
fn the_pick_is_asked_of_the_caster_alone_and_the_discarded_card_is_named_to_every_seat_and_the_rest_of_the_hand_to_none_but_its_owner(
    #[values(2, 3, 4)] seats: usize,
) {
    let mut driver = Driver::from_scenario(table(seats)).unwrap();
    let before = driver.state().clone();
    let logged = driver.engine().events().len();
    driver.begin_cast(SPELL).unwrap();
    let decision = driver.decision().unwrap();
    assert_eq!(decision.player, seat(0), "the casting seat is asked");
    let DecisionKind::ChooseCards(pick) = &decision.kind else {
        panic!("the discard is picked, not {}", decision.kind.name());
    };
    assert_eq!(
        pick.from.len(),
        2,
        "the spell is not offered for its own cost"
    );
    assert_eq!(
        driver.engine().events().len(),
        logged,
        "the pick leaves no trace in the log"
    );
    let counterspell = card_in(driver.state(), Zone::Hand(seat(0)), "Counterspell");
    driver
        .answer(Answer::ChooseCards(CardsChoice(vec![counterspell])))
        .unwrap();
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    let after = driver.state().clone();
    let events = driver.engine().events()[logged..].to_vec();
    assert!(events.iter().any(|event| matches!(
        event,
        Event::CardDiscarded { object, .. } if *object == counterspell
    )));
    let all: Vec<PlayerId> = (0..=seats).map(seat).collect();
    let batches = project_events(&before, &events, &all, u64::try_from(logged).unwrap()).unwrap();
    for (viewer, batch) in all.iter().zip(&batches) {
        assert!(
            batch.events.iter().any(|view| matches!(
                view,
                EventView::Public(Event::CardDiscarded { object, .. }) if *object == counterspell
            )),
            "{viewer:?} sees the discard"
        );
        assert!(
            batch_names(batch, "Counterspell"),
            "{viewer:?} reads the discarded card"
        );
        let mut hidden = hidden_across(&before, &after, *viewer);
        hidden.remove(&counterspell);
        assert_clean(batch, &hidden);
        if *viewer != seat(0) {
            assert!(
                !batch_names(batch, "Wrath of God"),
                "{viewer:?} reads the card that stayed in the hand"
            );
        }
        for name in [
            "Doom Blade",
            "Fireball",
            "Giant Growth",
            "Opt",
            "Brainstorm",
            "Lightning Bolt",
        ] {
            assert!(
                !batch_names(batch, name),
                "{viewer:?} reads {name} of another hand"
            );
        }
    }
    // The snapshot a client resumes from holds the discarded card in the graveyard and the rest of the hand for its owner.
    for viewer in &all {
        let value = serde_json::to_value(StateView::for_player(&after, *viewer)).unwrap();
        assert!(
            names(&value, "Counterspell"),
            "{viewer:?} reads the graveyard"
        );
        assert_eq!(names(&value, "Wrath of God"), *viewer == seat(0));
    }
}

#[rstest]
fn backing_out_of_the_pick_logs_nothing_and_names_no_card(#[values(2, 3, 4)] seats: usize) {
    let mut driver = Driver::from_scenario(table(seats)).unwrap();
    let logged = driver.engine().events().len();
    driver.begin_cast(SPELL).unwrap();
    driver
        .answer(Answer::ChooseCards(CardsChoice(Vec::new())))
        .unwrap();
    assert_eq!(driver.engine().events().len(), logged);
    assert!(matches!(
        driver.decision().unwrap().kind,
        DecisionKind::Priority(_)
    ));
    assert_eq!(driver.state().zone(Zone::Hand(seat(0))).len(), 3);
}
