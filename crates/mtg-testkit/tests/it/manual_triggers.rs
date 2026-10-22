//! Goal: the state-based actions and the triggered abilities run after a manual action as after any event: a creature put into the graveyard by hand dies and its trigger fires (and does not from a move to exile or to the hand), a permanent that arrives by hand or as a token triggers what watches arrivals, a draw and a discard by hand trigger what watches them, counters that leave a creature with no toughness kill it, an Equipment attached by hand to what it cannot equip is taken off, and a seat at no life or with ten poison counters loses.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::refs::{Ref, Selector};
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::CounterKind;
use mtg_core::ids::ObjectId;
use mtg_engine::GameResult;
use mtg_engine::Event;
use mtg_engine::manual::{CommonToken, ManualAction, ManualToken, ManualZone};
use mtg_engine::player::LossReason;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature, gain_life};
use mtg_testkit::manual_cards::manual_equipment;
use mtg_testkit::trigger_cards::{watcher, when};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::common::{name_of, pid};

struct Ids {
    watcher: ObjectId,
    fresh: ObjectId,
    spare: ObjectId,
    blade: ObjectId,
    land: ObjectId,
}

fn find(driver: &Driver, zone: Zone, name: &str) -> ObjectId {
    driver
        .state()
        .zone(zone)
        .iter()
        .copied()
        .find(|id| name_of(driver.state(), *id) == name)
        .unwrap_or_else(|| panic!("{name} is not in {zone:?}"))
}

/// Seat 0 holds priority with `watcher` named "Watcher" on the battlefield beside a land and an Equipment.
fn game(watching: Arc<CardDefinition>) -> (Driver, Ids) {
    let driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(
                0,
                [free_creature("Fresh", 2, 2), free_creature("Spare", 1, 1)],
            )
            .library(0, vec![forest(); 8])
            .library(1, vec![forest(); 8])
            .battlefield(
                0,
                [
                    ready(watching),
                    ready(manual_equipment("Blade")),
                    ready(forest()),
                ],
            ),
    )
    .unwrap();
    let ids = Ids {
        watcher: find(&driver, Zone::Battlefield, "Watcher"),
        fresh: find(&driver, Zone::Hand(pid(0)), "Fresh"),
        spare: find(&driver, Zone::Hand(pid(0)), "Spare"),
        blade: find(&driver, Zone::Battlefield, "Blade"),
        land: find(&driver, Zone::Battlefield, "Forest"),
    };
    (driver, ids)
}

/// Both seats pass until the stack is empty or the game is over.
fn settle(driver: &mut Driver) {
    while !driver.state().zone(Zone::Stack).is_empty() && driver.engine().decision().is_some() {
        driver.pass().unwrap();
    }
}

fn life(driver: &Driver) -> i32 {
    driver.state().player(pid(0)).unwrap().life()
}

fn dies_for_three() -> Arc<CardDefinition> {
    watcher("Watcher", when::this_dies(), vec![gain_life(3)])
}

fn arrivals_for_one() -> Arc<CardDefinition> {
    watcher("Watcher", when::any_creature_enters(), vec![gain_life(1)])
}

fn draws_for_one() -> Arc<CardDefinition> {
    watcher(
        "Watcher",
        TriggerPattern::Drew { who: Ref::You },
        vec![gain_life(1)],
    )
}

fn discards_for_two() -> Arc<CardDefinition> {
    watcher(
        "Watcher",
        TriggerPattern::Discarded {
            what: Selector::default(),
            by: Ref::You,
        },
        vec![gain_life(2)],
    )
}

fn nothing_watched() -> Arc<CardDefinition> {
    free_creature("Watcher", 1, 1)
}

#[rstest]
#[case::a_creature_put_into_the_graveyard_dies(dies_for_three,
    |i: &Ids| ManualAction::Move { object: i.watcher, to: ManualZone::Graveyard }, 23)]
#[case::a_creature_exiled_does_not_die(dies_for_three,
    |i: &Ids| ManualAction::Move { object: i.watcher, to: ManualZone::Exile }, 20)]
#[case::a_creature_returned_to_the_hand_does_not_die(dies_for_three,
    |i: &Ids| ManualAction::Move { object: i.watcher, to: ManualZone::Hand }, 20)]
