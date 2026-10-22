//! Goal: a ninjutsu activation names the ninja card to every seat as its ability goes on the stack, as it is shown at a real table, and nothing else of the hands: the pick of the attacker to return is asked of the activating seat only, the attacker that returns is told to everyone, no other card of any hand is named to a seat that may not see it, and the ninja is named in the snapshot of every seat while the ability waits and after it resolved.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameState};
use mtg_protocol::{EventBatch, EventView, StateView, project_events};
use mtg_testkit::cards::{forest, island};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;
use serde_json::Value;

use crate::hidden_info::{assert_clean, hidden_across};

const NINJA: &str = "Ninja of the Deep Hours";

fn seat(index: usize) -> PlayerId {
    PlayerId::from_index(index)
}

/// Seat 0 holds the ninja and a card that stays hidden and attacks with two creatures; every other seat has a hand of its
/// own.
fn table(seats: usize) -> Scenario {
    let creature = |name: &str| ready(mtg_testkit::corpus::card(name).unwrap());
    let mut scenario = Scenario::new(seats)
        .library(0, vec![forest(); 8])
        .hand_named(0, [NINJA, "Counterspell"])
        .battlefield(
            0,
            [
                ready(island()),
                ready(island()),
                creature("Grizzly Bears"),
                creature("Serra Angel"),
            ],
        );
    let others = [
        ["Wrath of God", "Doom Blade"],
        ["Fireball", "Giant Growth"],
        ["Opt", "Brainstorm"],
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

/// Seat 0 attacks seat 1 with both creatures and the game stops in the declare blockers step with nobody blocking.
fn to_blockers(driver: &mut Driver) {
    driver.idle_until(1, Step::DeclareAttackers).unwrap();
    driver
        .attack(&[("Grizzly Bears", 1), ("Serra Angel", 1)])
        .unwrap();
    while !(driver.state().turn().step() == Step::DeclareBlockers
        && matches!(driver.decision().unwrap().kind, DecisionKind::Priority(_)))
    {
        driver.answer_passively().unwrap();
    }
}

/// The ids of the cards called `name` in `zone`.
fn card_in(state: &GameState, zone: Zone, name: &str) -> ObjectId {
    state
        .zone(zone)
        .iter()
        .copied()
        .find(|id| state.object(*id).unwrap().card().name == name)
        .unwrap()
}

#[rstest]
fn the_ninja_is_named_to_every_seat_as_the_ability_goes_on_the_stack_and_no_other_hand_card_is(
    #[values(2, 3, 4)] seats: usize,
) {
    let mut driver = Driver::from_scenario(table(seats)).unwrap();
    to_blockers(&mut driver);
    let ninja = card_in(driver.state(), Zone::Hand(seat(0)), NINJA);
    let before = driver.state().clone();
    let logged = driver.engine().events().len();
    driver.begin_activation(NINJA).unwrap();
    let DecisionKind::ChooseCards(pick) = &driver.decision().unwrap().kind else {
        panic!("the activating seat picks the attacker to return");
    };
    assert_eq!(driver.decision().unwrap().player, seat(0));
    assert_eq!(pick.from.len(), 2, "both unblocked attackers are offered");
    let bears = card_in(driver.state(), Zone::Battlefield, "Grizzly Bears");
    driver
        .answer(Answer::ChooseCards(CardsChoice(vec![bears])))
        .unwrap();
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    assert_eq!(driver.state().stack().len(), 1, "the ability waits");
    let after = driver.state().clone();
    let events = driver.engine().events()[logged..].to_vec();
    let from_seq = u64::try_from(logged).unwrap();
    let all: Vec<PlayerId> = (0..=seats).map(seat).collect();
    let batches = project_events(&before, &events, &all, from_seq).unwrap();
    for (viewer, batch) in all.iter().zip(&batches) {
        assert!(
            batch.events.iter().any(|view| matches!(
                view,
                EventView::Public(Event::AbilityPutOnStack(put)) if put.source.id == ninja
            )),
            "{viewer:?} sees the ability go on the stack with the card as its source"
        );
        assert!(batch_names(batch, NINJA), "{viewer:?} reads the ninja card");
        assert!(
            batch.events.iter().any(|view| matches!(
                view,
                EventView::Public(Event::ZoneChanged { object, to: Zone::Hand(_), .. })
                    if *object == bears
            )) || batch_names(batch, "Grizzly Bears"),
            "{viewer:?} is told which attacker returned"
        );
        let mut hidden = hidden_across(&before, &after, *viewer);
        hidden.remove(&ninja);
        assert_clean(batch, &hidden);
        for name in ["Counterspell", "Wrath of God", "Fireball", "Opt"] {
            assert!(!batch_names(batch, name), "{viewer:?} reads {name}");
        }
    }
    // The state a client resumes from names the ninja card in the ability on the stack and the rest of the hand only to its owner.
    for viewer in &all {
        let value = serde_json::to_value(StateView::for_player(&after, *viewer)).unwrap();
        assert!(names(&value, NINJA), "{viewer:?} reads the ninja");
        assert_eq!(names(&value, "Counterspell"), *viewer == seat(0));
    }
}

#[rstest]
fn the_resolved_ability_shows_the_ninja_on_the_battlefield_to_every_seat(
    #[values(2, 3, 4)] seats: usize,
) {
    let mut driver = Driver::from_scenario(table(seats)).unwrap();
    to_blockers(&mut driver);
    let ninja = card_in(driver.state(), Zone::Hand(seat(0)), NINJA);
    let bears = card_in(driver.state(), Zone::Battlefield, "Grizzly Bears");
    let before = driver.state().clone();
    let logged = driver.engine().events().len();
    driver.begin_activation(NINJA).unwrap();
    driver
        .answer(Answer::ChooseCards(CardsChoice(vec![bears])))
        .unwrap();
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    let after = driver.state().clone();
    assert_eq!(after.object(ninja).unwrap().zone(), Zone::Battlefield);
    let events = driver.engine().events()[logged..].to_vec();
    let all: Vec<PlayerId> = (0..=seats).map(seat).collect();
    let batches = project_events(&before, &events, &all, u64::try_from(logged).unwrap()).unwrap();
    for (viewer, batch) in all.iter().zip(&batches) {
        assert!(
            batch.objects.iter().any(|object| object.id == ninja
                && object.zone == Zone::Battlefield
                && object.name == NINJA),
            "{viewer:?} sees the ninja on the battlefield"
        );
        let mut hidden = hidden_across(&before, &after, *viewer);
        hidden.remove(&ninja);
        assert_clean(batch, &hidden);
        assert!(
            !batch_names(batch, "Counterspell"),
            "{viewer:?} reads the hand"
        );
        let view = StateView::for_player(&after, *viewer);
        assert!(
            view.objects
                .iter()
                .any(|object| object.id == ninja && object.zone == Zone::Battlefield),
            "{viewer:?} has the ninja in the snapshot"
        );
    }
}
