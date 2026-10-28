//! Goal: a target for each opponent plays at tables of two, three and four seats: Dismantling Wave offers every opponent's artifacts with at most one target of each, refuses two targets of one opponent, destroys what was chosen and nothing else, and every game replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_engine::choose::{Target, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::plains;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::c4_control::replays;
use crate::common::{names, pid};

/// An artifact of no cost named after its seat and letter.
fn c10_dice_relic(seat: usize, letter: char) -> Arc<CardDefinition> {
    Arc::new(CardDefinition::new(format!("Relic {seat}{letter}")).with_type(CardType::Artifact))
}

/// Seat 0 holds Dismantling Wave with four Plains and a relic of its own; every opponent has relics `a` and `b`.
fn c10_dice_table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card("Dismantling Wave").unwrap()])
        .battlefield(
            0,
            (0..4)
                .map(|_| ready(plains()))
                .chain([ready(c10_dice_relic(0, 'a'))]),
        );
    for seat in 1..seats {
        scenario = scenario.battlefield(
            seat,
            [
                ready(c10_dice_relic(seat, 'a')),
                ready(c10_dice_relic(seat, 'b')),
            ],
        );
    }
    scenario
}

/// Passes until the stack is empty and nothing resolves.
fn c10_dice_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

#[rstest]
#[case::every_opponent(&['a', 'b', 'a'])]
#[case::only_the_first(&['b'])]
#[case::none(&[])]
fn c10_dice_one_target_of_each_opponent_is_destroyed(
    #[case] letters: &[char],
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(c10_dice_table(seats))?;
    driver.begin_cast("Dismantling Wave")?;
    let DecisionKind::ChooseTargets(options) = &driver.decision()?.kind else {
        panic!("the spell asks for its targets first");
    };
    assert_eq!(
        (options.min, options.max),
        (0, u32::try_from(seats - 1).unwrap())
    );
    assert_eq!(
        options.legal.len(),
        2 * (seats - 1),
        "only the opponents' artifacts"
    );
    assert_eq!(options.apart.len(), options.legal.len());
    let chosen: Vec<String> = letters
        .iter()
        .take(seats - 1)
        .enumerate()
        .map(|(at, letter)| format!("Relic {}{letter}", at + 1))
        .collect();
    let targets: Vec<Target> = chosen
        .iter()
        .map(|name| driver.target_named(name))
        .collect::<Result<_, _>>()?;
    driver.choose_targets(&targets)?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    c10_dice_settle(&mut driver)?;
    for seat in 0..seats {
        for letter in ['a', 'b'] {
            let name = format!("Relic {seat}{letter}");
            let gone = chosen.contains(&name);
            let yard = names(driver.state(), Zone::Graveyard(pid(seat)));
            if seat == 0 && letter == 'b' {
                continue;
            }
            assert_eq!(yard.contains(&name), gone, "{name}");
        }
    }
    replays(&driver);
    Ok(())
}

#[rstest]
fn c10_dice_two_targets_of_one_opponent_are_refused(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(c10_dice_table(seats))?;
    driver.begin_cast("Dismantling Wave")?;
    let both = vec![
        driver.target_named("Relic 1a")?,
        driver.target_named("Relic 1b")?,
    ];
    let refused = driver.answer(Answer::ChooseTargets(TargetChoice::Targets(both)));
    assert!(refused.is_err(), "two artifacts of one opponent");
    let DecisionKind::ChooseTargets(options) = &driver.decision()?.kind else {
        panic!("the targets are asked again");
    };
    // What a chooser may take together is one target of each opponent, and the first `max` legal ones are those.
    let mut owners = options
        .c10_dice_choosable()
        .iter()
        .map(|target| options.apart[options.legal.iter().position(|l| l == target).unwrap()])
        .collect::<Vec<_>>();
    assert_eq!(options.c10_dice_choosable().len(), seats - 1);
    owners.dedup();
    assert_eq!(owners.len(), seats - 1, "one of each opponent");
    let firsts = options.c10_dice_choosable().to_vec();
    driver.choose_targets(&firsts)?;
    Ok(())
}
