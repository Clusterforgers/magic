//! Goal: "double the power of each creature you control" is one pass per creature, each reading its own power as the pass begins: a 2-power and a 3-power creature become 4 and 6, an opponent's creature is untouched, and the effect ends with the turn.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::characteristics::characteristics;
use mtg_engine::turn::Step;
use mtg_testkit::cards::forest;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const DOUBLE: &str = r#"{"Spell":{"effects":[{"RepeatEach":{"over":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"as":"d","effects":[{"Continuous":{"affected":{"Bound":"d"},"mods":[{"ModifyPT":{"power":{"Stat":{"of":{"Bound":"d"},"stat":"Power"}},"toughness":0}}],"duration":"UntilEndOfTurn"}}]}}]}}"#;

fn creature(name: &str, power: i32) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_power_toughness(power, 4),
    )
}

fn power(driver: &Driver, name: &str) -> i32 {
    characteristics(driver.state(), driver.id_of(name).unwrap())
        .unwrap()
        .power
        .unwrap()
}

#[rstest]
#[case::two_of_mine(&[("Small", 2), ("Large", 3)], 4, 6)]
#[case::zero_power(&[("Small", 0), ("Large", 5)], 0, 10)]
fn each_creature_doubles_its_own_power(
    #[case] mine: &[(&str, i32)],
    #[case] small: i32,
    #[case] large: i32,
) -> Result<(), ScriptError> {
    let rule: Ability = serde_json::from_str(DOUBLE).unwrap();
    let double = Arc::new(
        CardDefinition::new("Double")
            .with_type(CardType::Instant)
            .with_mana_cost(ManaCost::parse("{G}").unwrap())
            .with_rules(rule),
    );
    let mut scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .hand(0, [double])
        .library(0, vec![forest(); 30])
        .library(1, vec![forest(); 30]);
    let mut mine: Vec<_> = mine.iter().map(|(n, p)| ready(creature(n, *p))).collect();
    mine.push(ready(forest()));
    scenario = scenario
        .battlefield(0, mine)
        .battlefield(1, [ready(creature("Theirs", 5))]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast_tapping("Double", &["Forest"])?;
    driver.pass()?;
    driver.pass()?;
    assert_eq!(power(&driver, "Small"), small);
    assert_eq!(power(&driver, "Large"), large);
    assert_eq!(power(&driver, "Theirs"), 5);
    Ok(())
}
