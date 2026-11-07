//! Goal: the rules of round 11 pack g11c play whole at tables of two, three and four seats: a taxed blocker is declared
//! only with the payment of its tax (mana or life, once for each taxed blocker, for the attackers the tax names), a block
//! its player cannot pay for is refused, backing out of the payment returns to the declaration with nothing paid, a
//! creature that must block is not bound when blocking costs, an attack tax counted by an amount repeats its mana, and a
//! draw trigger tells the first card of its player's own draw step from every other draw.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::Event;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_testkit::cards::{free_creature, plains};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::lands;
use crate::common::pid;

/// A creature with one rule, from its JSON.
fn with_rule(name: &str, power: i32, rule: &str) -> Arc<CardDefinition> {
    let rules: Ability = serde_json::from_str(rule).unwrap();
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_power_toughness(power, power)
            .with_rules(rules),
    )
}

fn block_tax(fields: &str) -> String {
    format!(r#"{{"Rule":{{"rule":{{"BlockTax":{{{fields}}}}}}}}}"#)
}

/// Seat 0 attacks seat 1 with a 1/1 "Small" and a 3/3 "Big"; seat 1 holds the blockers and `count` Plains.
fn blocking(
    seats: usize,
    blockers: Vec<Arc<CardDefinition>>,
    count: usize,
) -> Result<Driver, ScriptError> {
    let mut held: Vec<_> = blockers.into_iter().map(ready).collect();
    held.extend(lands(count, plains));
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(
                0,
                [
                    ready(free_creature("Small", 1, 1)),
                    ready(free_creature("Big", 3, 3)),
                ],
            )
            .battlefield(1, held),
    )?;
    driver.attack(&[("Small", 1), ("Big", 1)])?;
    driver.pass_until(Step::DeclareBlockers)?;
    driver.expect_decision(1, "declare-blockers")?;
    Ok(driver)
}

fn declared(driver: &Driver) -> bool {
    driver
        .engine()
        .events()
        .iter()
        .any(|event| matches!(event, Event::BlockersDeclared { blocks, .. } if !blocks.is_empty()))
}

