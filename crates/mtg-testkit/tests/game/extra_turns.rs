//! Goal: extra turns and added phases follow the rules at tables of two, three and four seats: an extra turn comes right after the turn it was added in, the latest one added is taken first (several for several players, several for one, one added in an extra turn), the turn order goes on from the seat of the regular turn and an extra turn opens no round (CR 500.7); the extra turn untaps, upkeeps and draws like any other (or begins at its upkeep when its untap step is skipped), resets the land drop and the turn's tallies, and is passed over when its player has left the game; an added phase begins after the phase it was added in, before any phase added earlier, at every place and chained (CR 500.8); and the logs replay to the state they were played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_cards::effect::PhaseKind;
use mtg_cards::refs::Ref;
use mtg_core::ids::PlayerId;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, EngineError, Event, replay};
use mtg_testkit::cards::{forest, island};
use mtg_testkit::{Driver, Scenario, ScriptError, phase_spell, ready, tapped, time_spell};
use rstest::rstest;

/// The turn the scenarios start in.
const START: u32 = 4;

/// What the state says when a turn begins.
#[derive(Debug, PartialEq, Eq)]
struct Began {
    number: u32,
    active: PlayerId,
    extra: bool,
    round: u32,
}

/// Answers passively until the turn `number` has begun; every turn that begins on the way is recorded.
fn play_until(driver: &mut Driver, number: u32, seen: &mut Vec<Began>) -> Result<(), ScriptError> {
    for _ in 0..4_000 {
        let turn = driver.state().turn();
        if seen.last().map(|b| b.number) != Some(turn.number()) {
            seen.push(Began {
                number: turn.number(),
                active: turn.active(),
                extra: turn.is_extra(),
                round: turn.round(),
            });
        }
        if turn.number() >= number && driver.engine().decision().is_some() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck(format!("turn {number}")))
}

fn at_main(seats: usize, active: usize) -> Scenario {
    Scenario::new(seats).at(START, active, Step::Main1)
}

/// The same table one step before the first main phase, for the tests that count the phases as they begin.
fn at_upkeep(seats: usize, active: usize) -> Scenario {
    Scenario::new(seats).at(START, active, Step::Upkeep)
}

/// The sorcery is cast in the first main phase and resolves.
fn cast_and_resolve(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    driver.cast(name)?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    Ok(())
}

#[rstest]
#[case::mine_in_two(2, 0, Ref::You, 1, &[(1, 0, true), (2, 1, false)])]
#[case::two_of_mine(2, 1, Ref::You, 2, &[(1, 1, true), (2, 1, true), (3, 0, false)])]
#[case::each_player_latest_first(3, 0, Ref::EachPlayer, 1, &[(1, 2, true), (2, 1, true), (3, 0, true), (4, 1, false)])]
#[case::the_opponents_of_four(4, 2, Ref::Opponents, 1, &[(1, 1, true), (2, 0, true), (3, 3, true), (4, 3, false)])]
#[case::three_of_mine(4, 1, Ref::You, 3, &[(1, 1, true), (2, 1, true), (3, 1, true), (4, 2, false)])]
#[case::two_for_each_of_two(2, 0, Ref::EachPlayer, 2, &[(1, 1, true), (2, 1, true), (3, 0, true), (4, 0, true), (5, 1, false)])]
fn extra_turns_come_right_after_this_one_and_the_latest_is_taken_first(
    #[case] seats: usize,
    #[case] active: usize,
    #[case] who: Ref,
    #[case] count: i32,
    #[case] expected: &[(u32, usize, bool)],
) -> Result<(), ScriptError> {
    let walk = time_spell("Hourglass", who, count, false);
    let mut driver = Driver::from_scenario(at_main(seats, active).hand(active, [walk]))?;
    cast_and_resolve(&mut driver, "Hourglass")?;
    assert_eq!(
        driver.state().extra_turns().len(),
        expected.iter().filter(|(_, _, extra)| *extra).count()
    );
    let last = START + u32::try_from(expected.len()).unwrap();
    let mut seen = Vec::new();
    play_until(&mut driver, last, &mut seen)?;
    let began: Vec<_> = seen.iter().map(|b| (b.number, b.active, b.extra)).collect();
    let mut want = vec![(START, pid(active), false)];
    want.extend(
        expected
            .iter()
            .map(|(offset, seat, extra)| (START + offset, pid(*seat), *extra)),
    );
    assert_eq!(began, want);
    assert_eq!(driver.state().extra_turns(), []);
    // An extra turn opens no round: the rounds count the regular turns only.
    let rounds: Vec<_> = seen.iter().map(|b| (b.extra, b.round)).collect();
    for pair in rounds.windows(2) {
        if pair[1].0 {
            assert_eq!(
                pair[1].1, pair[0].1,
                "an extra turn is part of the round it came in: {rounds:?}"
            );
        }
    }
    Ok(())
}

