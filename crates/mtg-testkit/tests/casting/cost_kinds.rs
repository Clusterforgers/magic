//! Goal: a cost that names a kind of object ("Discard a land card", "Sacrifice a Cleric creature", "Sacrifice a Swamp and a Forest") offers only what matches, one pick for each part, and pays it with the activation, at 2, 3 and 4 seats; an ability with nothing to give is not offered; the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common;

use common::{name_of, pid};
use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{forest, free_creature, island, swamp};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A permanent named Altar whose ability costs `cost` and gains 1 life.
fn altar(cost: &str) -> mtg_engine::setup::PresetPermanent {
    let ability =
        format!(r#"{{"Activated":{{"cost":{cost},"effects":[{{"GainLife":{{"amount":1}}}}]}}}}"#);
    ready(Arc::new(
        CardDefinition::new("Altar")
            .with_type(CardType::Artifact)
            .with_rules(serde_json::from_str(&ability).unwrap()),
    ))
}

fn cleric(name: &str) -> Arc<CardDefinition> {
    Arc::new((*free_creature(name, 1, 1)).clone().with_subtype("Cleric"))
}

fn id_in(driver: &Driver, zone: Zone, name: &str) -> ObjectId {
    *driver
        .state()
        .zone(zone)
        .iter()
        .find(|id| name_of(driver.state(), **id) == name)
        .unwrap_or_else(|| panic!("no {name} in {zone:?}"))
}

/// The cards the pending pick offers, by name.
fn offered(driver: &Driver) -> Vec<String> {
    let DecisionKind::ChooseCards(options) = &driver.decision().unwrap().kind else {
        panic!("a pick was expected");
    };
    let mut names: Vec<String> = options
        .from
        .iter()
        .map(|id| name_of(driver.state(), *id))
        .collect();
    names.sort();
    names
}

fn table(seats: usize, scenario: Scenario) -> Scenario {
    (1..seats).fold(scenario, |scenario, seat| {
        scenario.library(seat, (0..8).map(|_| forest()))
    })
}

fn life(driver: &Driver) -> i32 {
    driver.state().player(pid(0)).unwrap().life()
}

fn pick(driver: &mut Driver, zone: Zone, name: &str) -> Result<Vec<Event>, ScriptError> {
    let id = id_in(driver, zone, name);
    driver.answer(Answer::ChooseCards(CardsChoice(vec![id])))
}

const LAND_CARD: &str = r#"[{"Discard":{"filter":[{"Type":"Land"}]}}]"#;
const CLERIC: &str = r#"[{"Sacrifice":{"filter":[{"Type":"Creature"},{"Subtype":"Cleric"}]}}]"#;

#[rstest]
fn a_discard_of_a_kind_offers_only_that_kind(#[values(2, 3, 4)] seats: usize) {
    let scenario = table(
        seats,
        Scenario::new(seats)
            .battlefield(0, [altar(LAND_CARD)])
            .hand(0, [island(), free_creature("Bear", 2, 2), forest()]),
    );
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    let before = life(&driver);
    driver.begin_activation("Altar").unwrap();
    assert_eq!(offered(&driver), ["Forest", "Island"]);
    pick(&mut driver, Zone::Hand(pid(0)), "Island").unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    assert_eq!(life(&driver), before + 1);
    assert!(
        driver
            .state()
            .zone(Zone::Graveyard(pid(0)))
            .iter()
            .any(|id| name_of(driver.state(), *id) == "Island")
    );
    assert_eq!(
        replay(&scenario.setup().unwrap(), driver.engine().events())
            .unwrap()
            .state_hash(),
        driver.state().state_hash()
    );
}

#[rstest]
fn a_discard_of_a_kind_cannot_be_paid_without_the_kind(#[values(2, 3, 4)] seats: usize) {
    let scenario = table(
        seats,
        Scenario::new(seats)
            .battlefield(0, [altar(LAND_CARD)])
            .hand(0, [free_creature("Bear", 2, 2)]),
    );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    assert!(driver.begin_activation("Altar").is_err());
}

#[rstest]
fn a_sacrifice_by_subtype_and_type_offers_only_the_creatures_that_match(
    #[values(2, 3, 4)] seats: usize,
) {
    let scenario = table(
        seats,
        Scenario::new(seats).battlefield(
            0,
            [
                altar(CLERIC),
                ready(cleric("Acolyte")),
                ready(cleric("Deacon")),
                ready(free_creature("Bear", 2, 2)),
            ],
        ),
    );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_activation("Altar").unwrap();
    assert_eq!(offered(&driver), ["Acolyte", "Deacon"]);
    pick(&mut driver, Zone::Battlefield, "Deacon").unwrap();
    assert!(
        driver
            .state()
            .zone(Zone::Graveyard(pid(0)))
            .iter()
            .any(|id| name_of(driver.state(), *id) == "Deacon")
    );
}

#[rstest]
fn a_run_of_sacrifices_asks_one_pick_for_each_part(#[values(2, 3, 4)] seats: usize) {
    let cost = r#"[{"Sacrifice":{"filter":[{"Subtype":"Swamp"}]}},{"Sacrifice":{"filter":[{"Subtype":"Forest"}]}}]"#;
    let scenario = table(
        seats,
        Scenario::new(seats).battlefield(
            0,
            [
                altar(cost),
                ready(swamp()),
                ready(swamp()),
                ready(forest()),
                ready(island()),
            ],
        ),
    );
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    let before = life(&driver);
    driver.begin_activation("Altar").unwrap();
    assert_eq!(offered(&driver), ["Swamp", "Swamp"]);
    pick(&mut driver, Zone::Battlefield, "Swamp").unwrap();
    // The one Forest is the only candidate for the second part, so it is not asked.
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    assert_eq!(life(&driver), before + 1);
    let gone = driver
        .state()
        .zone(Zone::Graveyard(pid(0)))
        .iter()
        .map(|id| name_of(driver.state(), *id))
        .collect::<Vec<_>>();
    assert!(gone.contains(&"Swamp".to_owned()) && gone.contains(&"Forest".to_owned()));
    assert_eq!(
        replay(&scenario.setup().unwrap(), driver.engine().events())
            .unwrap()
            .state_hash(),
        driver.state().state_hash()
    );
}

#[rstest]
fn a_run_of_sacrifices_is_not_offered_without_both_kinds(#[values(2, 3, 4)] seats: usize) {
    let cost = r#"[{"Sacrifice":{"filter":[{"Subtype":"Swamp"}]}},{"Sacrifice":{"filter":[{"Subtype":"Forest"}]}}]"#;
    let scenario = table(
        seats,
        Scenario::new(seats).battlefield(0, [altar(cost), ready(swamp()), ready(swamp())]),
    );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    assert!(driver.begin_activation("Altar").is_err());
}
