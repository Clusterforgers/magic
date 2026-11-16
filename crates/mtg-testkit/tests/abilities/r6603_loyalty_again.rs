//! Goal: "you may activate ~'s loyalty ability two more times this turn" (`GameRule::R6603LoyaltyAgain`) lets the loyalty
//! ability that made the rule be activated that many more times this turn, adds up when made again, leaves the
//! planeswalker's other loyalty abilities at the usual one activation (Comet, Stellar Pup rulings) and ends with the
//! turn; at two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A planeswalker whose first ability "0: You may activate this loyalty ability two more times this turn." and whose
/// second is "0: You gain 1 life."
fn pup() -> Arc<CardDefinition> {
    let again: Ability = serde_json::from_str(
        r#"{"Loyalty":{"cost":"Zero","effects":[{"Rule":{"rule":{"R6603LoyaltyAgain":2},"duration":"UntilEndOfTurn"}}]}}"#,
    )
    .unwrap();
    let gain: Ability = serde_json::from_str(
        r#"{"Loyalty":{"cost":"Zero","effects":[{"GainLife":{"amount":1}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Pup")
            .with_type(CardType::Planeswalker)
            .with_loyalty(3)
            .with_rules(again)
            .with_rules(gain),
    )
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

/// Activates the ability at `index` while it is offered, at most `most` times; how many times that was.
fn activate(driver: &mut Driver, index: usize, most: u32) -> Result<u32, ScriptError> {
    let mut times = 0;
    while times < most && driver.offered_abilities("Pup")?.contains(&index) {
        driver.begin_ability("Pup", index)?;
        settle(driver)?;
        times += 1;
    }
    Ok(times)
}

#[rstest]
fn r6603_the_ability_that_made_the_rule_goes_again(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(0, [ready(pup())]),
    )?;
    let offered = driver.offered_abilities("Pup")?;
    assert_eq!(offered.len(), 2, "both loyalty abilities at first");
    let (again, gain) = (offered[0], offered[1]);
    // Once, then twice more by the first rule, then twice more by each rule each of those made: it adds up.
    assert_eq!(activate(&mut driver, again, 1)?, 1);
    assert!(
        !driver.offered_abilities("Pup")?.contains(&gain),
        "the other ability keeps the one activation"
    );
    assert_eq!(activate(&mut driver, again, 2)?, 2);
    assert!(driver.offered_abilities("Pup")?.contains(&again));
    // The next turn of seat 0 gives the usual one activation.
    let next = u32::try_from(seats).unwrap() + 1;
    driver.idle_until(next, Step::Main1)?;
    settle(&mut driver)?;
    assert_eq!(activate(&mut driver, gain, 5)?, 1);
    assert!(!driver.offered_abilities("Pup")?.contains(&again));
    Ok(())
}

#[rstest]
fn r6603_another_ability_first_spends_the_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(0, [ready(pup())]),
    )?;
    let offered = driver.offered_abilities("Pup")?;
    let (again, gain) = (offered[0], offered[1]);
    assert_eq!(activate(&mut driver, gain, 5)?, 1);
    assert!(!driver.offered_abilities("Pup")?.contains(&again));
    Ok(())
}
