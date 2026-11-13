//! Goal: three trigger steps play at two to four seats: Shark Shredder returns a creature card from the damaged player's
//! graveyard tapped and attacking that player; Galloping Lizrog removes X +1/+1 counters from among its controller's
//! creatures and puts twice X on itself (none when they decline); The Motherlode pays four energy to destroy a nonbasic
//! land of the defending player and keep that player's creatures without flying from blocking.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::{NumberChoice, Target, YesNoChoice};
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

/// Answers until the stack is empty and the step is done: yes to a payment when `pay`, X for a number, the passive answer
/// to anything else.
fn r4710_settle(driver: &mut Driver, pay: bool, x: u32) -> Result<(), ScriptError> {
    for _ in 0..120 {
        match &driver.decision()?.kind {
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(pay)))?;
            }
            DecisionKind::ChooseNumber(_) => {
                driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Ok(())
}

/// The +1/+1 counters on the permanent named `name`.
fn r4710_plus(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|o| o.permanent())
        .map_or(0, |p| p.counters_of(&CounterKind::PlusOnePlusOne))
}

/// Seat 0 controls an Elder with two +1/+1 counters and a Youngster with one, casts Galloping Lizrog and removes `x`.
#[rstest]
#[case::declined(2, false, 0)]
#[case::one_counter(3, true, 1)]
#[case::every_counter(4, true, 3)]
fn r4710_lizrog_doubles_the_counters_removed(
    #[case] seats: usize,
    #[case] pay: bool,
    #[case] x: u32,
) -> Result<(), ScriptError> {
    let mut field = vec![
        with_counters(
            ready(free_creature("Elder", 1, 1)),
            CounterKind::PlusOnePlusOne,
            2,
        ),
        with_counters(
            ready(free_creature("Youngster", 1, 1)),
            CounterKind::PlusOnePlusOne,
            1,
        ),
    ];
    field.extend((0..3).map(|_| ready(forest())));
    field.extend((0..2).map(|_| ready(island())));
    let scenario = Scenario::new(seats)
        .battlefield(0, field)
        .hand(0, [card("Galloping Lizrog").unwrap()])
        .at(1, 0, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Galloping Lizrog")?;
    r4710_settle(&mut driver, pay, x)?;
    let removed = if pay { x } else { 0 };
    assert_eq!(r4710_plus(&driver, "Galloping Lizrog"), 2 * removed);
    assert_eq!(
        r4710_plus(&driver, "Elder") + r4710_plus(&driver, "Youngster"),
        3 - removed
    );
    Ok(())
}

/// Seat 0's Shark Shredder attacks `defender`, whose graveyard holds a Grizzly Bears; after combat damage the Bears is
/// seat 0's, tapped and attacking `defender`.
#[rstest]
#[case::two_seats(2, 1)]
#[case::three_seats(3, 2)]
#[case::four_seats(4, 3)]
fn r4710_shark_shredder_returns_a_creature_attacking_that_player(
    #[case] seats: usize,
    #[case] defender: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .battlefield(0, [ready(card("Shark Shredder, Killer Clone").unwrap())])
        .graveyard(defender, [card("Grizzly Bears").unwrap()])
        .at(2, 0, Step::DeclareAttackers);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.attack(&[("Shark Shredder, Killer Clone", defender)])?;
    for _ in 0..120 {
        if driver.state().turn().step() == Step::CombatDamage
            && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
            && driver.state().stack().is_empty()
            && driver
                .state()
                .zone(Zone::Graveyard(PlayerId::from_index(defender)))
                .is_empty()
        {
            break;
        }
        match &driver.decision()?.kind {
            DecisionKind::ChooseTargets(_) => {
                let state = driver.state();
                let card = state.zone(Zone::Graveyard(PlayerId::from_index(defender)))[0];
                let bears = Target::Object(state.object(card).unwrap().reference());
                driver.choose_targets(&[bears])?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    let bears = driver.id_of("Grizzly Bears")?;
    let state = driver.state();
    assert_eq!(
        state.object(bears).map(|o| o.controller()),
        Some(PlayerId::from_index(0))
    );
    assert!(
        state
            .object(bears)
            .and_then(|o| o.permanent())
            .is_some_and(|p| p.tapped())
    );
    let attacking = state
        .combat()
        .attackers()
        .iter()
        .find(|a| a.object() == bears)
        .map(|a| a.target());
    assert_eq!(
        attacking,
        Some(AttackTarget::Player(PlayerId::from_index(defender)))
    );
    Ok(())
}

/// Seat 0 with `energy` energy attacks `defender` with The Motherlode; the defender controls Command Tower and a Bear.
/// Paying destroys the Tower and keeps the Bear from blocking.
#[rstest]
#[case::paid_two_seats(2, 1, 4, true)]
#[case::paid_four_seats(4, 2, 5, true)]
#[case::too_little_energy(3, 1, 3, false)]
fn r4710_motherlode_destroys_a_nonbasic_land_of_the_defender(
    #[case] seats: usize,
    #[case] defender: usize,
    #[case] energy: u32,
    #[case] destroyed: bool,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .battlefield(0, [ready(card("The Motherlode, Excavator").unwrap())])
        .battlefield(
            defender,
            [
                ready(card("Command Tower").unwrap()),
                ready(free_creature("Bear", 2, 2)),
            ],
        )
        .counters(0, CounterKind::Other("energy".into()), energy)
        .at(2, 0, Step::DeclareAttackers);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.attack(&[("The Motherlode, Excavator", defender)])?;
    for _ in 0..60 {
        match &driver.decision()?.kind {
            DecisionKind::YesNo(_) => {
                if driver.answer(Answer::YesNo(YesNoChoice(true))).is_err() {
                    driver.answer(Answer::YesNo(YesNoChoice(false)))?;
                }
            }
            DecisionKind::ChooseTargets(_) => {
                let tower = driver.target_named("Command Tower")?;
                driver.choose_targets(&[tower])?;
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    let graveyard = driver
        .state()
        .zone(Zone::Graveyard(PlayerId::from_index(defender)))
        .len();
    assert_eq!(graveyard, usize::from(destroyed));
    let left = driver
        .state()
        .player(PlayerId::from_index(0))
        .unwrap()
        .counters_of(&CounterKind::Other("energy".into()));
    assert_eq!(left, if destroyed { energy - 4 } else { energy });
    driver.pass_until(Step::DeclareBlockers)?;
    if let DecisionKind::DeclareBlockers(options) = &driver.decision()?.kind {
        let bear = driver.id_of("Bear")?;
        assert_eq!(
            options.blockers.iter().any(|b| b.object == bear),
            !destroyed
        );
    }
    Ok(())
}
