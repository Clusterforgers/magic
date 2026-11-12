//! Goal: demonstrate plays whole at tables of two, three and four seats: the caster may copy the spell, and if they do they choose an opponent (asked only when there are several) who also copies it; each copy belongs to its own player, who alone chooses its new targets (the opponent's copy of "target permanent you control" keeps an illegal target when they do not retarget and fizzles); nothing is copied when the caster declines, a copy is never cast, and every game replays to the same state; bot games with the demonstrate cards finish and really copy spells for two players.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::casting::lands;
use crate::common::seeds::fuzz_games;
use crate::common::{name_of, pid};
use mtg_core::ids::PlayerId;
use mtg_engine::choose::{ChoiceOption, OptionChoice, TargetChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{free_creature, island};
use mtg_testkit::{Driver, Scenario, ScriptError, demonstrate_fuzz_setup, play_game, ready};
use rstest::rstest;

/// Seat 0 holds Replication Technique with five Islands and two creatures; every other seat has one creature of its own.
fn table(seats: usize) -> Result<Driver, ScriptError> {
    let mut mine = lands(5, island);
    mine.push(ready(free_creature("Mine A", 2, 2)));
    mine.push(ready(free_creature("Mine B", 3, 3)));
    let scenario = (1..seats).fold(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .hand_named(0, ["Replication Technique"])
            .battlefield(0, mine),
        |scenario, seat| {
            scenario.battlefield(seat, [ready(free_creature(&format!("Foe {seat}"), 1, 1))])
        },
    );
    let scenario = (0..seats).fold(scenario, |scenario, seat| {
        scenario.library(seat, (0..8).map(|_| island()))
    });
    Driver::from_scenario(scenario)
}

/// What the players answer to the questions of the demonstration.
#[derive(Clone, Copy)]
struct Policy {
    /// The caster copies the spell.
    copy: bool,
    /// The seat the caster chooses.
    opponent: usize,
    /// The opponent chooses new targets for their copy.
    retarget: bool,
}

/// Casts the spell at "Mine A" and answers every question by `policy` until the stack is empty again.
fn cast_and_answer(driver: &mut Driver, policy: Policy) -> Result<(), ScriptError> {
    let target = driver.target_named("Mine A")?;
    driver.cast_at("Replication Technique", &[target])?;
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        let answer = match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(()),
            DecisionKind::Priority(_) => {
                driver.pass()?;
                continue;
            }
            DecisionKind::YesNo(options) if options.text.starts_with("Choose new targets") => {
                Answer::YesNo(YesNoChoice(decision.player != pid(0) && policy.retarget))
            }
            DecisionKind::YesNo(_) => {
                assert_eq!(decision.player, pid(0), "the caster decides to copy");
                Answer::YesNo(YesNoChoice(policy.copy))
            }
            DecisionKind::ChooseOption(options) => {
                assert_eq!(decision.player, pid(0), "the caster chooses the opponent");
                let at = options
                    .options
                    .iter()
                    .position(|option| *option == ChoiceOption::Player(pid(policy.opponent)))
                    .expect("the opponent is offered");
                assert!(
                    options
                        .options
                        .iter()
                        .all(|option| *option != ChoiceOption::Player(pid(0))),
                    "only opponents are offered"
                );
                Answer::ChooseOption(OptionChoice(at))
            }
            DecisionKind::ChooseTargets(options) => {
                let foe = format!("Foe {}", decision.player.0);
                let wanted = options
                    .legal
                    .iter()
                    .copied()
                    .find(|target| {
                        target
                            .object()
                            .is_some_and(|o| name_of(driver.state(), o.id) == foe)
                    })
                    .expect("a creature of the copy's controller is a legal target");
                Answer::ChooseTargets(TargetChoice::Targets(vec![wanted]))
            }
            other => panic!("unexpected question: {}", other.name()),
        };
        driver.answer(answer)?;
    }
    panic!("the stack did not empty");
}

fn count(driver: &Driver, name: &str) -> usize {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| name_of(state, **id) == name)
        .count()
}

fn copies(driver: &Driver) -> Vec<PlayerId> {
    driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::SpellCopied(copy) => Some(copy.controller),
            _ => None,
        })
        .collect()
}

#[rstest]
fn declining_copies_nothing(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let mut driver = table(seats)?;
    let policy = Policy {
        copy: false,
        opponent: 1,
        retarget: true,
    };
    cast_and_answer(&mut driver, policy)?;
    assert_eq!(copies(&driver), []);
    assert_eq!(count(&driver, "Mine A"), 2, "the original's token only");
    Ok(())
}

#[rstest]
#[case::retargets(true, 2)]
#[case::keeps_the_illegal_target(false, 1)]
fn the_caster_copies_and_the_chosen_opponent_copies_with_their_own_targets(
    #[case] retarget: bool,
    #[case] foes: usize,
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] chosen: usize,
) -> Result<(), ScriptError> {
    if chosen >= seats {
        return Ok(());
    }
    let mut driver = table(seats)?;
    let policy = Policy {
        copy: true,
        opponent: chosen,
        retarget,
    };
    cast_and_answer(&mut driver, policy)?;
    assert_eq!(
        copies(&driver),
        [pid(0), pid(chosen)],
        "mine first, the opponent's above it"
    );
    assert_eq!(
        count(&driver, "Mine A"),
        3,
        "the spell's and my copy's token"
    );
    assert_eq!(
        count(&driver, &format!("Foe {chosen}")),
        foes,
        "the opponent's copy made a token only with a target of theirs"
    );
    let casts = driver
        .engine()
        .events()
        .iter()
        .filter(|event| matches!(event, Event::SpellCast { .. }))
        .count();
    assert_eq!(casts, 1, "no copy is cast");
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    Ok(())
}

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn bot_games_with_demonstrate_finish_and_copy_spells_for_two_players(#[case] seats: usize) {
    let mut doubled = 0;
    for seed in 0..fuzz_games(60) {
        let setup = demonstrate_fuzz_setup(seats, seed).unwrap();
        let outcome =
            play_game(setup, seed).unwrap_or_else(|failure| panic!("seed {seed}: {failure}"));
        let controllers: Vec<_> = outcome
            .events
            .iter()
            .filter_map(|event| match event {
                Event::SpellCopied(copy) => Some((copy.of, copy.controller)),
                _ => None,
            })
            .collect();
        doubled += controllers
            .windows(2)
            .filter(|pair| pair[0].0 == pair[1].0 && pair[0].1 != pair[1].1)
            .count();
    }
    assert!(
        doubled > 0,
        "a spell was copied by its caster and by an opponent"
    );
}

#[rstest]
#[case::two(2, 31)]
#[case::four(4, 32)]
fn a_game_with_demonstrate_repeats_exactly_from_its_seed(#[case] seats: usize, #[case] seed: u64) {
    let play = || {
        let setup = demonstrate_fuzz_setup(seats, seed).unwrap();
        play_game(setup, seed).unwrap_or_else(|failure| panic!("seed {seed}: {failure}"))
    };
    let (first, second) = (play(), play());
    assert_eq!(first.events, second.events);
    assert_eq!(first.final_hash, second.final_hash);
    let setup = demonstrate_fuzz_setup(seats, seed).unwrap();
    assert_eq!(
        replay(&setup, &first.events).unwrap().state_hash(),
        first.final_hash
    );
}
