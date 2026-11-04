//! Goal: a creature spell paid with mana that says "that spell can't be countered" (Cavern of Souls) resolves past a counterspell, a spell paid otherwise is countered, and the log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::mana::{ManaCost, ManaType};
use mtg_engine::choose::{ChoiceOption, OptionChoice, Target};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::replays;

const ABILITIES: [&str; 3] = [
    r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"Choose":"CreatureType"}}}"#,
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless"]}}}]}}"#,
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"AnyColor":1},"only":[
        {"Spell":{"types":["Creature"],"chosen_type":true}},
        {"Uncounterable":{"types":["Creature"],"chosen_type":true}}]}}]}}"#,
];

/// Cavern of Souls as the translator writes it.
fn cavern() -> Arc<CardDefinition> {
    let mut card = CardDefinition::new("Cavern of Souls").with_type(CardType::Land);
    for ability in ABILITIES {
        card = card.with_rules(serde_json::from_str::<Ability>(ability).unwrap());
    }
    Arc::new(card)
}

fn creature(name: &str, subtype: &str, cost: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_subtype(subtype)
            .with_power_toughness(2, 2)
            .with_mana_cost(ManaCost::parse(cost).unwrap()),
    )
}

/// Seat 0 has played Cavern of Souls naming Elf and holds Elf Scout, Goblin Scout and a Forest on the battlefield; seat 1 holds
/// Counterspell and two Islands.
fn table(seats: usize, spell: Arc<CardDefinition>) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .hand(0, [cavern(), spell])
        .battlefield(0, [ready(forest()), ready(forest())])
        .hand(1, [card("Counterspell").unwrap()])
        .battlefield(1, [ready(island()), ready(island())]);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..16).map(|_| forest()));
    }
    scenario
}

fn start(seats: usize, spell: Arc<CardDefinition>) -> Result<Driver, ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, spell))?;
    driver.play_land("Cavern of Souls")?;
    let DecisionKind::ChooseOption(options) = driver.decision()?.kind.clone() else {
        panic!("a creature type is chosen");
    };
    let at = options
        .options
        .iter()
        .position(|o| matches!(o, ChoiceOption::Label(label) if label == "Elf"))
        .unwrap();
    driver.answer(Answer::ChooseOption(OptionChoice(at)))?;
    Ok(driver)
}

/// Seat 0 casts `name` tapping `lands`; seat 1 counters it; the stack resolves. Whether the spell is on the battlefield.
fn countered_cast_resolves(driver: &mut Driver, name: &str, lands: &[&str]) -> bool {
    for land in lands {
        driver.tap_for_mana_of(land, ManaType::Green).unwrap();
    }
    driver.cast(name).unwrap();
    let spell = driver.state().stack()[0];
    let at = Target::Object(driver.state().object(spell).unwrap().reference());
    driver.pass().unwrap();
    driver.cast_at("Counterspell", &[at]).unwrap();
    for _ in 0..12 {
        if driver.state().stack().is_empty() {
            break;
        }
        driver.pass().unwrap();
    }
    assert!(driver.state().stack().is_empty());
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .any(|id| driver.state().object(*id).unwrap().card().name == name)
}

#[rstest]
#[case::paid_with_the_cavern(&["Cavern of Souls"], true)]
#[case::paid_with_a_forest(&["Forest"], false)]
fn a_creature_of_the_chosen_type_is_uncounterable_only_when_the_cavern_paid(
    #[values(2, 3, 4)] seats: usize,
    #[case] lands: &[&str],
    #[case] resolves: bool,
) -> Result<(), ScriptError> {
    let spell = creature("Elf Scout", "Elf", "{G}");
    let mut driver = start(seats, spell)?;
    assert_eq!(
        countered_cast_resolves(&mut driver, "Elf Scout", lands),
        resolves
    );
    Ok(())
}

#[rstest]
fn one_mana_of_the_cavern_among_several_is_enough(
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = start(seats, creature("Elf Brute", "Elf", "{1}{G}"))?;
    assert!(countered_cast_resolves(
        &mut driver,
        "Elf Brute",
        &["Cavern of Souls", "Forest"]
    ));
    Ok(())
}

#[test]
fn the_log_of_a_protected_cast_replays() -> Result<(), ScriptError> {
    let spell = creature("Elf Scout", "Elf", "{G}");
    let setup = table(2, spell.clone()).setup().unwrap();
    let mut driver = start(2, spell)?;
    assert!(countered_cast_resolves(
        &mut driver,
        "Elf Scout",
        &["Cavern of Souls"]
    ));
    replays(&driver, &setup);
    Ok(())
}
