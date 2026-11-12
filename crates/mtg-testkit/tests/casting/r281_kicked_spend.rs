//! Goal: mana an Elfhame Druid adds for kicked spells pays for a spell once its kicker is announced and for no unkicked spell, at two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::choose::CostsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::zones::Zone;
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

/// Taps Elfhame Druid for its {G}{G} for kicked spells.
fn add_kicked_mana(driver: &mut Driver) -> Result<(), ScriptError> {
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        panic!("priority was expected");
    };
    let source = options
        .mana_abilities
        .iter()
        .find(|m| m.count == 2)
        .map(|m| m.source)
        .unwrap();
    driver.answer(Answer::Priority(PriorityAction::ActivateMana(source)))?;
    Ok(())
}

/// Casts Kavu Titan, kicked, and plays until the stack is empty; whether the payment could be made.
fn cast_kicked_titan(driver: &mut Driver) -> Result<bool, ScriptError> {
    driver.begin_cast("Kavu Titan")?;
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::Priority(_)) && driver.state().stack().is_empty() {
            break;
        }
        let answer = match &decision.kind {
            DecisionKind::ChooseCosts(_) => Answer::ChooseCosts(CostsChoice::Costs(vec![0])),
            DecisionKind::PayMana(options) if !options.auto_available => {
                return Ok(false);
            }
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Ok(true)
}

#[rstest]
fn the_druids_mana_pays_for_a_kicked_spell(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand_named(0, ["Kavu Titan"])
            .battlefield_named(0, ["Elfhame Druid", "Forest", "Forest", "Forest"]),
    )?;
    // {G}{G} for kicked spells and three Forests pay {1}{G} and the kicker {2}{G}.
    add_kicked_mana(&mut driver)?;
    assert!(cast_kicked_titan(&mut driver)?);
    let titan = driver.id_of("Kavu Titan")?;
    let state = driver.state();
    assert_eq!(state.zone(Zone::Battlefield).len(), 5);
    let counters = state
        .object(titan)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&mtg_cards::value::CounterKind::PlusOnePlusOne);
    assert_eq!(counters, 3, "it was kicked");
    Ok(())
}

#[rstest]
fn the_druids_mana_does_not_pay_for_an_unkicked_spell(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand_named(0, ["Kavu Titan"])
            .battlefield_named(0, ["Elfhame Druid", "Forest"]),
    )?;
    add_kicked_mana(&mut driver)?;
    // {G}{G} for kicked spells and one Forest: the kicker is too dear, and unkicked the druid's mana cannot help.
    let titan = driver.state().zone(Zone::Hand(mtg_core::ids::PlayerId(0)))[0];
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        panic!("priority was expected");
    };
    assert!(!options.castable.contains(&titan));
    Ok(())
}
