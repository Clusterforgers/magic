//! Goal: a static ability that gives a supertype ("nonland permanents are legendary", "enchanted land is snow") makes the objects it affects have it at 2 to 4 seats: a selector of legendary creatures sees it, the legend rule applies to two permanents it made legendary, the mana of a land it made snow is snow mana, and nothing else is changed.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::{CardType, Supertype};
use mtg_core::ids::ObjectId;
use mtg_engine::characteristics;
use mtg_engine::decision::DecisionKind;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, creature, forest};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// An enchantment with one static ability (JSON).
fn enchantment(name: &str, rules: &str) -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_str(rules).unwrap();
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Enchantment)
            .with_mana_cost(cost(1, &[]))
            .with_rules(ability),
    )
}

/// "Creatures are legendary."
fn singular() -> Arc<CardDefinition> {
    enchantment(
        "Singular",
        r#"{"Static":{"affected":{"Each":[{"Type":"Creature"}]},"mods":[{"AddSupertype":"Legendary"}]}}"#,
    )
}

/// "Legendary creatures get +1/+1."
fn proud() -> Arc<CardDefinition> {
    enchantment(
        "Proud",
        r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"Supertype":"Legendary"}]},"mods":[{"ModifyPT":{"power":1,"toughness":1}}]}}"#,
    )
}

/// "Lands you control are snow."
fn frost() -> Arc<CardDefinition> {
    enchantment(
        "Frost",
        r#"{"Static":{"affected":{"Each":[{"Type":"Land"},{"ControlledBy":"You"}]},"mods":[{"AddSupertype":"Snow"}]}}"#,
    )
}

fn bears() -> Arc<CardDefinition> {
    creature("Bears", 0, &[], 2, 2)
}

fn named(driver: &Driver, name: &str, zone: Zone) -> Vec<ObjectId> {
    driver
        .state()
        .zone(zone)
        .iter()
        .copied()
        .filter(|id| driver.state().object(*id).unwrap().card().name == name)
        .collect()
}

#[rstest]
fn a_selector_of_legendary_creatures_sees_the_given_supertype(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] given: bool,
) {
    let mut permanents = vec![ready(proud()), ready(bears())];
    if given {
        permanents.push(ready(singular()));
    }
    let driver = Driver::from_scenario(Scenario::new(seats).battlefield(0, permanents)).unwrap();
    let bear = named(&driver, "Bears", Zone::Battlefield)[0];
    let chars = characteristics(driver.state(), bear).unwrap();
    assert_eq!(chars.power, Some(if given { 3 } else { 2 }));
    assert_eq!(chars.supertypes.contains(&Supertype::Legendary), given);
    // The enchantment is not a creature, so it stays as it is.
    let proud = named(&driver, "Proud", Zone::Battlefield)[0];
    assert_eq!(
        characteristics(driver.state(), proud).unwrap().supertypes,
        Vec::<Supertype>::new()
    );
}

#[rstest]
fn the_legend_rule_applies_to_permanents_made_legendary(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] given: bool,
) {
    let mut permanents = vec![ready(bears())];
    if given {
        permanents.push(ready(singular()));
    }
    let scenario = Scenario::new(seats)
        .battlefield(0, permanents)
        .hand(0, vec![bears()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Bears").unwrap();
    for _ in 0..seats {
        driver.pass().unwrap();
    }
    // The legend rule asks which one to keep; any answer keeps one.
    for _ in 0..4 {
        if matches!(driver.decision().unwrap().kind, DecisionKind::Priority(_)) {
            break;
        }
        driver.answer_passively().unwrap();
    }
    let kept = named(&driver, "Bears", Zone::Battlefield).len();
    assert_eq!(kept, if given { 1 } else { 2 });
}

#[rstest]
fn a_land_made_snow_has_snow_supertype(#[values(2, 3, 4)] seats: usize) {
    let driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(0, vec![ready(frost()), ready(forest())])
            .battlefield(1, vec![ready(forest())]),
    )
    .unwrap();
    let mine = named(&driver, "Forest", Zone::Battlefield);
    let snow: Vec<bool> = mine
        .iter()
        .map(|id| {
            characteristics(driver.state(), *id)
                .unwrap()
                .supertypes
                .contains(&Supertype::Snow)
        })
        .collect();
    let owners: Vec<bool> = mine
        .iter()
        .map(|id| driver.state().object(*id).unwrap().owner().0 == 0)
        .collect();
    assert_eq!(snow, owners);
}
