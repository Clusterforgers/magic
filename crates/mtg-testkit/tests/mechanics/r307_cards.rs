//! Goal: the r30-7 pack's cards play whole at tables of two, three and four seats: Kroxa's opponents who discard no nonland card lose 3 life and the others none, Wrathful Raptors redirects the damage a Dinosaur is dealt to a target that isn't a Dinosaur, Spider-Woman's creature stays tapped through its controller's untap step, and every game replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_engine::choose::Target;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, GameSetup, replay};
use mtg_testkit::cards::{burn, forest, free_creature, mountain, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

/// Passes and answers the first legal target, paying automatically, until the stack is empty; `yes` answers a "may".
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..80 {
        match &driver.decision()?.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(()),
            DecisionKind::ChooseTargets(options) => {
                let first = options.legal[0];
                driver.choose_targets(&[first])?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack did not empty".to_owned()))
}

fn assert_replays(driver: &Driver, setup: &GameSetup) {
    let log = driver.engine().events().to_vec();
    let replayed = replay(setup, &log).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    let resumed = Engine::resume(setup.clone(), log).unwrap();
    assert_eq!(resumed.state_hash(), driver.state().state_hash());
}

#[rstest]
fn kroxa_spares_the_opponents_who_discard_a_nonland_card(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let table = || {
        let mut scenario = Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(
                0,
                [
                    ready(swamp()),
                    ready(swamp()),
                    ready(mountain()),
                    ready(mountain()),
                ],
            )
            .hand(0, [card("Kroxa, Titan of Death's Hunger").unwrap()])
            .hand(1, [forest()]);
        if seats > 2 {
            scenario = scenario.hand(2, [free_creature("Keeper", 1, 1)]);
        }
        scenario
    };
    let mut driver = Driver::from_scenario(table())?;
    driver.cast("Kroxa, Titan of Death's Hunger")?;
    settle(&mut driver)?;
    // A land discarded, or an empty hand, costs 3 life; a nonland card discarded costs none.
    assert_eq!(life(&driver, 1), 17);
    if seats > 2 {
        assert_eq!(life(&driver, 2), 20);
    }
    if seats > 3 {
        assert_eq!(life(&driver, 3), 17);
    }
    assert_eq!(life(&driver, 0), 20);
    assert_replays(&driver, &table().setup().unwrap());
    Ok(())
}

#[rstest]
fn wrathful_raptors_sends_the_damage_to_no_dinosaur(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let table = || {
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(
                0,
                [ready(card("Wrathful Raptors").unwrap()), ready(mountain())],
            )
            .hand(0, [burn("Spark", 2)])
    };
    let mut driver = Driver::from_scenario(table())?;
    let raptors = driver.id_of("Wrathful Raptors")?;
    let raptors = driver.state().object(raptors).unwrap().reference();
    driver.cast_at("Spark", &[Target::Object(raptors)])?;
    let mut chosen = 1;
    for _ in 0..80 {
        match &driver.decision()?.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() && chosen == 2 => break,
            DecisionKind::ChooseTargets(options) => {
                let legal = options.legal.clone();
                if chosen == 1 {
                    // The Raptors' own trigger: the Dinosaur itself is no legal target.
                    assert!(!legal.contains(&Target::Object(raptors)));
                    driver.choose_targets(&[Target::Player(pid(1))])?;
                } else {
                    driver.choose_targets(&[Target::Object(raptors)])?;
                }
                chosen += 1;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    assert_eq!(life(&driver, 1), 18);
    assert_replays(&driver, &table().setup().unwrap());
    Ok(())
}

#[rstest]
fn spider_womans_creature_stays_tapped_while_she_is_controlled(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let table = || {
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(0, (0..6).map(|_| ready(plains())).collect::<Vec<_>>())
            .battlefield(1, [ready(free_creature("Victim", 2, 2))])
            .hand(0, [card("Spider-Woman, Secret Agent").unwrap()])
    };
    let mut driver = Driver::from_scenario(table())?;
    driver.cast("Spider-Woman, Secret Agent")?;
    settle(&mut driver)?;
    let victim = driver.id_of("Victim")?;
    let tapped = |driver: &Driver| {
        driver
            .state()
            .object(victim)
            .and_then(|object| object.permanent())
            .is_some_and(mtg_engine::object::PermanentState::tapped)
    };
    assert!(tapped(&driver));
    for _ in 0..400 {
        if driver.state().turn().active() == pid(1) && driver.state().turn().step() == Step::Main1 {
            break;
        }
        let decision = driver.decision()?.clone();
        driver.answer(Driver::passive_answer(&decision))?;
    }
    assert_eq!(
        driver.state().object(victim).unwrap().zone(),
        Zone::Battlefield
    );
    assert!(tapped(&driver));
    assert_replays(&driver, &table().setup().unwrap());
    Ok(())
}
