//! Goal: champion (a creature that exiles another creature of a type as it enters or is sacrificed, and gives the card back when it leaves) exiles the chosen creature for as long as the champion stays, returns it when the champion leaves, and sacrifices the champion when the player declines or has no creature to exile; at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::effects::{Duration, Modification};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, plains};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

const TRIGGER: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[
{"Choose":{"from":{"Each":[{"Subtype":"Goblin"},"Other",{"ControlledBy":"You"}]},"count":{"min":0,"max":1},"as":"champion"}},
{"Exile":{"what":{"Bound":"champion"},"duration":"WhileSourceOnBattlefield"}},
{"If":{"condition":{"Not":"Performed"},"then":[{"Sacrifice":{"filter":["This"]}}]}}]}}"#;

fn mob() -> Arc<CardDefinition> {
    let card = (*free_creature("Mob", 3, 3)).clone();
    Arc::new(card.with_rules(serde_json::from_str::<Ability>(TRIGGER).unwrap()))
}

fn goblin() -> Arc<CardDefinition> {
    let card = (*free_creature("Goblin", 1, 1)).clone();
    Arc::new(card.with_subtype("Goblin"))
}

fn table(seats: usize, goblins: bool) -> Driver {
    let mut scenario = Scenario::new(seats).hand(0, vec![mob()]);
    if goblins {
        scenario = scenario.battlefield(0, vec![ready(goblin())]);
    }
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..16).map(|_| plains()));
    }
    Driver::from_scenario(scenario).unwrap()
}

/// Casts the mob and passes until the question about the exile stands or the stack is empty; whether it was asked.
fn cast(driver: &mut Driver) -> bool {
    driver.cast("Mob").unwrap();
    for _ in 0..16 {
        if matches!(
            driver.decision().unwrap().kind,
            DecisionKind::ChooseCards(_)
        ) {
            return true;
        }
        if driver.state().stack().is_empty() {
            return false;
        }
        driver.pass().unwrap();
    }
    panic!("the mob neither resolved nor asked");
}

fn settle(driver: &mut Driver) {
    for _ in 0..16 {
        if driver.state().stack().is_empty() {
            return;
        }
        driver.pass().unwrap();
    }
}

fn in_zone(driver: &Driver, name: &str, zone: Zone) -> bool {
    driver.state().zone(zone).iter().any(|id| {
        driver
            .state()
            .object(*id)
            .is_some_and(|object| object.card().name == name)
    })
}

#[rstest]
fn exiling_the_creature_keeps_the_champion(#[values(2, 3, 4)] seats: usize) {
    let mut driver = table(seats, true);
    assert!(cast(&mut driver));
    let goblin = driver.id_of("Goblin").unwrap();
    driver
        .answer(Answer::ChooseCards(CardsChoice(vec![goblin])))
        .unwrap();
    settle(&mut driver);
    assert!(in_zone(&driver, "Mob", Zone::Battlefield));
    assert!(in_zone(&driver, "Goblin", Zone::Exile));
}

#[rstest]
fn declining_sacrifices_the_champion(#[values(2, 3)] seats: usize) {
    let mut driver = table(seats, true);
    assert!(cast(&mut driver));
    driver
        .answer(Answer::ChooseCards(CardsChoice(Vec::new())))
        .unwrap();
    settle(&mut driver);
    assert!(in_zone(
        &driver,
        "Mob",
        Zone::Graveyard(mtg_core::ids::PlayerId(0))
    ));
    assert!(in_zone(&driver, "Goblin", Zone::Battlefield));
}

#[rstest]
fn the_exiled_creature_returns_when_the_champion_leaves(#[values(2, 3, 4)] seats: usize) {
    let mut driver = table(seats, true);
    assert!(cast(&mut driver));
    let goblin = driver.id_of("Goblin").unwrap();
    driver
        .answer(Answer::ChooseCards(CardsChoice(vec![goblin])))
        .unwrap();
    settle(&mut driver);
    assert!(in_zone(&driver, "Goblin", Zone::Exile));
    let mob = driver.id_of("Mob").unwrap();
    driver
        .add_effect(
            driver.on(mob),
            Modification::ModifyPowerToughness {
                power: 0,
                toughness: -3,
            },
            Duration::UntilEndOfTurn,
        )
        .unwrap();
    settle(&mut driver);
    assert!(!in_zone(&driver, "Mob", Zone::Battlefield));
    assert!(in_zone(&driver, "Goblin", Zone::Battlefield));
    assert!(!in_zone(&driver, "Goblin", Zone::Exile));
}

#[rstest]
fn with_no_creature_to_exile_the_champion_is_sacrificed(#[values(2, 3)] seats: usize) {
    let mut driver = table(seats, false);
    cast(&mut driver);
    settle(&mut driver);
    assert!(in_zone(
        &driver,
        "Mob",
        Zone::Graveyard(mtg_core::ids::PlayerId(0))
    ));
}
