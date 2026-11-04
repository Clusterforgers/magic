//! Goal: a static line of the mode `CantAttack,CantBlock,CantBeActivated` ("Arrest") translates to the restriction and
//! the activation rule of its parts, and the enchanted creature can neither attack nor block when it is played.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::turn::Step;
use mtg_testkit::cards::free_creature;
use mtg_testkit::scenario::attached_to;
use mtg_testkit::{Driver, Scenario, ready};
use serde_json::json;

use super::{abilities, gaps};

const ARREST: &str = "Name:Arrest\nTypes:Enchantment Aura\nK:Enchant:Creature\nS:Mode$ CantAttack,CantBlock,CantBeActivated | ValidCard$ Creature.EnchantedBy | Description$ x\n";

#[test]
fn the_line_is_a_restriction_and_a_rule() {
    let found = abilities(ARREST);
    assert!(gaps(ARREST).is_empty(), "{:?}", gaps(ARREST));
    assert_eq!(
        found[1],
        json!({"Static": {"affected": "Attached", "mods": ["CantAttack", "CantBlock"]}})
    );
    assert!(found[2]["Rule"]["rule"]["Cant"]["action"]["Activate"].is_array());
}

#[test]
fn the_enchanted_creature_cannot_attack() {
    let mut card = CardDefinition::new("Arrest")
        .with_type(CardType::Enchantment)
        .with_subtype("Aura");
    for value in abilities(ARREST) {
        card = card.with_rules(serde_json::from_value::<Ability>(value).unwrap());
    }
    let aura = attached_to(ready(Arc::new(card)), 0, 0);
    let scenario = Scenario::new(2).at(1, 0, Step::Main1).battlefield(
        0,
        [
            ready(free_creature("Host", 2, 2)),
            aura,
            ready(free_creature("Free", 2, 2)),
        ],
    );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.pass_until(Step::DeclareAttackers).unwrap();
    assert!(driver.attack(&[("Host", 1)]).is_err());
    driver.attack(&[("Free", 1)]).unwrap();
}