#[rstest]
fn an_extra_turn_taken_in_an_extra_turn_is_taken_before_the_regular_turns()
-> Result<(), ScriptError> {
    let first = time_spell("First Hourglass", Ref::You, 1, false);
    let second = time_spell("Second Hourglass", Ref::Opponents, 1, false);
    let mut driver = Driver::from_scenario(at_main(3, 0).hand(0, [first, second]))?;
    cast_and_resolve(&mut driver, "First Hourglass")?;
    driver.idle_until(START + 1, Step::Main1)?;
    assert!(driver.state().turn().is_extra());
    cast_and_resolve(&mut driver, "Second Hourglass")?;
    let mut seen = Vec::new();
    play_until(&mut driver, START + 5, &mut seen)?;
    let order: Vec<_> = seen
        .iter()
        .map(|b| (b.number - START, b.active, b.extra))
        .collect();
    // Seats 1 and 2 are added in turn order, so seat 2's turn comes first; then the regular turn after seat 0's.
    assert_eq!(
        order,
        [
            (1, pid(0), true),
            (2, pid(2), true),
            (3, pid(1), true),
            (4, pid(1), false),
            (5, pid(2), false),
        ]
    );
    Ok(())
}

/// The steps that began in `events`, in order.
fn steps(events: &[Event]) -> Vec<Step> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::StepBegan { step } => Some(*step),
            _ => None,
        })
        .collect()
}

#[rstest]
fn the_extra_turn_untaps_and_draws_and_resets_the_land_drop(
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let walk = time_spell("Hourglass", Ref::You, 1, false);
    let scenario = at_main(seats, 0)
        .hand(0, [walk, forest(), forest()])
        .battlefield(0, [tapped(island())]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.play_land("Forest")?;
    assert_eq!(driver.state().turn().lands_played(), 1);
    cast_and_resolve(&mut driver, "Hourglass")?;
    let cast_at = driver.engine().events().len();
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    driver.idle_until(START + 1, Step::Main1)?;
    let events = &driver.engine().events()[cast_at..];
    let began = steps(events);
    let untap = began.iter().position(|step| *step == Step::Untap).unwrap();
    assert_eq!(
        began[untap..],
        [Step::Untap, Step::Upkeep, Step::Draw, Step::Main1]
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::PermanentUntapped { .. }))
    );
    assert_eq!(
        driver.state().zone(Zone::Hand(pid(0))).len(),
        hand + 1,
        "one card drawn in the draw step"
    );
    let turn = driver.state().turn();
    assert_eq!(
        (turn.number(), turn.lands_played(), turn.combats()),
        (START + 1, 0, 0)
    );
    driver.play_land("Forest")?;
    assert_eq!(driver.state().turn().lands_played(), 1);
    Ok(())
}

#[rstest]
fn a_skipped_untap_step_does_not_begin_and_the_lands_stay_tapped() -> Result<(), ScriptError> {
    let walk = time_spell("Hourglass", Ref::You, 1, true);
    let scenario = at_main(2, 0)
        .hand(0, [walk])
        .battlefield(0, [tapped(island())]);
    let mut driver = Driver::from_scenario(scenario)?;
    cast_and_resolve(&mut driver, "Hourglass")?;
    let cast_at = driver.engine().events().len();
    driver.idle_until(START + 1, Step::Main1)?;
    let events = &driver.engine().events()[cast_at..];
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::PermanentUntapped { .. }))
    );
    let began = steps(events);
    let cleanup = began
        .iter()
        .position(|step| *step == Step::Cleanup)
        .unwrap();
    assert_eq!(
        began[cleanup..],
        [Step::Cleanup, Step::Upkeep, Step::Draw, Step::Main1]
    );
    let land = driver.id_of("Island")?;
    assert!(
        driver
            .state()
            .object(land)
            .unwrap()
            .permanent()
            .unwrap()
            .tapped()
    );
    Ok(())
}

