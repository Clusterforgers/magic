//! Goal: the Gloamwood innates that ask for choices act as written in real games against one to four heroes: The Hag's
//! Due makes only the hero in the spotlight sacrifice a creature unless they pay its {N}, and Hedge Blessing puts a
//! counter on each of up to N of the boss's creatures, never more than it has.
use std::sync::Arc;

use mtg_campaign::content;
use mtg_campaign::innate::innate_id;
use mtg_campaign::scale::PHASE_ROUNDS;
use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::house::HouseRules;
use mtg_core::ids::PlayerId;
use mtg_core::teams::{TeamMode, Teams};
use mtg_engine::choose::{TargetChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::phases::{BossPhase, PhaseStart};
use mtg_engine::spotlight;
use mtg_engine::turn::Step;
use mtg_engine::w6b_emblems::emblem_definition;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn pid(seat: usize) -> PlayerId {
    PlayerId::from_index(seat)
}

fn custom(id: &str) -> Arc<CardDefinition> {
    content().unwrap().card(id).unwrap().definition()
}

/// `heroes` heroes (seats 0 to `heroes - 1`, one team: a Spirit emblem, two ready Bears and `lands` ready Mountains
/// each) against the Gloamwood boss `id` (the last seat: `imps` ready 1/1 Imps, its innate at `level` and its round
/// phases), all on libraries of Mountains; the first hero acts first in turn 1.
fn teams_table(heroes: usize, lands: usize, id: &str, level: u8, imps: usize) -> Scenario {
    let boss = heroes;
    let mut seats = vec![0; heroes];
    seats.push(1);
    let house = HouseRules {
        teams: Some(Teams {
            mode: TeamMode::Teams,
            seats,
        }),
        ..HouseRules::default()
    };
    let mut scenario = Scenario::new(heroes + 1).house_rules(house);
    for hero in 0..heroes {
        let mut own: Vec<_> = (0..2).map(|_| ready(free_creature("Bear", 2, 2))).collect();
        own.extend((0..lands).map(|_| ready(mountain())));
        scenario = scenario
            .life(hero, 20)
            .spirit(hero, Arc::new(emblem_definition("Haunt", &[])))
            .battlefield(hero, own);
    }
    for seat in 0..=boss {
        scenario = scenario.library(seat, (0..60).map(|_| mountain()));
    }
    let record = content()
        .unwrap()
        .theme("gloamwood")
        .unwrap()
        .boss(id)
        .unwrap();
    // The round phases make the boss's seat the boss side, which the spotlight reads.
    let phases = record
        .round_phases
        .iter()
        .zip(PHASE_ROUNDS)
        .map(|(card, round)| BossPhase {
            begins: PhaseStart::AtRound(round),
            emblem: custom(card),
        });
    scenario
        .life(boss, 40)
        .battlefield(boss, (0..imps).map(|_| ready(free_creature("Imp", 1, 1))))
        .emblems(boss, vec![custom(&innate_id(&record.innate, level))])
        .phases(boss, phases)
        .at(1, 0, Step::Main1)
}

/// Answers until `stop` holds: the boss (seat `boss`) picks as many legal targets as it may, up to the decision's N, and
/// says yes; a hero pays what it is asked when `pays`, else answers passively.
fn drive(
    driver: &mut Driver,
    boss: usize,
    pays: bool,
    stop: impl Fn(&Driver) -> bool,
) -> Result<(), ScriptError> {
    for _ in 0..5_000 {
        if stop(driver) {
            return Ok(());
        }
        let decision = driver.decision()?.clone();
        let is_boss = decision.player == pid(boss);
        let answer = match &decision.kind {
            DecisionKind::ChooseTargets(o) if is_boss && !o.legal.is_empty() => {
                let most = usize::try_from(o.max).unwrap_or(usize::MAX);
                Answer::ChooseTargets(TargetChoice::Targets(
                    o.legal.iter().copied().take(most).collect(),
                ))
            }
            DecisionKind::YesNo(_) if is_boss || pays => Answer::YesNo(YesNoChoice(true)),
            DecisionKind::PayMana(_) if is_boss || pays => Answer::PayMana(PayChoice::Auto),
            _ => Driver::passive_answer(&decision),
        };
        // A payment the hero cannot make is not offered: it answers passively instead.
        if driver.answer(answer).is_err() {
            driver.answer_passively()?;
        }
    }
    Err(ScriptError::Stuck("drive".into()))
}

fn at(turn: u32, step: Step) -> impl Fn(&Driver) -> bool {
    move |d: &Driver| d.state().turn().number() == turn && d.state().turn().step() == step
}

/// The battlefield permanents named `name` that `seat` controls.
fn count(driver: &Driver, seat: usize, name: &str) -> usize {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|o| o.controller() == pid(seat) && o.card().name == name)
        .count()
}

/// The +1/+1 counters on each of `seat`'s creatures that has any.
fn counters(driver: &Driver, seat: usize) -> Vec<u32> {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|o| o.controller() == pid(seat))
        .filter_map(|o| o.permanent())
        .map(|p| p.counters_of(&CounterKind::PlusOnePlusOne))
        .filter(|n| *n > 0)
        .collect()
}

#[rstest]
#[case::level_one_paid(1, 2, true, 0)]
#[case::level_one_declined(1, 2, false, 1)]
#[case::level_five_too_dear(5, 2, true, 1)]
#[case::level_five_paid(5, 3, true, 0)]
fn the_hags_due_takes_a_creature_from_the_spotlit_hero_unless_they_pay(
    #[values(1, 2, 3, 4)] heroes: usize,
    #[case] level: u8,
    #[case] lands: usize,
    #[case] pays: bool,
    #[case] lost: usize,
) -> Result<(), ScriptError> {
    let boss = heroes;
    let turn = u32::try_from(boss + 1).unwrap();
    let scenario = teams_table(heroes, lands, "nettlewick_hag", level, 1);
    let mut driver = Driver::from_scenario(scenario)?;
    drive(&mut driver, boss, pays, at(turn, Step::Upkeep))?;
    // The Hag's Due is on the stack in the boss's upkeep; it names the hero eyed now.
    let eyed = spotlight::focus(driver.state(), pid(boss)).unwrap().index();
    drive(&mut driver, boss, pays, at(turn, Step::Main1))?;
    for hero in 0..heroes {
        let expected = if hero == eyed { 2 - lost } else { 2 };
        assert_eq!(count(&driver, hero, "Bear"), expected, "hero {hero}");
    }
    Ok(())
}

#[rstest]
#[case::level_one(1, 3, vec![1])]
#[case::level_three(3, 3, vec![1, 1])]
#[case::level_five(5, 3, vec![1, 1, 1])]
#[case::level_five_one_imp(5, 1, vec![1])]
#[case::level_five_no_creature(5, 0, vec![])]
fn hedge_blessing_counters_up_to_n_of_the_bosss_creatures(
    #[values(1, 2, 3, 4)] heroes: usize,
    #[case] level: u8,
    #[case] imps: usize,
    #[case] expected: Vec<u32>,
) -> Result<(), ScriptError> {
    let boss = heroes;
    let turn = u32::try_from(boss + 1).unwrap();
    let scenario =
        teams_table(heroes, 2, "hawthorn_hedge", level, imps).at(turn, boss, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    drive(&mut driver, boss, false, at(turn + 1, Step::Upkeep))?;
    assert_eq!(counters(&driver, boss), expected);
    for hero in 0..heroes {
        assert!(counters(&driver, hero).is_empty(), "hero {hero}");
    }
    Ok(())
}
