//! Goal: an X that a trigger reads without an `{X}` in the mana cost is the X an action announced, at tables of two, three and four seats: Flameblast Dragon deals the X its controller pays for as it attacks (nothing when they decline), Shark Typhoon makes an X/X Shark for the X of its cycling cost, Rampaging War Mammoth may target up to the X of its cycling cost, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_engine::choose::{NumberChoice, Target, TargetChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{GameSetup, replay};
use mtg_testkit::cards::{cost, island, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// Answers until the stack is empty and nothing resolves: X is `x`, a payment is accepted when `pays`, the targets are
/// the first legal ones up to the most offered (`limit` records that most), mana is paid automatically.
fn w6c_settle(
    driver: &mut Driver,
    x: u32,
    pays: bool,
    target: Option<Target>,
    limit: &mut Option<u32>,
) -> Result<(), ScriptError> {
    for _ in 0..60 {
        let decision = driver.decision()?.clone();
        let state = driver.state();
        if matches!(decision.kind, DecisionKind::Priority(_))
            && state.stack().is_empty()
            && state.resolving().is_none()
        {
            return Ok(());
        }
        let answer = match &decision.kind {
            DecisionKind::ChooseNumber(_) => Answer::ChooseNumber(NumberChoice::Number(x)),
            DecisionKind::YesNo(options) if options.text.starts_with("Pay {X}") => {
                Answer::YesNo(YesNoChoice(pays))
            }
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            DecisionKind::ChooseTargets(options) if decision.player == pid(0) => {
                *limit = Some(options.max);
                let chosen = match target {
                    Some(target) => vec![target],
                    None => options
                        .legal
                        .iter()
                        .take(options.max as usize)
                        .copied()
                        .collect(),
                };
                Answer::ChooseTargets(TargetChoice::Targets(chosen))
            }
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck("the stack did not empty".into()))
}

fn w6c_replays(setup: &GameSetup, driver: &Driver) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

#[rstest]
#[case::two(2, true)]
#[case::four(4, true)]
#[case::nothing(0, true)]
#[case::declined(3, false)]
fn w6c_flameblast_dragon_deals_the_x_paid_as_it_attacks(
    #[case] x: u32,
    #[case] pays: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        let mut permanents = vec![ready(card("Flameblast Dragon").unwrap())];
        permanents.extend((0..6).map(|_| ready(mountain())));
        Scenario::new(seats)
            .at(4, 0, Step::Main1)
            .battlefield(0, permanents)
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let last = seats - 1;
    let life = |driver: &Driver| driver.state().player(pid(last)).unwrap().life();
    let before = life(&driver);
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Flameblast Dragon", 1)])?;
    let target = driver.target_player(last);
    w6c_settle(&mut driver, x, pays, Some(target), &mut None)?;
    let dealt = if pays { i32::try_from(x).unwrap() } else { 0 };
    assert_eq!(before - life(&driver), dealt);
    w6c_replays(&setup, &driver);
    Ok(())
}

#[rstest]
#[case::one(1)]
#[case::three(3)]
#[case::nothing(0)]
fn w6c_shark_typhoon_makes_a_shark_of_the_x_it_was_cycled_for(
    #[case] x: u32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(seats)
            .at(4, 0, Step::Main1)
            .hand(0, [card("Shark Typhoon").unwrap()])
            .battlefield(0, (0..6).map(|_| ready(island())).collect::<Vec<_>>())
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_activation("Shark Typhoon")?;
    w6c_settle(&mut driver, x, true, None, &mut None)?;
    let state = driver.state();
    let sharks: Vec<(i32, i32)> = state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|object| object.card().name == "Shark")
        .map(|object| {
            let chars = mtg_engine::characteristics::characteristics(state, object.id()).unwrap();
            (chars.power.unwrap(), chars.toughness.unwrap())
        })
        .collect();
    // A 0/0 Shark dies to the state-based actions.
    let size = i32::try_from(x).unwrap();
    let expected = if x == 0 {
        Vec::new()
    } else {
        vec![(size, size)]
    };
    assert_eq!(sharks, expected);
    w6c_replays(&setup, &driver);
    Ok(())
}

fn w6c_relic(index: usize) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(format!("Relic {index}"))
            .with_type(CardType::Artifact)
            .with_mana_cost(cost(1, &[])),
    )
}

#[rstest]
#[case::one(1)]
#[case::two(2)]
#[case::more_than_there_are(4)]
fn w6c_rampaging_war_mammoth_targets_up_to_the_x_it_was_cycled_for(
    #[case] x: u32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(seats)
            .at(4, 0, Step::Main1)
            .hand(0, [card("Rampaging War Mammoth").unwrap()])
            .battlefield(0, (0..7).map(|_| ready(mountain())).collect::<Vec<_>>())
            .battlefield(
                seats - 1,
                (0..3).map(|i| ready(w6c_relic(i))).collect::<Vec<_>>(),
            )
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_activation("Rampaging War Mammoth")?;
    let mut limit = None;
    w6c_settle(&mut driver, x, true, None, &mut limit)?;
    assert_eq!(limit, Some(x.min(3)));
    let state = driver.state();
    let left = state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|object| object.card().name.starts_with("Relic"))
        .count();
    assert_eq!(left, 3 - x.min(3) as usize);
    w6c_replays(&setup, &driver);
    Ok(())
}
