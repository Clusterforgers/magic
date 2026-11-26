//! Goal: "You may activate the loyalty abilities of ~ twice each turn rather than only once."
//! (`GameRule::P158LoyaltyTwiceThis`) lets that planeswalker's loyalty abilities go twice each turn, the same or two
//! different ones, never a third time, leaves another planeswalker of the same player at one activation and comes back
//! with the next turn; at two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A planeswalker with two "0: You gain 1 life." abilities, with the rule when `twice`.
fn walker(name: &str, twice: bool) -> Arc<CardDefinition> {
    let gain: Ability = serde_json::from_str(
        r#"{"Loyalty":{"cost":"Zero","effects":[{"GainLife":{"amount":1}}]}}"#,
    )
    .unwrap();
    let mut card = CardDefinition::new(name)
        .with_type(CardType::Planeswalker)
        .with_loyalty(3);
    if twice {
        let rule: Ability =
            serde_json::from_str(r#"{"Rule":{"rule":"P158LoyaltyTwiceThis"}}"#).unwrap();
        card = card.with_rules(rule);
    }
    Arc::new(card.with_rules(gain.clone()).with_rules(gain))
}

fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..20 {
        if matches!(driver.decision()?.kind, DecisionKind::Priority(_))
            && driver.state().stack().is_empty()
        {
            return Ok(());
        }
        driver.pass()?;
    }
    Ok(())
}

/// Activates the loyalty abilities of `name` while one is offered, the one at `pick` of those offered (or the last),
/// at most `most` times; how many times that was.
fn activate(driver: &mut Driver, name: &str, pick: usize, most: u32) -> Result<u32, ScriptError> {
    let mut times = 0;
    while times < most {
        let offered = driver.offered_abilities(name)?;
        let Some(index) = offered.get(pick).or(offered.last()).copied() else {
            break;
        };
        driver.begin_ability(name, index)?;
        settle(driver)?;
        times += 1;
    }
    Ok(times)
}

#[rstest]
fn p158_the_loyalty_abilities_go_twice_each_turn(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] second: usize,
) -> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(Scenario::new(seats).at(1, 0, Step::Main1).battlefield(
            0,
            [ready(walker("Urza", true)), ready(walker("Other", false))],
        ))?;
    assert_eq!(activate(&mut driver, "Urza", 0, 1)?, 1);
    // The second activation may be the same ability or the other one; no third.
    assert_eq!(activate(&mut driver, "Urza", second, 1)?, 1);
    assert!(
        driver.offered_abilities("Urza")?.is_empty(),
        "never a third time"
    );
    assert_eq!(
        activate(&mut driver, "Other", 0, 5)?,
        1,
        "another planeswalker goes once"
    );
    let next = u32::try_from(seats).unwrap() + 1;
    driver.idle_until(next, Step::Main1)?;
    settle(&mut driver)?;
    assert_eq!(
        activate(&mut driver, "Urza", 0, 5)?,
        2,
        "twice again on the next turn"
    );
    Ok(())
}
