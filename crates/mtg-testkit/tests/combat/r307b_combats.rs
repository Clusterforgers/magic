//! Goal: corpus cards of pack r30-7b that act in combat play as their rules say at two, three and four seats: Raphael's
//! trigger on combat damage to a player works the first time each turn only, so it adds one combat phase and no more;
//! Typhoid Mary's modes are picked at random, and its controller chooses them when they discarded a card this turn.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::refs::Ref;
use mtg_cards::value::Amount;
use mtg_core::card::CardType;
use mtg_engine::Answer;
use mtg_engine::choose::ModeChoice;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, free_creature};
use mtg_testkit::corpus::corpus;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Plays the turn out: `attacker` attacks seat 1 in every combat it can, each mode question takes `mode`, anything else
/// is answered passively; stops at the end step. Whether a mode question was asked.
fn play_combats(driver: &mut Driver, attacker: &str, mode: usize) -> Result<bool, ScriptError> {
    let mut asked = false;
    for _ in 0..400 {
        if driver.state().turn().step() == Step::End {
            return Ok(asked);
        }
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::DeclareAttackers(_) if decision.player == pid(0) => {
                if driver.attack(&[(attacker, 1)]).is_err() {
                    driver.answer_passively()?;
                }
            }
            DecisionKind::ChooseMode(_) => {
                asked = true;
                driver.answer(Answer::ChooseMode(ModeChoice::Modes(vec![mode])))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the end step".to_owned()))
}

#[rstest]
fn raphael_adds_one_combat_phase_a_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let raphael = corpus().unwrap().card("Raphael, Tag Team Tough").unwrap();
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 0, Step::Main1)
            .battlefield(0, [ready(raphael)]),
    )?;
    play_combats(&mut driver, "Raphael, Tag Team Tough", 0)?;
    // Two combats of 5 damage: the first one's trigger adds the second, whose damage does not trigger again.
    assert_eq!(driver.state().player(pid(1)).unwrap().life(), 10);
    Ok(())
}

/// A free sorcery: "Discard a card."
fn rummage() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Rummage")
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[]))
            .with_rules(Ability::Spell {
                targets: Vec::new(),
                effects: vec![Effect::Discard {
                    who: Ref::You,
                    count: Amount::Fixed(1),
                    random: false,
                    filter: None,
                    by: None,
                    optional: false,
                    bind: None,
                }],
            }),
    )
}

#[rstest]
fn typhoid_mary_lets_you_choose_after_a_discard(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] discarded: bool,
) -> Result<(), ScriptError> {
    let mary = corpus().unwrap().card("Typhoid Mary, Fractured").unwrap();
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 0, Step::Main1)
            .battlefield(0, [ready(mary)])
            .hand(0, [rummage(), free_creature("Spare", 1, 1)]),
    )?;
    if discarded {
        driver.cast("Rummage")?;
        while !driver.state().zone(Zone::Hand(pid(0))).is_empty() {
            driver.answer_passively()?;
        }
    }
    let asked = play_combats(&mut driver, "Typhoid Mary, Fractured", 2)?;
    assert_eq!(asked, discarded);
    if discarded {
        // The controller chose Bloody Mary: each opponent loses 2 life and they gain 2; seat 1 also took 3 in combat.
        assert_eq!(driver.state().player(pid(0)).unwrap().life(), 22);
        for seat in 1..seats {
            let combat = if seat == 1 { 3 } else { 0 };
            assert_eq!(
                driver.state().player(pid(seat)).unwrap().life(),
                18 - combat
            );
        }
    }
    Ok(())
}
