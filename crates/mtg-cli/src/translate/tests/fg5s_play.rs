//! Goal: a static line that refines its defined object (`AffectedDefined$ Self | Affected$ Card.attacking`) plays through
//! the engine as a bonus that holds only while the object matches the refinement.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::turn::Step;
use mtg_testkit::cards::free_creature;
use mtg_testkit::scenario::attached_to;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use super::abilities;

fn size(driver: &Driver, name: &str) -> (i32, i32) {
    let id = driver.id_of(name).unwrap();
    let chars = mtg_engine::characteristics::characteristics(driver.state(), id).unwrap();
    (chars.power.unwrap(), chars.toughness.unwrap())
}

/// A card with the abilities a Forge script translates to.
fn translated(script: &str, ty: CardType, stats: Option<(i32, i32)>) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new("Subject").with_type(ty);
    if script.contains("Aura") {
        card = card.with_subtype("Aura");
    }
    if let Some((power, toughness)) = stats {
        card = card.with_power_toughness(power, toughness);
    }
    for value in abilities(script) {
        card = card.with_rules(serde_json::from_value::<Ability>(value).unwrap());
    }
    Arc::new(card)
}

#[test]
fn a_creature_gets_the_bonus_only_while_it_matches_its_own_refinement() {
    let text = "Name:Subject\nTypes:Creature Elf\nPT:2/2\nS:Mode$ Continuous | AffectedDefined$ Self | Affected$ Card.attacking | AddPower$ 3 | Description$ x\n";
    let scenario = Scenario::new(2).at(1, 0, Step::Main1).battlefield(
        0,
        [ready(translated(text, CardType::Creature, Some((2, 2))))],
    );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    assert_eq!(size(&driver, "Subject"), (2, 2));
    driver.pass_until(Step::DeclareAttackers).unwrap();
    driver.attack(&[("Subject", 1)]).unwrap();
    assert_eq!(size(&driver, "Subject"), (5, 2));
}

#[rstest]
#[case::matching_host(4, (6, 4))]
#[case::other_host(2, (2, 2))]
fn an_aura_bonus_with_a_refinement_applies_to_the_enchanted_creature_only_when_it_matches(
    #[case] host_power: i32,
    #[case] expected: (i32, i32),
) {
    let text = "Name:Mantle\nTypes:Enchantment Aura\nK:Enchant:Creature\nS:Mode$ Continuous | AffectedDefined$ Enchanted | Affected$ Creature.powerGE3 | AddPower$ 2 | Description$ x\n";
    let aura = attached_to(ready(translated(text, CardType::Enchantment, None)), 0, 0);
    let host = free_creature("Host", host_power, host_power);
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(host), aura]);
    let driver = Driver::from_scenario(scenario).unwrap();
    assert_eq!(size(&driver, "Host"), expected);
}
