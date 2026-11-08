//! Goal: "a creature spell that has an Adventure" matches a card of the Adventure layout and no other card, so the trigger over casting such a spell gains life for the Adventure creature and not for a plain creature.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{forest, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::{choose_way, pay};
use crate::common::pid;

/// A 1/1 that gains 3 life whenever its controller casts a creature spell that has an Adventure.
fn watcher() -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_str(
        r#"{"Triggered":{"when":{"SpellCast":{"what":[{"Type":"Creature"},"HasAdventure"],"by":"You"}},"effects":[{"GainLife":{"who":"You","amount":3}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Watcher")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(ability),
    )
}

#[rstest]
#[case::adventure_creature("Bonecrusher Giant", true, 3)]
#[case::plain_creature("Grizzly Bears", false, 0)]
fn only_a_card_with_an_adventure_triggers_the_watcher(
    #[case] spell: &str,
    #[case] asks_the_way: bool,
    #[case] gained: i32,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .hand_named(0, [spell])
        .library(0, vec![forest(); 30])
        .library(1, vec![forest(); 30])
        .battlefield(
            0,
            [
                watcher(),
                forest(),
                forest(),
                mountain(),
                mountain(),
                mountain(),
            ]
            .map(ready),
        );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast(spell)?;
    if asks_the_way {
        choose_way(&mut driver, spell)?;
    }
    pay(&mut driver)?;
    driver.pass()?;
    driver.pass()?;
    let life = driver.state().player(pid(0)).unwrap().life();
    assert_eq!(life, 20 + gained, "{spell}");
    Ok(())
}
