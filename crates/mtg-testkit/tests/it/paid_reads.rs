//! Goal: an effect that reads what its cost took ("the sacrificed creature's power", "the discarded card's mana value") reads the object as it was when the cost took it, with its counters and not as a card in the graveyard, for an ability and for a spell with a required additional cost, at 2, 3 and 4 seats; the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common;

use common::{name_of, pid};
use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_core::mana::ManaCost;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

const BEAR: &str = "Bear";

/// A 1/1 with two +1/+1 counters, a 3/3 whichever way it is read, and a plain 5/1 beside it so the pick is a question.
fn creatures() -> Vec<mtg_engine::setup::PresetPermanent> {
    vec![
        with_counters(
            ready(free_creature(BEAR, 1, 1)),
            CounterKind::PlusOnePlusOne,
            2,
        ),
        ready(free_creature("Ox", 5, 1)),
    ]
}

/// An artifact that costs `{2}`, an artifact that costs nothing, both on the battlefield.
fn artifacts() -> Vec<mtg_engine::setup::PresetPermanent> {
    let costly = CardDefinition::new("Gem")
        .with_type(CardType::Artifact)
        .with_mana_cost(ManaCost::parse("{2}").unwrap());
    let free = CardDefinition::new("Pebble").with_type(CardType::Artifact);
    vec![ready(Arc::new(costly)), ready(Arc::new(free))]
}

fn card_of_value(name: &str, cost: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Sorcery)
            .with_mana_cost(ManaCost::parse(cost).unwrap()),
    )
}

/// A permanent with this name and the ability `cost: You gain life equal to the <stat> of what the cost took.`
fn altar(cost: &str, stat: &str) -> mtg_engine::setup::PresetPermanent {
    let ability = format!(
        r#"{{"Activated":{{"cost":{cost},"effects":[{{"GainLife":{{"amount":{{"Stat":{{"of":"Paid","stat":"{stat}"}}}}}}}}]}}}}"#
    );
    ready(Arc::new(
        CardDefinition::new("Altar")
            .with_type(CardType::Artifact)
            .with_rules(serde_json::from_str(&ability).unwrap()),
    ))
}

fn id_in(driver: &Driver, zone: Zone, name: &str) -> ObjectId {
    *driver
        .state()
        .zone(zone)
        .iter()
        .find(|id| name_of(driver.state(), **id) == name)
        .unwrap_or_else(|| panic!("no {name} in {zone:?}"))
}

fn life(driver: &Driver) -> i32 {
    driver.state().player(pid(0)).unwrap().life()
}

/// Passes until nothing is on the stack.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    while !driver.state().stack().is_empty()
        && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
    {
        driver.pass()?;
    }
    Ok(())
}

fn table(seats: usize, scenario: Scenario) -> Scenario {
    (1..seats).fold(scenario, |scenario, seat| {
        scenario.library(seat, (0..8).map(|_| forest()))
    })
}

fn replays(driver: &Driver, scenario: Scenario) {
    let replayed = replay(&scenario.setup().unwrap(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

#[rstest]
#[case::power_of_a_sacrificed_creature(
    r#"[{"Sacrifice":{"filter":[{"Type":"Creature"}]}}]"#,
    "Power",
    Zone::Battlefield,
    BEAR,
    3
)]
#[case::toughness_of_a_sacrificed_creature(
    r#"[{"Sacrifice":{"filter":[{"Type":"Creature"}]}}]"#,
    "Toughness",
    Zone::Battlefield,
    BEAR,
    3
)]
#[case::mana_value_of_a_sacrificed_artifact(
    r#"[{"Sacrifice":{"filter":[{"Type":"Artifact"},"Other"]}}]"#,
    "ManaValue",
    Zone::Battlefield,
    "Gem",
    2
)]
#[case::mana_value_of_a_discarded_card(
    r#"[{"Discard":{}}]"#,
    "ManaValue",
    Zone::Hand(pid(0)),
    "Big",
    4
)]
fn an_ability_reads_the_object_its_cost_took_as_it_was(
    #[case] cost: &str,
    #[case] stat: &str,
    #[case] zone: Zone,
    #[case] name: &str,
    #[case] gained: i32,
    #[values(2, 3, 4)] seats: usize,
) {
    let scenario = table(
        seats,
        Scenario::new(seats)
            .battlefield(
                0,
                [vec![altar(cost, stat)], creatures(), artifacts()].concat(),
            )
            .hand(
                0,
                [
                    card_of_value("Big", "{3}{R}"),
                    card_of_value("Small", "{1}"),
                ],
            ),
    );
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    let before = life(&driver);
    driver.begin_activation("Altar").unwrap();
    let picked = id_in(&driver, zone, name);
    driver
        .answer(Answer::ChooseCards(CardsChoice(vec![picked])))
        .unwrap();
    settle(&mut driver).unwrap();
    assert_eq!(life(&driver) - before, gained);
    assert_ne!(
        driver.state().object(picked).map(|o| o.zone()),
        Some(zone),
        "the cost took it"
    );
    replays(&driver, scenario);
}

/// The sorcery "As an additional cost to cast this spell, sacrifice a creature. Draw cards equal to its power."
fn legacy() -> Arc<CardDefinition> {
    let cost = r#"{"AdditionalCost":{"cost":[{"Sacrifice":{"filter":[{"Type":"Creature"}]}}],"required":true}}"#;
    let spell =
        r#"{"Spell":{"effects":[{"Draw":{"count":{"Stat":{"of":"Paid","stat":"Power"}}}}]}}"#;
    Arc::new(
        CardDefinition::new("Legacy")
            .with_type(CardType::Sorcery)
            .with_mana_cost(ManaCost::parse("{1}").unwrap())
            .with_rules(serde_json::from_str(cost).unwrap())
            .with_rules(serde_json::from_str(spell).unwrap()),
    )
}

#[rstest]
fn a_spell_reads_the_creature_sacrificed_to_cast_it_as_it_was(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(
        seats,
        Scenario::new(seats)
            .library(0, (0..8).map(|_| forest()))
            .hand(0, [legacy()])
            .battlefield(0, [creatures(), vec![ready(forest())]].concat()),
    );
    let mut driver = Driver::from_scenario(scenario.clone())?;
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    driver.begin_cast("Legacy")?;
    let bear = id_in(&driver, Zone::Battlefield, BEAR);
    driver.answer(Answer::ChooseCards(CardsChoice(vec![bear])))?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver)?;
    // The spell left the hand and drew the power the creature had, 3, not the 1 it prints.
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))).len(), hand - 1 + 3);
    assert!(
        driver
            .engine()
            .events()
            .iter()
            .any(|event| matches!(event, Event::SpellResolved { .. }))
    );
    replays(&driver, scenario);
    Ok(())
}
