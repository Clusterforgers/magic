//! Goal: "since your last turn ended" counts only what happened after the caster's latest finished turn, at two to four
//! seats: Premature Burial may target a nonblack creature that entered in an opponent's turn or in this turn, never one
//! that entered in the caster's last turn or before it, nor a black one; and "if you haven't been dealt combat damage
//! since your last turn" holds after a turn with no attack or with burn only, and fails after an attack that dealt combat
//! damage; each log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::choose::Target;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, burn, creature, free_creature, mountain, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// Answers passively until seat `seat` has priority in its main phase `step` with an empty stack.
fn r379_main_of(driver: &mut Driver, seat: usize, step: Step) -> Result<(), ScriptError> {
    for _ in 0..600 {
        let decision = driver.decision()?.clone();
        let state = driver.state();
        if matches!(decision.kind, DecisionKind::Priority(_))
            && decision.player == pid(seat)
            && state.turn().active() == pid(seat)
            && state.turn().step() == step
            && state.stack().is_empty()
        {
            return Ok(());
        }
        driver.answer(Driver::passive_answer(&decision))?;
    }
    Err(ScriptError::Stuck(format!("seat {seat} {step:?}")))
}

/// Casts the named spell, choosing `target` when asked, and lets it resolve.
fn r379_cast(driver: &mut Driver, name: &str, target: Option<Target>) -> Result<(), ScriptError> {
    driver.begin_cast(name)?;
    for _ in 0..30 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(()),
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::ChooseTargets(_) => {
                driver.choose_targets(&[target.unwrap()])?;
            }
            _ => {
                driver.answer(Driver::passive_answer(&decision))?;
            }
        }
    }
    Err(ScriptError::Stuck(name.to_owned()))
}

fn r379_replays(driver: &Driver, scenario: Scenario) {
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

/// Seat 0 holds Premature Burial, two free creatures and lands; seat 1 holds a free creature and a black one.
fn r379_burial_table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(
            0,
            [
                card("Premature Burial").unwrap(),
                free_creature("Early", 1, 1),
                free_creature("Mine", 1, 1),
            ],
        )
        .battlefield(0, [ready(swamp()), ready(swamp())])
        .hand(
            1,
            [
                free_creature("New", 1, 1),
                creature("Shade", 0, &[Color::Black], 1, 1),
            ],
        )
        .battlefield(1, [ready(free_creature("Old", 1, 1)), ready(swamp())]);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| bear()));
    }
    scenario
}

#[rstest]
fn r379_premature_burial_targets_what_entered_since_your_last_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(r379_burial_table(seats))?;
    r379_cast(&mut driver, "Early", None)?;
    r379_main_of(&mut driver, 1, Step::Main1)?;
    r379_cast(&mut driver, "New", None)?;
    r379_cast(&mut driver, "Shade", None)?;
    r379_main_of(&mut driver, 0, Step::Main1)?;
    r379_cast(&mut driver, "Mine", None)?;
    driver.begin_cast("Premature Burial")?;
    let DecisionKind::ChooseTargets(options) = &driver.decision()?.kind else {
        panic!("the target is asked");
    };
    let mut legal = options.legal.clone();
    let mut wanted = vec![driver.target_named("New")?, driver.target_named("Mine")?];
    legal.sort();
    wanted.sort();
    assert_eq!(legal, wanted, "not Old, Early or the black Shade");
    let new = driver.target_named("New")?;
    driver.choose_targets(&[new])?;
    for _ in 0..10 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => driver.answer(Driver::passive_answer(&decision)).map(|_| ())?,
        }
    }
    let state = driver.state();
    assert!(
        state
            .zone(Zone::Graveyard(pid(1)))
            .iter()
            .any(|id| state.object(*id).unwrap().card().name == "New")
    );
    r379_replays(&driver, r379_burial_table(seats));
    Ok(())
}

/// A creature with Marchesa's upkeep trigger.
fn r379_watcher() -> Arc<CardDefinition> {
    let rules: Ability = serde_json::from_str(
        r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"if":{"Not":{"CombatDamagedSinceLastTurn":"You"}},"effects":[{"Draw":{}},{"LoseLife":{"amount":1}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Watcher")
            .with_type(CardType::Creature)
            .with_power_toughness(0, 4)
            .with_rules(rules),
    )
}

/// Seat 0 controls the Watcher; seat 1 has a Raider, a Mountain and a burn spell.
fn r379_watch_table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(r379_watcher())])
        .hand(1, [burn("Zap", 1)])
        .battlefield(1, [ready(free_creature("Raider", 2, 2)), ready(mountain())]);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| bear()));
    }
    scenario
}

#[rstest]
#[case::nothing("nothing", true)]
#[case::burn_only("burn", true)]
#[case::near_miss_combat_damage("attack", false)]
fn r379_marchesa_draws_unless_hurt_in_combat(
    #[values(2, 3, 4)] seats: usize,
    #[case] what: &str,
    #[case] draws: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(r379_watch_table(seats))?;
    match what {
        "burn" => {
            r379_main_of(&mut driver, 1, Step::Main1)?;
            let me = driver.target_player(0);
            r379_cast(&mut driver, "Zap", Some(me))?;
        }
        "attack" => {
            for _ in 0..600 {
                let decision = driver.decision()?.clone();
                if matches!(decision.kind, DecisionKind::DeclareAttackers(_))
                    && decision.player == pid(1)
                {
                    break;
                }
                driver.answer(Driver::passive_answer(&decision))?;
            }
            driver.attack(&[("Raider", 0)])?;
        }
        _ => {}
    }
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    let life = driver.state().player(pid(0)).unwrap().life();
    r379_main_of(&mut driver, 0, Step::Main1)?;
    let state = driver.state();
    let drawn = state.zone(Zone::Hand(pid(0))).len() - hand;
    let lost = life - state.player(pid(0)).unwrap().life();
    assert_eq!(drawn, if draws { 2 } else { 1 }, "the draw step and the trigger");
    let hurt_later = if what == "attack" { 2 } else { 0 };
    assert_eq!(lost, i32::from(draws) + hurt_later, "1 life lost with the card");
    r379_replays(&driver, r379_watch_table(seats));
    Ok(())
}