#[rstest]
fn a_player_who_has_left_loses_the_extra_turn() -> Result<(), ScriptError> {
    let walk = time_spell("Hourglass", Ref::Opponents, 1, false);
    let mut driver = Driver::from_scenario(at_main(3, 0).hand(0, [walk]))?;
    cast_and_resolve(&mut driver, "Hourglass")?;
    driver.concede(pid(2))?;
    let mut seen = Vec::new();
    play_until(&mut driver, START + 2, &mut seen)?;
    let order: Vec<_> = seen
        .iter()
        .map(|b| (b.number - START, b.active, b.extra))
        .collect();
    assert_eq!(
        order,
        [(0, pid(0), false), (1, pid(1), true), (2, pid(1), false)]
    );
    assert_eq!(driver.state().extra_turns(), []);
    Ok(())
}

/// The first step of each phase that began in `events`.
fn starts(events: &[Event]) -> Vec<Step> {
    steps(events)
        .into_iter()
        .filter(|step| {
            matches!(
                step,
                Step::Untap | Step::Main1 | Step::BeginCombat | Step::Main2 | Step::End
            )
        })
        .collect()
}

/// Passes until the `nth` (from 1) beginning of `step` in the turn, with a decision pending in it.
fn reach(driver: &mut Driver, step: Step, nth: usize) -> Result<(), ScriptError> {
    let from = driver
        .engine()
        .events()
        .iter()
        .rposition(|e| matches!(e, Event::TurnBegan { .. }))
        .unwrap_or(0);
    for _ in 0..4_000 {
        let count = driver.engine().events()[from..]
            .iter()
            .filter(|e| matches!(e, Event::StepBegan { step: s } if *s == step))
            .count();
        if driver.state().turn().step() == step
            && count >= nth
            && driver.engine().decision().is_some()
        {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck(format!("{step:?} #{nth}")))
}

use PhaseKind::{Beginning, Combat, Main};

#[rstest]
#[case::combat_and_main_after_the_first_main(Step::Main1, &[Combat, Main], &[Step::BeginCombat, Step::Main2, Step::BeginCombat, Step::Main2, Step::End])]
#[case::combat_and_main_after_the_second_main(Step::Main2, &[Combat, Main], &[Step::BeginCombat, Step::Main2, Step::End])]
#[case::a_combat_after_combat(Step::BeginCombat, &[Combat], &[Step::BeginCombat, Step::Main2, Step::End])]
#[case::two_combats(Step::Main1, &[Combat, Combat], &[Step::BeginCombat, Step::BeginCombat, Step::BeginCombat, Step::Main2, Step::End])]
#[case::a_beginning_phase(Step::Main2, &[Beginning], &[Step::Untap, Step::End])]
#[case::a_beginning_and_a_combat_phase(Step::Main1, &[Beginning, Combat], &[Step::Untap, Step::BeginCombat, Step::BeginCombat, Step::Main2, Step::End])]
#[case::nothing_after_the_ending_phase(Step::End, &[Combat, Main], &[])]
fn an_added_phase_begins_after_the_phase_it_was_added_in(
    #[values(2, 3, 4)] seats: usize,
    #[case] cast_in: Step,
    #[case] phases: &[PhaseKind],
    #[case] expected: &[Step],
) -> Result<(), ScriptError> {
    let spell = phase_spell("Extra", phases);
    let mut driver = Driver::from_scenario(at_upkeep(seats, 0).hand(0, [spell]))?;
    reach(&mut driver, cast_in, 1)?;
    let cast_at = driver.engine().events().len();
    cast_and_resolve(&mut driver, "Extra")?;
    reach(&mut driver, Step::End, 1)?;
    let events = &driver.engine().events()[cast_at..];
    assert_eq!(starts(events), expected);
    assert_eq!(
        events
            .iter()
            .any(|e| matches!(e, Event::PhasesAdded { .. })),
        cast_in != Step::End
    );
    Ok(())
}

#[rstest]
#[case::chained_in_the_extra_main(Step::Main2, &[Step::BeginCombat, Step::Main2, Step::BeginCombat, Step::Main2, Step::BeginCombat, Step::Main2, Step::End])]
#[case::chained_in_the_extra_combat(Step::BeginCombat, &[Step::BeginCombat, Step::BeginCombat, Step::Main2, Step::Main2, Step::BeginCombat, Step::Main2, Step::End])]
fn added_phases_chain_and_the_latest_comes_first(
    #[values(2, 4)] seats: usize,
    #[case] second_in: Step,
    #[case] expected: &[Step],
) -> Result<(), ScriptError> {
    let first = phase_spell("First Extra", &[Combat, Main]);
    let second = phase_spell("Second Extra", &[Combat, Main]);
    let mut driver = Driver::from_scenario(at_upkeep(seats, 0).hand(0, [first, second]))?;
    reach(&mut driver, Step::Main1, 1)?;
    let cast_at = driver.engine().events().len();
    cast_and_resolve(&mut driver, "First Extra")?;
    // The first phase of that kind to begin after the first spell is the one it added.
    reach(&mut driver, second_in, 1)?;
    cast_and_resolve(&mut driver, "Second Extra")?;
    reach(&mut driver, Step::End, 1)?;
    let events = &driver.engine().events()[cast_at..];
    assert_eq!(starts(events), expected);
    let declared = events
        .iter()
        .filter(|e| matches!(e, Event::AttackersDeclared { .. }))
        .count();
    assert_eq!(declared, 3, "a combat phase of each");
    Ok(())
}

#[rstest]
fn the_combat_phases_of_a_turn_are_counted_and_each_declares_attackers() -> Result<(), ScriptError>
{
    let spell = phase_spell("Extra", &[Combat, Combat]);
    let mut driver = Driver::from_scenario(at_upkeep(2, 0).hand(0, [spell]))?;
    reach(&mut driver, Step::Main1, 1)?;
    driver.cast("Extra")?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    reach(&mut driver, Step::Main2, 1)?;
    assert_eq!(driver.state().turn().combats(), 3);
    assert_eq!(driver.state().turn().phases_ahead(), []);
    reach(&mut driver, Step::End, 1)?;
    let declared = driver
        .engine()
        .events()
        .iter()
        .rev()
        .take_while(|e| !matches!(e, Event::TurnBegan { .. }))
        .filter(|e| matches!(e, Event::AttackersDeclared { .. }))
        .count();
    assert_eq!(declared, 3);
    Ok(())
}

#[rstest]
fn the_next_turn_resets_the_added_phases_and_the_combat_count() -> Result<(), ScriptError> {
    let spell = phase_spell("Extra", &[Combat]);
    let mut driver = Driver::from_scenario(at_upkeep(2, 0).hand(0, [spell]))?;
    reach(&mut driver, Step::Main2, 1)?;
    driver.cast("Extra")?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    assert_eq!(
        driver.state().turn().phases_ahead(),
        [Step::BeginCombat, Step::End]
    );
    driver.idle_until(START + 1, Step::Main1)?;
    let turn = driver.state().turn();
    assert_eq!(turn.phases_ahead(), []);
    assert_eq!(turn.combats(), 0);
    assert!(!turn.is_extra());
    Ok(())
}

#[rstest]
fn the_log_of_extra_turns_and_phases_replays_to_the_state_it_was_played_to(
    #[values(2, 3)] seats: usize,
) -> Result<(), ScriptError> {
    let walk = time_spell("Hourglass", Ref::EachPlayer, 1, true);
    let extra = phase_spell("Extra", &[Combat, Main]);
    let scenario = at_main(seats, 0)
        .hand(0, [walk, extra])
        .battlefield(0, [ready(island())]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Extra")?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    reach(&mut driver, Step::Main2, 2)?;
    cast_and_resolve(&mut driver, "Hourglass")?;
    let mut seen = Vec::new();
    play_until(
        &mut driver,
        START + 1 + u32::try_from(seats).unwrap(),
        &mut seen,
    )?;
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    assert_eq!(replayed.extra_turns(), driver.state().extra_turns());
    assert_eq!(replayed.turn().number(), driver.state().turn().number());
    Ok(())
}

#[rstest]
fn a_log_that_takes_an_extra_turn_nobody_added_does_not_fold() {
    let scenario = at_main(2, 0);
    let setup = scenario.clone().setup().unwrap();
    let engine = scenario.build().unwrap();
    let mut events = engine.events().to_vec();
    events.push(Event::TurnBegan {
        number: START + 1,
        active: pid(0),
        extra: true,
    });
    let error = replay(&setup, &events).unwrap_err();
    assert!(matches!(error, EngineError::InvalidEvent(_)), "{error}");
}

#[rstest]
fn no_phase_is_added_after_the_ending_phase_in_a_log() {
    let setup = |step| Scenario::new(2).at(START, 0, step).setup().unwrap();
    for (step, folds) in [(Step::Main2, true), (Step::End, false)] {
        let engine = Engine::new(setup(step)).unwrap();
        let mut events = engine.events().to_vec();
        events.push(Event::PhasesAdded {
            steps: vec![Step::BeginCombat],
        });
        assert_eq!(replay(&setup(step), &events).is_ok(), folds, "{step:?}");
    }
}