fn untapped_plains(driver: &Driver) -> usize {
    driver
        .state()
        .zone(mtg_engine::zones::Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter(|object| object.card().name == "Plains")
        .filter(|object| object.permanent().is_some_and(|p| !p.tapped()))
        .count()
}

const SELF_TWO: &str = r#""blockers":["This"],"mana":"{2}""#;
const BIG_ONLY: &str = r#""blockers":["This"],"attackers":[{"Compare":{"op":"Ge","stat":"Power","to":3}}],"mana":"{1}""#;
const EVERY_ONE: &str = r#""mana":"{1}""#;

#[rstest]
#[case::its_own_tax(SELF_TWO, &[("Taxed", "Small")], 2, Some(2))]
#[case::one_mana_short(SELF_TWO, &[("Taxed", "Small")], 1, None)]
#[case::an_attacker_it_names(BIG_ONLY, &[("Taxed", "Big")], 1, Some(1))]
#[case::an_attacker_it_does_not_name(BIG_ONLY, &[("Taxed", "Small")], 0, Some(0))]
#[case::an_untaxed_blocker(SELF_TWO, &[("Free", "Small")], 0, Some(0))]
#[case::every_blocker_once_each(EVERY_ONE, &[("Taxed", "Small"), ("Free", "Big")], 2, Some(2))]
#[case::every_blocker_short(EVERY_ONE, &[("Taxed", "Small"), ("Free", "Big")], 1, None)]
fn a_taxed_block_is_declared_with_its_payment(
    #[values(2, 3, 4)] seats: usize,
    #[case] tax: &str,
    #[case] blocks: &[(&str, &str)],
    #[case] mana: usize,
    #[case] paid: Option<usize>,
) -> Result<(), ScriptError> {
    let taxed = with_rule("Taxed", 2, &block_tax(tax));
    let mut driver = blocking(seats, vec![taxed, free_creature("Free", 2, 2)], mana)?;
    let attempt = driver.block(blocks);
    let Some(paid) = paid else {
        assert!(attempt.is_err(), "a block that cannot be paid is refused");
        driver.expect_decision(1, "declare-blockers")?;
        assert!(!declared(&driver));
        assert_eq!(untapped_plains(&driver), mana, "nothing was paid");
        return Ok(());
    };
    attempt?;
    if paid > 0 {
        driver.expect_decision(1, "pay-mana")?;
        let options = driver.pay_options()?;
        assert!(options.block && !options.attack && options.can_cancel);
        assert_eq!(options.cost.mana_value() as usize, paid);
        assert!(!declared(&driver), "the declaration waits for the payment");
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
    }
    assert!(declared(&driver));
    assert_eq!(untapped_plains(&driver), mana - paid);
    Ok(())
}

#[rstest]
fn backing_out_of_the_payment_returns_to_the_declaration(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let taxed = with_rule("Taxed", 2, &block_tax(SELF_TWO));
    let mut driver = blocking(seats, vec![taxed], 2)?;
    driver.block(&[("Taxed", "Small")])?;
    driver.expect_decision(1, "pay-mana")?;
    driver.answer(Answer::PayMana(PayChoice::Cancel))?;
    driver.expect_decision(1, "declare-blockers")?;
    assert_eq!(untapped_plains(&driver), 2);
    driver.block(&[])?;
    assert!(!declared(&driver));
    Ok(())
}

#[rstest]
fn a_block_tax_in_life_is_paid_in_life(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let taxed = with_rule("Taxed", 2, &block_tax(r#""life":1"#));
    let mut driver = blocking(seats, vec![taxed, free_creature("Free", 2, 2)], 0)?;
    let before = driver.state().player(pid(1)).unwrap().life();
    driver.block(&[("Taxed", "Small"), ("Free", "Big")])?;
    assert!(declared(&driver));
    assert_eq!(driver.state().player(pid(1)).unwrap().life(), before - 2);
    Ok(())
}

#[rstest]
fn a_creature_that_must_block_is_not_bound_when_blocking_costs(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let rules: Ability = serde_json::from_str(r#"{"Rule":{"rule":{"MustBlock":"This"}}}"#).unwrap();
    let taxed = Arc::new(
        CardDefinition::new("Taxed")
            .with_type(CardType::Creature)
            .with_power_toughness(2, 2)
            .with_rules(serde_json::from_str::<Ability>(&block_tax(SELF_TWO)).unwrap())
            .with_rules(rules),
    );
    let mut driver = blocking(seats, vec![taxed], 2)?;
    let DecisionKind::DeclareBlockers(options) = &driver.decision()?.kind else {
        unreachable!();
    };
    assert!(options.required.is_empty());
    driver.block(&[])?;
    assert!(!declared(&driver));
    Ok(())
}

/// A permanent that gains 1 life for each card its trigger sees drawn.
fn draw_watcher(when: &str) -> Arc<CardDefinition> {
    let rules: Ability = serde_json::from_str(&format!(
        r#"{{"Triggered":{{"effects":[{{"GainLife":{{"amount":1}}}}],"when":{{"Drew":{when}}}}}}}"#
    ))
    .unwrap();
    Arc::new(
        CardDefinition::new("Watcher")
            .with_type(CardType::Enchantment)
            .with_rules(rules),
    )
}

/// Seat 0 holds the watcher; seat 1's turn goes from upkeep through its draw step into its main phase, where seat 1
/// draws one more card by hand. Returns seat 0's life gained.
fn draws(seats: usize, when: &str, active: usize) -> Result<i32, ScriptError> {
    let library: Vec<_> = (0..4)
        .map(|n| free_creature(&format!("Card {n}"), 1, 1))
        .collect();
    let mut scenario = Scenario::new(seats)
        .at(2, active, Step::Upkeep)
        .battlefield(0, [ready(draw_watcher(when))]);
    for seat in 0..seats {
        scenario = scenario.library(seat, library.clone());
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let before = driver.state().player(pid(0)).unwrap().life();
    driver.pass_until(Step::Main1)?;
    for _ in 0..12 {
        if driver.state().stack().is_empty() && driver.state().pending_triggers().is_empty() {
            break;
        }
        driver.answer_passively()?;
    }
    driver.manual_by(
        pid(active),
        &mtg_engine::manual::ManualAction::Draw { count: 1 },
    )?;
    for _ in 0..12 {
        if driver.state().stack().is_empty() && driver.state().pending_triggers().is_empty() {
            break;
        }
        driver.answer_passively()?;
    }
    Ok(driver.state().player(pid(0)).unwrap().life() - before)
}

#[rstest]
#[case::except_the_first(r#"{"first_in_draw_step":false,"who":"Opponents"}"#, 1, 1)]
#[case::the_first_of_an_opponent(r#"{"first_in_draw_step":true,"who":"Opponents"}"#, 1, 1)]
#[case::every_draw_of_an_opponent(r#"{"who":"Opponents"}"#, 1, 2)]
#[case::the_first_of_your_own(r#"{"first_in_draw_step":true,"who":"You"}"#, 0, 1)]
#[case::except_your_first(r#"{"first_in_draw_step":false,"who":"You"}"#, 0, 1)]
fn a_draw_trigger_tells_the_first_card_of_a_draw_step(
    #[values(2, 3, 4)] seats: usize,
    #[case] when: &str,
    #[case] active: usize,
    #[case] gained: i32,
) -> Result<(), ScriptError> {
    assert_eq!(draws(seats, when, active)?, gained);
    Ok(())
}
