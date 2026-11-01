//! Goal: a seat searches its library and puts a card it looked through into its hand, the library shuffled after; ends the summoning sickness of a permanent it controls; sacrifices a permanent it controls so what watches dying and sacrifices triggers; and gives another seat control of a permanent it controls, or puts a card it owns from its graveyard or exile onto the battlefield under that seat's control, through the control layer; each is refused for what the seat may not do, and a rebuilt engine reaches the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::refs::{Ref, Selector};
use mtg_cards::trigger::TriggerPattern;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::characteristics::controller_of;
use mtg_engine::error::IllegalAnswer;
use mtg_engine::keywords::summoning_sick;
use mtg_engine::manual::{ManualAction, ManualRefusal, ManualZone};
use mtg_engine::sight::known_top;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, EngineError};
use mtg_testkit::cards::{forest, free_creature, gain_life, instant};
use mtg_testkit::trigger_cards::{watcher, when};
use mtg_testkit::{Driver, Scenario, ScriptError, ready, sick};
use rstest::rstest;

use crate::common::{name_of, pid};

/// Seat 0 holds priority with a dying watcher, a sacrifice watcher and a sick creature on the battlefield, a creature
/// and an instant in its graveyard, a card in hand and a library of A and B over forests; seat 1 has a creature.
fn game() -> Driver {
    Driver::from_scenario(
        Scenario::new(2)
            .battlefield(
                0,
                [
                    ready(watcher("Dier", when::this_dies(), vec![gain_life(3)])),
                    ready(watcher(
                        "Altar",
                        TriggerPattern::Sacrificed {
                            what: Selector::default(),
                            by: Ref::You,
                        },
                        vec![gain_life(1)],
                    )),
                    sick(free_creature("Fresh", 2, 2)),
                ],
            )
            .graveyard(0, [free_creature("Dead", 3, 3), instant("Bolt", 0, &[])])
            .hand(0, [free_creature("Held", 1, 1)])
            .library(
                0,
                [
                    free_creature("A", 1, 1),
                    free_creature("B", 1, 1),
                    forest(),
                    forest(),
                ],
            )
            .battlefield(1, [ready(free_creature("Theirs", 1, 1))])
            .library(1, vec![forest(); 4]),
    )
    .unwrap()
}

fn id(driver: &Driver, name: &str) -> ObjectId {
    driver
        .state()
        .objects()
        .find(|object| object.card().name == name)
        .unwrap_or_else(|| panic!("no {name}"))
        .id()
}

/// Takes the action as seat 0, settles the stack and checks that a rebuilt engine reaches the same state.
fn take(driver: &mut Driver, action: ManualAction) {
    driver.manual(action).unwrap();
    loop {
        if let Ok(options) = driver.trigger_options() {
            let order: Vec<usize> = (0..options.triggers.len()).collect();
            driver.order_triggers(&order).unwrap();
        } else if driver.state().zone(Zone::Stack).is_empty() {
            break;
        } else {
            driver.pass().unwrap();
        }
    }
    let rebuilt = Engine::resume(
        driver.engine().setup().clone(),
        driver.engine().events().to_vec(),
    )
    .unwrap();
    assert_eq!(rebuilt.state_hash(), driver.state().state_hash());
}

fn refusal(driver: &mut Driver, action: ManualAction) -> ManualRefusal {
    let hash = driver.state().state_hash();
    let error = driver.manual(action).unwrap_err();
    assert_eq!(driver.state().state_hash(), hash);
    let ScriptError::Engine(EngineError::Illegal(IllegalAnswer::Manual(refusal))) = error else {
        panic!("not a manual refusal: {error:?}");
    };
    refusal
}

fn life(driver: &Driver) -> i32 {
    driver.state().player(pid(0)).unwrap().life()
}

fn hand(driver: &Driver) -> Vec<String> {
    let state = driver.state();
    let mut names: Vec<String> = state
        .zone(Zone::Hand(pid(0)))
        .iter()
        .map(|card| name_of(state, *card))
        .collect();
    names.sort();
    names
}

#[rstest]
#[case::a_tutor_names_a_card_not_looked_at(|d: &Driver| ManualAction::Tutor { object: id(d, "A") }, |_: &Driver| ManualRefusal::NotFound)]
#[case::a_tutor_names_a_card_outside_the_library(|d: &Driver| ManualAction::Tutor { object: id(d, "Held") }, |_: &Driver| ManualRefusal::NotFound)]
#[case::a_creature_that_is_not_sick(|d: &Driver| ManualAction::RemoveSickness { object: id(d, "Dier") }, |_: &Driver| ManualRefusal::NoChange)]
#[case::sickness_of_another_seats_creature(|d: &Driver| ManualAction::RemoveSickness { object: id(d, "Theirs") }, |_: &Driver| ManualRefusal::NotYours)]
#[case::sickness_of_a_card_in_hand(|d: &Driver| ManualAction::RemoveSickness { object: id(d, "Held") },
    |d: &Driver| ManualRefusal::WrongZone { object: id(d, "Held"), zone: Zone::Hand(pid(0)) })]
#[case::a_sacrifice_of_another_seats_creature(|d: &Driver| ManualAction::Sacrifice { object: id(d, "Theirs") }, |_: &Driver| ManualRefusal::NotYours)]
#[case::a_sacrifice_from_the_graveyard(|d: &Driver| ManualAction::Sacrifice { object: id(d, "Dead") },
    |d: &Driver| ManualRefusal::WrongZone { object: id(d, "Dead"), zone: Zone::Graveyard(pid(0)) })]
