//! Goal: damage dealt at the same time is one event for "Whenever ~ is dealt damage" and "Whenever ~ deals damage" (CR 510.2, 603.2c), at tables of two, three and four seats: Strong, the Brutish Thespian blocked by two creatures triggers once for three counters, Hercules counts the first batch of the turn with all its damage, and Phantom Nishoba trampling over a blocker gains the damage to the blocker and to the player in one trigger, not one for each recipient; the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::pid;
use mtg_cards::value::CounterKind;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::free_creature;
use mtg_testkit::{Driver, Scenario, ScriptError, corpus, ready};
use rstest::rstest;

/// Seat 0 controls `attacker` (with `counters` +1/+1 counters), ready; seat 1 controls `blockers` Bears (2/2).
fn table(
    seats: usize,
    attacker: &str,
    counters: u32,
    blockers: usize,
) -> Result<Driver, ScriptError> {
    let mut attacking = ready(corpus::card(attacker).unwrap());
    if counters > 0 {
        attacking
            .counters
            .push((CounterKind::PlusOnePlusOne, counters));
    }
    let bears: Vec<_> = (0..blockers)
        .map(|_| ready(free_creature("Bear", 2, 2)))
        .collect();
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [attacking])
        .battlefield(1, bears);
    Driver::from_scenario(scenario)
}

/// `attacker` attacks seat 1, which blocks it with all its Bears; the first Bear is assigned `first` damage and each
/// other Bear lethal damage, and the rest tramples over. Plays on until the end of combat with the stack empty.
fn fight(
    driver: &mut Driver,
    attacker: &str,
    blockers: usize,
    first: u32,
) -> Result<(), ScriptError> {
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[(attacker, 1)])?;
    for _ in 0..80 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::DeclareBlockers(_) => {
                let chosen: Vec<(&str, &str)> = if decision.player == pid(1) {
                    vec![("Bear", attacker); blockers]
                } else {
                    Vec::new()
                };
                driver.block(&chosen)?;
            }
            DecisionKind::AssignDamage(_) => {
                let mut shares: Vec<(&str, u32)> = vec![("Bear", 2); blockers];
                shares[0].1 = first;
                driver.assign_damage(&shares)?;
            }
            DecisionKind::Priority(_)
                if driver.state().turn().step() == Step::EndCombat
                    && driver.state().stack().is_empty() =>
            {
                return Ok(());
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    panic!("combat did not end");
}

fn counters(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| {
            permanent.counters_of(&CounterKind::PlusOnePlusOne)
        })
}

/// The abilities of `name` put on the stack.
fn triggers(driver: &Driver, name: &str) -> usize {
    let id = driver.id_of(name).unwrap();
    driver
        .engine()
        .events()
        .iter()
        .filter(|event| matches!(event, Event::AbilityPutOnStack(put) if put.source.id == id))
        .count()
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

#[rstest]
#[case::one_blocker(1)]
#[case::two_blockers_at_once(2)]
fn strong_is_dealt_damage_once_however_many_creatures_block_it(
    #[values(2, 3, 4)] seats: usize,
    #[case] blockers: usize,
) -> Result<(), ScriptError> {
    let strong = "Strong, the Brutish Thespian";
    let mut driver = table(seats, strong, 0, blockers)?;
    fight(
        &mut driver,
        strong,
        blockers,
        9 - 2 * u32::try_from(blockers).unwrap(),
    )?;
    assert_eq!(
        triggers(&driver, strong),
        1,
        "one trigger for the damage dealt at once"
    );
    assert_eq!(counters(&driver, strong), 3);
    replays(&driver);
    Ok(())
}

#[rstest]
#[case::one_blocker(1, 1 + 2)]
#[case::two_blockers_add_their_damage(2, 1 + 4)]
fn hercules_counts_all_the_damage_of_the_first_batch(
    #[values(2, 3, 4)] seats: usize,
    #[case] blockers: usize,
    #[case] expected: u32,
) -> Result<(), ScriptError> {
    let hercules = "Hercules, Olympian Hero";
    let mut driver = table(seats, hercules, 0, blockers)?;
    fight(&mut driver, hercules, blockers, 2)?;
    // The attack trigger and one for the first damage of the turn.
    assert_eq!(triggers(&driver, hercules), 2);
    assert_eq!(counters(&driver, hercules), expected);
    replays(&driver);
    Ok(())
}

#[rstest]
#[case::unblocked(0, 7)]
#[case::trampling_over_a_blocker(1, 5)]
fn nishoba_gains_the_damage_to_the_blocker_and_the_player_in_one_trigger(
    #[values(2, 3, 4)] seats: usize,
    #[case] blockers: usize,
    #[case] to_the_player: i32,
) -> Result<(), ScriptError> {
    let nishoba = "Phantom Nishoba";
    let mut driver = table(seats, nishoba, 7, blockers)?;
    let before = life(&driver, 0);
    fight(&mut driver, nishoba, blockers, 2)?;
    assert_eq!(
        triggers(&driver, nishoba),
        1,
        "the blocker and the player are one batch"
    );
    assert_eq!(life(&driver, 0), before + 7);
    assert_eq!(life(&driver, 1), 20 - to_the_player);
    replays(&driver);
    Ok(())
}
