//! Goal: Tunnel of Love plays as printed at two to four seats: the chosen opponent picks a creature of theirs, its
//! controller one of its own, and either both are exiled and come back under their owners' control at the next end
//! step, or they fight each other.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::{name_of, names, pid};
use mtg_cards::keyword::Keyword;
use mtg_core::attraction::Lights;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::keywords::has_keyword;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, free_creature, un33_opener};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Answers until seat 0 has priority with an empty stack, saying `exile` to the "may".
fn r6401_settle(driver: &mut Driver, exile: bool) -> Result<(), ScriptError> {
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_)
                if decision.player == pid(0)
                    && driver.state().stack().is_empty()
                    && driver.state().pending_triggers().is_empty()
                    && driver.state().resolving().is_none() =>
            {
                return Ok(());
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::YesNo(_) if decision.player == pid(0) => {
                driver.answer(Answer::YesNo(YesNoChoice(exile)))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack did not empty".to_owned()))
}

#[rstest]
fn r6401_tunnel_of_love_exiles_until_the_end_step_or_fights(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] exile: bool,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Upkeep)
        .attractions(0, [(card("Tunnel of Love").unwrap(), Lights::of(1..=6))])
        .hand(0, [un33_opener("Open One", Some(1))])
        .battlefield(0, [ready(bear())]);
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(free_creature("Ogre", 3, 3))]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Open One")?;
    r6401_settle(&mut driver, exile)?;
    driver.pass_until(Step::Main1)?;
    r6401_settle(&mut driver, exile)?;
    let on_field = |driver: &Driver, name: &str| {
        names(driver.state(), Zone::Battlefield)
            .iter()
            .filter(|n| *n == name)
            .count()
    };
    let ogres = seats - 1;
    if exile {
        assert_eq!(on_field(&driver, "Grizzly Bears"), 0);
        assert_eq!(on_field(&driver, "Ogre"), ogres - 1);
        assert_eq!(names(driver.state(), Zone::Exile).len(), 2);
        driver.idle_until(1, Step::End)?;
        r6401_settle(&mut driver, exile)?;
        assert_eq!(on_field(&driver, "Grizzly Bears"), 1);
        assert_eq!(on_field(&driver, "Ogre"), ogres);
        let ogre_owners: Vec<_> = driver
            .state()
            .zone(Zone::Battlefield)
            .iter()
            .filter_map(|id| driver.state().object(*id))
            .filter(|o| name_of(driver.state(), o.id()) == "Ogre")
            .map(|o| o.controller() == o.owner())
            .collect();
        assert!(ogre_owners.iter().all(|same| *same));
    } else {
        // The 2/2 and a 3/3 fight: the Bears die, the Ogre lives with 2 damage.
        assert_eq!(on_field(&driver, "Grizzly Bears"), 0);
        assert_eq!(on_field(&driver, "Ogre"), ogres);
        let hurt = driver
            .state()
            .zone(Zone::Battlefield)
            .iter()
            .filter_map(|id| driver.state().object(*id))
            .filter(|o| {
                name_of(driver.state(), o.id()) == "Ogre"
                    && o.permanent().is_some_and(|p| p.damage() == 2)
            })
            .count();
        assert_eq!(hurt, 1);
    }
    Ok(())
}

#[rstest]
fn r6401_drop_tower_gives_flying_for_the_turn(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] seed: u64,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .seed(seed)
        .at(1, 0, Step::Upkeep)
        .attractions(0, [(card("Drop Tower").unwrap(), Lights::of(1..=6))])
        .hand(0, [un33_opener("Open One", Some(1))])
        .battlefield(0, [ready(bear())]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Open One")?;
    r6401_settle(&mut driver, false)?;
    driver.pass_until(Step::Main1)?;
    r6401_settle(&mut driver, false)?;
    let bears = driver.id_of("Grizzly Bears")?;
    // The roll to visit came before the effect, so even a 1 there leaves the flying in place.
    assert!(has_keyword(driver.state(), bears, &Keyword::Flying));
    driver.idle_until(1, Step::End)?;
    assert!(has_keyword(driver.state(), bears, &Keyword::Flying));
    driver.idle_until(2, Step::Upkeep)?;
    assert!(!has_keyword(driver.state(), bears, &Keyword::Flying));
    Ok(())
}
