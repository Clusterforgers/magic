//! Goal: the mana a cast spends is counted by the kind of its source ("mana from a Treasure was spent", snow mana): the sources that paid are what counts, a spell that a Treasure did not pay for counts none, both the spell and the permanent it becomes read it, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{cost, island};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn rules(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

/// An artifact Treasure: "{T}, Sacrifice this artifact: Add one mana of any color."
fn treasure() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Treasure")
            .with_type(CardType::Artifact)
            .with_subtype("Treasure")
            .with_rules(rules(
                r#"{"Activated":{"cost":["Tap","SacrificeSelf"],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#,
            )),
    )
}

/// A creature for `{1}{U}` that gains its controller as much life as mana from a Treasure was spent to cast it.
fn hexer() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Hexer")
            .with_type(CardType::Creature)
            .with_mana_cost(cost(1, &[Color::Blue]))
            .with_rules(rules(
                r#"{"Triggered":{"when":{"ZoneChange":{"to":"Battlefield","what":["This"]}},"effects":[{"GainLife":{"who":"You","amount":{"ManaSpent":{"of":null,"from":{"subtypes":["Treasure"]}}}}}]}}"#,
            )),
    )
}

fn life(driver: &Driver) -> i32 {
    driver
        .state()
        .player(mtg_core::ids::PlayerId(0))
        .unwrap()
        .life()
}

#[rstest]
#[case::no_treasure(&["Island", "Island"], 0)]
#[case::one_treasure(&["Treasure", "Island"], 1)]
fn the_mana_of_a_treasure_is_counted_for_the_spell_and_for_its_permanent(
    #[case] paying: &[&str],
    #[case] gained: i32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .hand(0, vec![hexer()])
            .battlefield(0, [ready(treasure()), ready(island()), ready(island())]),
    )?;
    let before = life(&driver);
    driver.cast_tapping("Hexer", paying)?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    assert_eq!(life(&driver) - before, gained);
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    Ok(())
}
