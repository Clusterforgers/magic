//! Goal: Adeline, Resplendent Cathar makes one tapped Human for each opponent, attacking that opponent or, at its
//! controller's choice, a planeswalker that opponent controls, never another player's, at tables of two, three and four
//! seats; Protean Hydra puts two +1/+1 counters back at the next end step for each counter damage removed; and every
//! game replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use mtg_cards::value::CounterKind;
use mtg_engine::choose::OptionChoice;
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_testkit::cards::{free_creature, mountain, walker};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

use crate::c4_control::{replays, settle};
use crate::common::name_of;

/// Answers until the stack is empty: a choice of what the tokens attack takes the last option offered (a planeswalker
/// when one is offered) when `walkers`, else the first (the player); everything else passively.
fn r407_settle(driver: &mut Driver, walkers: bool) -> Result<usize, ScriptError> {
    let mut asked = 0;
    for _ in 0..80 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().pending_triggers().is_empty() =>
            {
                return Ok(asked);
            }
            DecisionKind::ChooseOption(options) => {
                asked += 1;
                let pick = if walkers {
                    options.options.len() - 1
                } else {
                    0
                };
                driver.answer(Answer::ChooseOption(OptionChoice(pick)))?;
            }
            _ => {
                driver.answer(Driver::passive_answer(&decision))?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack did not empty".to_owned()))
}

#[rstest]
fn r407_adeline_makes_a_human_attacking_each_opponent_or_their_planeswalker(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] walkers: bool,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card("Adeline, Resplendent Cathar").unwrap()),
                ready(free_creature("Bear", 2, 2)),
            ],
        )
        .battlefield(1, [ready(walker("Lone Walker", 3))]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Bear", 1)])?;
    let asked = r407_settle(&mut driver, walkers)?;
    // Only the opponent with a planeswalker is asked about.
    assert_eq!(asked, 1);
    let state = driver.state();
    let humans: Vec<_> = state
        .combat()
        .attackers()
        .iter()
        .filter(|a| name_of(state, a.object()) == "Human")
        .collect();
    assert_eq!(humans.len(), seats - 1, "one Human for each opponent");
    let defenders: BTreeSet<usize> = humans
        .iter()
        .map(|a| a.defender(state).unwrap().index())
        .collect();
    assert_eq!(defenders, (1..seats).collect::<BTreeSet<_>>());
    let at_walker = humans
        .iter()
        .filter(|a| matches!(a.target(), AttackTarget::Planeswalker(_)))
        .count();
    assert_eq!(at_walker, usize::from(walkers));
    replays(&driver);
    Ok(())
}

#[rstest]
fn r407_protean_hydra_regrows_two_for_each_counter_removed(
    #[values(2, 3, 4)] seats: usize,
    #[values(("Burst Lightning", 2), ("Lightning Bolt", 3))] burn: (&str, u32),
) -> Result<(), ScriptError> {
    let (spell, damage) = burn;
    let hydra = with_counters(
        ready(card("Protean Hydra").unwrap()),
        CounterKind::PlusOnePlusOne,
        4,
    );
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [hydra, ready(mountain())])
        .hand(0, [card(spell).unwrap()]);
    let mut driver = Driver::from_scenario(scenario)?;
    let id = driver.id_of("Protean Hydra")?;
    let counters = |driver: &Driver| {
        driver
            .state()
            .object(id)
            .unwrap()
            .permanent()
            .unwrap()
            .counters_of(&CounterKind::PlusOnePlusOne)
    };
    driver.begin_cast(spell)?;
    let hydra = driver.target_named("Protean Hydra")?;
    driver.choose_targets(&[hydra])?;
    driver.pay_tapping(&["Mountain"], 0)?;
    settle(&mut driver, None)?;
    assert_eq!(counters(&driver), 4 - damage);
    driver.idle_until(2, Step::Upkeep)?;
    assert_eq!(counters(&driver), 4 - damage + 2 * damage);
    replays(&driver);
    Ok(())
}