#[case::a_creature_with_no_toughness_left_dies(dies_for_three,
    |i: &Ids| ManualAction::Counters { object: i.watcher, counter: CounterKind::MinusOneMinusOne, delta: 1 }, 23)]
#[case::a_card_put_onto_the_battlefield_arrives(arrivals_for_one,
    |i: &Ids| ManualAction::Move { object: i.fresh, to: ManualZone::Battlefield }, 21)]
#[case::a_token_arrives(arrivals_for_one,
    |_: &Ids| ManualAction::CreateToken { token: ManualToken::Common(CommonToken::Soldier), count: 2 }, 22)]
#[case::an_artifact_token_is_no_creature_arriving(arrivals_for_one,
    |_: &Ids| ManualAction::CreateToken { token: ManualToken::Common(CommonToken::Treasure), count: 1 }, 20)]
#[case::a_draw_is_watched(draws_for_one, |_: &Ids| ManualAction::Draw { count: 2 }, 22)]
#[case::a_card_moved_to_the_hand_is_no_draw(draws_for_one,
    |_: &Ids| ManualAction::MoveTop { count: 2, to: ManualZone::Hand }, 20)]
#[case::a_discard_is_watched(discards_for_two,
    |i: &Ids| ManualAction::Discard { object: i.spare }, 22)]
#[case::a_card_moved_to_the_graveyard_is_no_discard(discards_for_two,
    |i: &Ids| ManualAction::Move { object: i.spare, to: ManualZone::Graveyard }, 20)]
fn a_manual_action_triggers_what_watches_the_events_it_makes(
    #[case] watching: fn() -> Arc<CardDefinition>,
    #[case] action: fn(&Ids) -> ManualAction,
    #[case] life_after: i32,
) {
    let (mut driver, ids) = game(watching());
    driver.manual(action(&ids)).unwrap();
    settle(&mut driver);
    assert_eq!(life(&driver), life_after);
}

#[test]
fn a_creature_that_died_by_hand_is_in_the_graveyard_when_its_trigger_resolves() {
    let (mut driver, ids) = game(dies_for_three());
    driver
        .manual(ManualAction::Move {
            object: ids.watcher,
            to: ManualZone::Graveyard,
        })
        .unwrap();
    assert_eq!(
        driver.state().object(ids.watcher).unwrap().zone(),
        Zone::Graveyard(pid(0))
    );
    assert_eq!(driver.state().zone(Zone::Stack).len(), 1);
}

#[test]
fn an_equipment_attached_by_hand_to_what_it_cannot_equip_is_taken_off() {
    let (mut driver, ids) = game(nothing_watched());
    let events = driver
        .manual(ManualAction::Attach {
            object: ids.blade,
            to: ids.land,
        })
        .unwrap();
    assert!(events
        .iter()
        .any(|e| matches!(e, Event::Attached { object, .. } if *object == ids.blade)));
    assert!(events
        .iter()
        .any(|e| matches!(e, Event::Unattached { object } if *object == ids.blade)));
    assert_eq!(driver.state().host_of(ids.blade), None);
}

#[rstest]
#[case::no_life_left(ManualAction::Life { player: pid(0), delta: -20 }, LossReason::ZeroLife)]
#[case::less_than_no_life(ManualAction::Life { player: pid(0), delta: -25 }, LossReason::ZeroLife)]
#[case::ten_poison_counters(
    ManualAction::PlayerCounters { player: pid(0), counter: CounterKind::Poison, delta: 10 },
    LossReason::Poison)]
fn a_seat_that_loses_by_a_manual_action_loses_the_game(
    #[case] action: ManualAction,
    #[case] reason: LossReason,
) {
    let (mut driver, _) = game(nothing_watched());
    driver.manual(action).unwrap();
    assert_eq!(
        driver.state().player(pid(0)).unwrap().status(),
        &mtg_engine::player::PlayerStatus::Lost(reason)
    );
    assert_eq!(driver.engine().result(), Some(GameResult::Winner(pid(1))));
    assert!(driver.engine().decision().is_none());
}

#[test]
fn drawing_from_an_empty_library_by_hand_loses_the_game() {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .library(0, Vec::<Arc<CardDefinition>>::new())
            .library(1, vec![forest(); 8]),
    )
    .unwrap();
    driver.manual(ManualAction::Draw { count: 1 }).unwrap();
    assert_eq!(driver.engine().result(), Some(GameResult::Winner(pid(1))));
}
