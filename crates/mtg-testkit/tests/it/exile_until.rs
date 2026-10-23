//! Goal: "exile target ... until this leaves the battlefield" plays whole at tables of two, three and four seats: the card waits in exile linked to its source and returns to the battlefield as a new object under its owner's control the moment the source leaves, a card is not exiled at all when the source left before the trigger resolved (CR 610.3c), an exiled token ceases to exist and nothing returns, and two sources on two cards return only their own.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::ObjectId;
use mtg_engine::Event;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, free_creature, mountain, plains};
use mtg_testkit::replace_cards::raise_soldiers;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{name_of, pid};

/// Seat 0 holds the cards named and six Plains; seat 1 holds Burn and Raise Soldiers and controls two Mountains, a
/// Plains, a Bear and an Ogre. The game stands in seat 0's first main phase.
fn table(seats: usize, hand: &[&str]) -> Result<Driver, ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand_named(0, hand.iter().copied())
        .battlefield(0, (0..6).map(|_| ready(plains())))
        .hand(1, [burn("Burn", 2), raise_soldiers().unwrap()])
        .battlefield(
            1,
            [
                ready(mountain()),
                ready(plains()),
                ready(free_creature("Bear", 2, 2)),
                ready(free_creature("Ogre", 3, 3)),
            ],
        );
    Driver::from_scenario(scenario)
}

/// Every seat passes once, so the top of the stack resolves.
fn resolve(driver: &mut Driver, seats: usize) {
    for _ in 0..seats {
        driver.pass().unwrap();
    }
}

/// Seat 0 casts the card and the card resolves; its trigger is put on the stack aimed at `target`, and it waits there.
fn cast_aimed(driver: &mut Driver, seats: usize, card: &str, target: &str) {
    driver.cast(card).unwrap();
    resolve(driver, seats);
    let aimed = driver.target_named(target).unwrap();
    driver.choose_targets(&[aimed]).unwrap();
}

/// Seat 1 gets priority with the stack as it is and burns the named permanent; the burn resolves.
fn burn_it(driver: &mut Driver, seats: usize, victim: &str) {
    driver.pass().unwrap();
    let aimed = driver.target_named(victim).unwrap();
    driver.cast_at("Burn", &[aimed]).unwrap();
    resolve(driver, seats);
}

fn exiled(driver: &Driver) -> Vec<ObjectId> {
    driver.state().zone(Zone::Exile).to_vec()
}

fn in_play(driver: &Driver, name: &str) -> bool {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .any(|id| name_of(driver.state(), *id) == name)
}

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn the_card_waits_in_exile_and_returns_as_a_new_object_when_the_source_leaves(
    #[case] seats: usize,
) {
    let mut driver = table(seats, &["Banisher Priest"]).unwrap();
    let bear = driver.id_of("Bear").unwrap();
    let before = driver.state().object(bear).unwrap().counter();
    cast_aimed(&mut driver, seats, "Banisher Priest", "Bear");
    resolve(&mut driver, seats);
    let priest = driver.id_of("Banisher Priest").unwrap();
    let held = driver.state().object(bear).unwrap();
    assert_eq!(held.zone(), Zone::Exile);
    assert_eq!(
        held.exiled_until(),
        Some(driver.state().object(priest).unwrap().reference())
    );
    burn_it(&mut driver, seats, "Banisher Priest");
    let back = driver.state().object(bear).unwrap();
    assert_eq!(back.zone(), Zone::Battlefield, "the Bear came back");
    assert!(back.counter() > before + 1, "as a new object");
    assert_eq!(back.owner(), pid(1));
    assert_eq!(back.controller(), pid(1));
    assert_eq!(back.exiled_until(), None);
    assert!(exiled(&driver).is_empty());
}

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn a_source_that_left_before_the_trigger_resolved_exiles_nothing(#[case] seats: usize) {
    let mut driver = table(seats, &["Banisher Priest"]).unwrap();
    let bear = driver.id_of("Bear").unwrap();
    let counter = driver.state().object(bear).unwrap().counter();
    cast_aimed(&mut driver, seats, "Banisher Priest", "Bear");
    burn_it(&mut driver, seats, "Banisher Priest");
    assert!(!in_play(&driver, "Banisher Priest"));
    resolve(&mut driver, seats);
    let bear = driver.state().object(bear).unwrap();
    assert_eq!(bear.zone(), Zone::Battlefield);
    assert_eq!(bear.counter(), counter, "the Bear never left");
    assert!(exiled(&driver).is_empty());
}

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn an_exiled_token_ceases_to_exist_and_nothing_returns(#[case] seats: usize) {
    let mut driver = table(seats, &["Banisher Priest"]).unwrap();
    driver.pass().unwrap();
    driver.cast("Raise Soldiers").unwrap();
    resolve(&mut driver, seats);
    cast_aimed(&mut driver, seats, "Banisher Priest", "Soldier");
    resolve(&mut driver, seats);
    assert!(exiled(&driver).is_empty(), "the token ceased to exist");
    let soldiers = |driver: &Driver| {
        driver
            .state()
            .zone(Zone::Battlefield)
            .iter()
            .filter(|id| name_of(driver.state(), **id) == "Soldier")
            .count()
    };
    assert_eq!(soldiers(&driver), 1);
    burn_it(&mut driver, seats, "Banisher Priest");
    assert_eq!(soldiers(&driver), 1, "no token came back");
    assert!(exiled(&driver).is_empty());
}

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn two_sources_return_only_their_own_card(#[case] seats: usize) {
    let mut driver = table(seats, &["Banisher Priest", "Banishing Light"]).unwrap();
    cast_aimed(&mut driver, seats, "Banisher Priest", "Bear");
    resolve(&mut driver, seats);
    cast_aimed(&mut driver, seats, "Banishing Light", "Ogre");
    resolve(&mut driver, seats);
    assert_eq!(exiled(&driver).len(), 2);
    burn_it(&mut driver, seats, "Banisher Priest");
    assert!(in_play(&driver, "Bear"), "the Priest's card is back");
    assert!(!in_play(&driver, "Ogre"), "the Light still holds its card");
    let ogre = driver.id_of("Ogre");
    assert!(ogre.is_err());
    assert_eq!(exiled(&driver).len(), 1);
}

#[test]
fn the_log_of_a_return_is_a_move_from_exile_to_the_battlefield() {
    let mut driver = table(2, &["Banisher Priest"]).unwrap();
    let bear = driver.id_of("Bear").unwrap();
    cast_aimed(&mut driver, 2, "Banisher Priest", "Bear");
    resolve(&mut driver, 2);
    driver.pass().unwrap();
    let aimed = driver.target_named("Banisher Priest").unwrap();
    driver.cast_at("Burn", &[aimed]).unwrap();
    driver.pass().unwrap();
    let events = driver.pass().unwrap();
    assert!(
        events.iter().any(|event| matches!(
            event,
            Event::ZoneChanged { object, from: Zone::Exile, to: Zone::Battlefield, .. } if *object == bear
        )),
        "{events:?}"
    );
}
