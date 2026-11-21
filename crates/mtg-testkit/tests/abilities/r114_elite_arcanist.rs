//! Goal: Elite Arcanist at 2 to 4 seats: as it enters its controller may exile an instant card from their hand; its
//! activation costs X equal to that card's mana value (CR 107.3), copies the exiled card (CR 707.12) and casts the copy
//! without paying its mana cost, so the copy's effect happens and the exiled card stays in exile.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::{CardsChoice, NumberChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, forest, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// An instant for {1}{U}: "Each opponent loses 2 life."
fn drain() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Drain")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(1, &[Color::Blue]))
            .with_rules(
                serde_json::from_str(
                    r#"{"Spell":{"effects":[{"LoseLife":{"who":"Opponents","amount":2}}]}}"#,
                )
                .unwrap(),
            ),
    )
}

/// Answers the decisions until priority comes back with an empty stack: yes to every question, the Drain for a pick,
/// `x` for X and an automatic payment.
fn settle(driver: &mut Driver, x: u32) -> Result<(), ScriptError> {
    for _ in 0..60 {
        let kind = driver.decision()?.kind.clone();
        match kind {
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(true)))?;
            }
            DecisionKind::ChooseCards(options) => {
                driver.answer(Answer::ChooseCards(CardsChoice(options.from[..1].to_vec())))?;
            }
            DecisionKind::ChooseNumber(_) => {
                driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ if driver.state().stack().is_empty()
                && driver.state().pending_triggers().is_empty() =>
            {
                return Ok(());
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack does not settle".into()))
}

#[rstest]
fn r114_elite_arcanist_casts_a_copy_of_the_exiled_instant(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, vec![card("Elite Arcanist").unwrap(), drain()])
        .battlefield(0, (0..6).map(|_| ready(island())));
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 6]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast_tapping("Elite Arcanist", &["Island"; 4])?;
    settle(&mut driver, 0)?;
    let exiled = driver.state().zone(Zone::Exile).to_vec();
    assert_eq!(exiled.len(), 1, "the Drain is exiled");
    // Seat 0's next turn: the Arcanist can tap.
    driver.idle_until(u32::try_from(seats).unwrap() + 1, Step::Main1)?;
    driver.begin_ability("Elite Arcanist", 1)?;
    settle(&mut driver, 2)?;
    for seat in 1..seats {
        let life = driver
            .state()
            .player(PlayerId::from_index(seat))
            .unwrap()
            .life();
        assert_eq!(life, 18, "the copy made each opponent lose 2 life");
    }
    assert_eq!(driver.state().zone(Zone::Exile), exiled.as_slice());
    Ok(())
}