#[case::a_sacrifice_of_no_object(|_: &Driver| ManualAction::Sacrifice { object: ObjectId(9999) }, |_: &Driver| ManualRefusal::Unknown(ObjectId(9999)))]
#[case::control_to_the_seat_itself(|d: &Driver| ManualAction::GiveControl { object: id(d, "Dier"), to: pid(0) }, |_: &Driver| ManualRefusal::NotAnotherSeat)]
#[case::control_to_no_seat(|d: &Driver| ManualAction::GiveControl { object: id(d, "Dier"), to: PlayerId(7) }, |_: &Driver| ManualRefusal::NotAnotherSeat)]
#[case::control_of_another_seats_creature(|d: &Driver| ManualAction::GiveControl { object: id(d, "Theirs"), to: pid(1) }, |_: &Driver| ManualRefusal::NotYours)]
#[case::an_instant_onto_the_battlefield(|d: &Driver| ManualAction::GiveControl { object: id(d, "Bolt"), to: pid(1) },
    |_: &Driver| ManualRefusal::NoDestination(ManualZone::Battlefield))]
#[case::control_of_a_card_in_hand(|d: &Driver| ManualAction::GiveControl { object: id(d, "Held"), to: pid(1) },
    |d: &Driver| ManualRefusal::WrongZone { object: id(d, "Held"), zone: Zone::Hand(pid(0)) })]
fn what_the_seat_may_not_do_is_refused(
    #[case] action: fn(&Driver) -> ManualAction,
    #[case] expected: fn(&Driver) -> ManualRefusal,
) {
    let mut driver = game();
    let (action, expected) = (action(&driver), expected(&driver));
    assert_eq!(refusal(&mut driver, action), expected);
}

#[rstest]
#[case::the_top_card_looked_at(ManualAction::LookTop { count: 1 }, "A")]
#[case::a_card_deep_in_a_search(ManualAction::Search {}, "B")]
fn a_card_looked_at_is_put_into_the_hand_and_the_library_shuffled(
    #[case] look: ManualAction,
    #[case] name: &str,
) {
    let mut driver = game();
    take(&mut driver, look);
    let card = id(&driver, name);
    take(&mut driver, ManualAction::Tutor { object: card });
    assert_eq!(hand(&driver), [name, "Held"]);
    assert_eq!(driver.state().zone(Zone::Library(pid(0))).len(), 3);
    assert_eq!(known_top(driver.state(), pid(0)), Vec::<ObjectId>::new());
}

#[test]
fn a_search_shows_the_whole_library_to_the_seat_alone() {
    let mut driver = game();
    take(&mut driver, ManualAction::Search {});
    let library: Vec<ObjectId> = driver
        .state()
        .zone(Zone::Library(pid(0)))
        .iter()
        .rev()
        .copied()
        .collect();
    assert_eq!(known_top(driver.state(), pid(0)), library);
    assert!(
        library
            .iter()
            .all(|card| !driver.state().is_shown_to(pid(1), *card))
    );
    let bottom = library[3];
    take(&mut driver, ManualAction::Tutor { object: bottom });
    assert_eq!(hand(&driver), ["Forest", "Held"]);
}

#[test]
fn an_empty_library_is_not_searched() {
    let mut driver = Driver::from_scenario(Scenario::new(2).library(0, [forest()])).unwrap();
    take(
        &mut driver,
        ManualAction::MoveTop {
            count: 1,
            to: ManualZone::Hand,
        },
    );
    assert_eq!(
        refusal(&mut driver, ManualAction::Search {}),
        ManualRefusal::NotEnough("cards in the library")
    );
}

#[test]
fn a_creature_without_sickness_may_attack_until_it_changes_control() {
    let mut driver = game();
    let fresh = id(&driver, "Fresh");
    assert!(summoning_sick(driver.state(), fresh));
    take(&mut driver, ManualAction::RemoveSickness { object: fresh });
    assert!(!summoning_sick(driver.state(), fresh));
    take(
        &mut driver,
        ManualAction::GiveControl {
            object: fresh,
            to: pid(1),
        },
    );
    assert!(summoning_sick(driver.state(), fresh));
}

#[rstest]
#[case::the_dying_watcher_dies_and_is_sacrificed("Dier", 24)]
#[case::the_sacrifice_watcher_sees_its_own_sacrifice("Altar", 21)]
#[case::a_plain_creature_is_seen_by_the_sacrifice_watcher("Fresh", 21)]
fn a_sacrifice_triggers_what_watches_dying_and_sacrifices(#[case] name: &str, #[case] after: i32) {
    let mut driver = game();
    let object = id(&driver, name);
    take(&mut driver, ManualAction::Sacrifice { object });
    assert_eq!(
        driver.state().object(object).unwrap().zone(),
        Zone::Graveyard(pid(0))
    );
    assert_eq!(life(&driver), after);
}

#[rstest]
#[case::a_permanent_on_the_battlefield("Fresh", None)]
#[case::a_card_from_the_graveyard("Dead", None)]
#[case::a_card_from_exile("Dead", Some(ManualZone::Exile))]
fn another_seat_gains_control_and_the_owner_stays(
    #[case] name: &str,
    #[case] first: Option<ManualZone>,
) {
    let mut driver = game();
    let object = id(&driver, name);
    if let Some(to) = first {
        take(&mut driver, ManualAction::Move { object, to });
    }
    take(
        &mut driver,
        ManualAction::GiveControl { object, to: pid(1) },
    );
    let held = driver.state().object(object).unwrap();
    assert_eq!(held.zone(), Zone::Battlefield);
    assert_eq!(held.owner(), pid(0));
    assert_eq!(controller_of(driver.state(), object), Some(pid(1)));
    assert_eq!(
        refusal(&mut driver, ManualAction::Sacrifice { object }),
        ManualRefusal::NotYours
    );
}
