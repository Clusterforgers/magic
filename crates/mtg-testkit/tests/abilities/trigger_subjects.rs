//! Goal: what `Triggering` and `Dealer` name for a trigger depends on its pattern (a block holds the attacker and the blocker is the dealer, damage holds its recipient and the source is the dealer), so a translator that picks the reference by pattern lands each effect on the right object.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::on_battlefield;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::turn::Step;
use mtg_testkit::cards::free_creature;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A creature with a toughness of 9 with one triggered ability written in the card DSL.
fn watcher(name: &str, power: i32, when: &str, to: &str) -> Arc<CardDefinition> {
    let rules: Ability = serde_json::from_str(&format!(
        r#"{{"Triggered":{{"when":{when},"effects":[{{"DealDamage":{{"to":{to},"amount":2}}}}]}}}}"#
    ))
    .expect("the trigger is written in the card DSL");
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_power_toughness(power, 9)
            .with_rules(rules),
    )
}

fn marked(driver: &Driver, name: &str) -> u32 {
    let id = on_battlefield(driver.state(), name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .damage()
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(common::pid(seat)).unwrap().life()
}

/// Passes until the stack is empty again after the declaration just made.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..6 {
        if driver.state().stack().is_empty()
            && driver.state().turn().step() != Step::DeclareAttackers
        {
            break;
        }
        driver.pass()?;
    }
    Ok(())
}

/// "Whenever this becomes blocked by a creature": `Triggering` is the attacker, `Dealer` the blocker.
#[rstest]
#[case::the_blocker_is_the_dealer(r#""Dealer""#, 0, 2)]
#[case::the_attacker_is_triggering(r#""Triggering""#, 2, 0)]
#[case::the_card_itself(r#""This""#, 2, 0)]
fn a_block_names_the_attacker_and_the_blocker_apart(
    #[case] to: &str,
    #[case] attacker_damage: u32,
    #[case] blocker_damage: u32,
) -> Result<(), ScriptError> {
    let attacker = watcher(
        "Acolyte",
        0,
        r#"{"BecomesBlocked":{"what":["This"],"by":[{"Type":"Creature"}]}}"#,
        to,
    );
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, [ready(attacker)])
            .battlefield(1, [ready(free_creature("Blocker", 0, 9))]),
    )?;
    driver.attack(&[("Acolyte", 1)])?;
    common::combat::to_blockers(&mut driver, 1)?;
    driver.block(&[("Blocker", "Acolyte")])?;
    settle(&mut driver)?;
    assert_eq!(marked(&driver, "Acolyte"), attacker_damage);
    assert_eq!(marked(&driver, "Blocker"), blocker_damage);
    Ok(())
}

/// "Whenever this deals combat damage": `Dealer` is the source, `Triggering` the player it hit. The creature deals 1 combat damage itself.
#[rstest]
#[case::the_source_is_the_dealer(r#""Dealer""#, 2, 19)]
#[case::the_recipient_is_triggering(r#""Triggering""#, 0, 17)]
fn damage_names_its_source_and_its_recipient_apart(
    #[case] to: &str,
    #[case] source_damage: u32,
    #[case] defender_life: i32,
) -> Result<(), ScriptError> {
    let source = watcher(
        "Striker",
        1,
        r#"{"DealsDamage":{"source":["This"],"combat":true}}"#,
        to,
    );
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, [ready(source)]),
    )?;
    driver.attack(&[("Striker", 1)])?;
    for _ in 0..8 {
        driver.pass()?;
    }
    assert_eq!(marked(&driver, "Striker"), source_damage);
    assert_eq!(life(&driver, 1), defender_life);
    Ok(())
}
