//! Goal: "loses all abilities and has X" keeps X whatever the order of the modifications in the list (CR 613.1f), for an effect of a spell and for a static ability, while a keyword given by another effect before the loss is lost.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_engine::keywords::has_keyword;
use mtg_testkit::cards::{free_creature, plains};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const ORDERS: [&str; 2] = [
    r#"["LoseAbilities",{"AddKeyword":"Hexproof"}]"#,
    r#"[{"AddKeyword":"Hexproof"},"LoseAbilities"]"#,
];

fn hexproof(driver: &Driver, name: &str) -> bool {
    has_keyword(
        driver.state(),
        driver.id_of(name).unwrap(),
        &Keyword::Hexproof,
    )
}

#[rstest]
fn a_spell_that_takes_every_ability_and_gives_a_keyword_gives_it(
    #[values(0, 1)] order: usize,
) -> Result<(), ScriptError> {
    let spell = format!(
        r#"{{"Spell":{{"targets":[{{"what":{{"Object":[{{"Type":"Creature"}}]}}}}],"effects":[{{"Continuous":{{"affected":{{"Target":0}},"mods":{},"duration":"UntilEndOfTurn"}}}}]}}}}"#,
        ORDERS[order]
    );
    let card = Arc::new(
        CardDefinition::new("Shift")
            .with_type(CardType::Instant)
            .with_mana_cost(mtg_core::mana::ManaCost::parse("{0}").unwrap())
            .with_rules(serde_json::from_str(&spell).unwrap()),
    );
    let scenario = Scenario::new(2)
        .battlefield(0, [ready(free_creature("Pawn", 1, 1)), ready(plains())])
        .hand(0, [card]);
    let mut driver = Driver::from_scenario(scenario)?;
    let target = driver.target_named("Pawn")?;
    driver.cast_at("Shift", &[target])?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    assert!(hexproof(&driver, "Pawn"));
    Ok(())
}

#[rstest]
fn a_static_ability_that_takes_every_ability_and_gives_a_keyword_gives_it(
    #[values(0, 1)] order: usize,
) -> Result<(), ScriptError> {
    let rules = format!(
        r#"{{"Static":{{"affected":"This","mods":{}}}}}"#,
        ORDERS[order]
    );
    let card = Arc::new(
        CardDefinition::new("Shell")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(serde_json::from_str(&rules).unwrap()),
    );
    let driver = Driver::from_scenario(Scenario::new(2).battlefield(0, [ready(card)]))?;
    assert!(hexproof(&driver, "Shell"));
    Ok(())
}
