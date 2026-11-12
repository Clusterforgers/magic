//! Goal: the cards of the owner's deck that were played by hand play whole: Abigale gives one other creature of any seat flying, first strike and lifelink counters after it loses its printed abilities, or nothing when no target is chosen.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_cards::keyword::Keyword;
use mtg_engine::decision::DecisionKind;
use mtg_engine::keywords::has_keyword;
use mtg_engine::setup::PresetPermanent;
use mtg_engine::turn::Step;
use mtg_engine::{Engine, GameSetup, replay};
use mtg_testkit::cards::plains;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The log replays to the live state, and an engine rebuilt from it stands at the same decision.
#[allow(clippy::unnecessary_wraps)] // the tests end with it, as they end with their last script step
pub(crate) fn assert_replays(driver: &Driver, setup: &GameSetup) -> Result<(), ScriptError> {
    let log = driver.engine().events().to_vec();
    let replayed = replay(setup, &log).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    let resumed = Engine::resume(setup.clone(), log).unwrap();
    assert_eq!(resumed.state_hash(), driver.state().state_hash());
    assert_eq!(resumed.decision(), driver.engine().decision());
    Ok(())
}

/// Passes priority while something waits on the stack and only priority is asked.
pub(crate) fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    while !driver.state().stack().is_empty()
        && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
    {
        driver.pass()?;
    }
    Ok(())
}

/// Passes priority until the table asks for targets.
pub(crate) fn until_targets(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..8 {
        if matches!(driver.decision()?.kind, DecisionKind::ChooseTargets(_)) {
            return Ok(());
        }
        driver.pass()?;
    }
    Ok(())
}

pub(crate) fn has(driver: &Driver, name: &str, keyword: &Keyword) -> bool {
    has_keyword(driver.state(), driver.id_of(name).unwrap(), keyword)
}

/// `plains` ready Plains and the corpus cards called `names`, all ready.
pub(crate) fn with_lands<const N: usize>(plain: usize, names: [&str; N]) -> Vec<PresetPermanent> {
    let mut permanents = vec![ready(plains()); plain];
    permanents.extend(names.map(|name| ready(card(name).unwrap())));
    permanents
}

pub(crate) fn first_main(seats: usize) -> Scenario {
    Scenario::new(seats).at(1, 0, Step::Main1)
}

#[rstest]
fn abigale_takes_the_abilities_of_one_other_creature_and_gives_it_three_counters(
    #[values(2, 3, 4)] seats: usize,
    #[values("Serra Angel", "Grizzly Bears", "Llanowar Elves")] target: &str,
) -> Result<(), ScriptError> {
    let build = || {
        let mut table = first_main(seats)
            .hand_named(0, ["Abigale, Eloquent First-Year"])
            .battlefield(0, with_lands(2, ["Serra Angel"]));
        for seat in 1..seats {
            table = table.battlefield_named(seat, ["Grizzly Bears", "Llanowar Elves"]);
        }
        table
    };
    let setup = build().setup()?;
    let mut driver = Driver::from_scenario(build())?;
    driver.cast("Abigale, Eloquent First-Year")?;
    until_targets(&mut driver)?;
    let aimed = driver.target_named(target)?;
    driver.choose_targets(&[aimed])?;
    settle(&mut driver)?;
    for keyword in [Keyword::Flying, Keyword::FirstStrike, Keyword::Lifelink] {
        assert!(has(&driver, target, &keyword), "{target} has {keyword:?}");
    }
    assert!(
        !has(&driver, target, &Keyword::Vigilance),
        "{target} lost its abilities"
    );
    assert!(has(
        &driver,
        "Abigale, Eloquent First-Year",
        &Keyword::Flying
    ));
    assert_replays(&driver, &setup)
}

#[rstest]
fn abigale_without_a_target_changes_nothing_else(
    #[values(2, 3)] seats: usize,
) -> Result<(), ScriptError> {
    let build = || {
        first_main(seats)
            .hand_named(0, ["Abigale, Eloquent First-Year"])
            .battlefield(0, with_lands(2, ["Serra Angel"]))
    };
    let setup = build().setup()?;
    let mut driver = Driver::from_scenario(build())?;
    driver.cast("Abigale, Eloquent First-Year")?;
    until_targets(&mut driver)?;
    driver.choose_targets(&[])?;
    settle(&mut driver)?;
    assert!(has(&driver, "Serra Angel", &Keyword::Vigilance));
    assert!(!has(&driver, "Serra Angel", &Keyword::FirstStrike));
    assert_replays(&driver, &setup)
}

#[test]
fn abigale_cannot_aim_at_herself() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        first_main(2)
            .hand_named(0, ["Abigale, Eloquent First-Year"])
            .battlefield(0, with_lands(2, [])),
    )?;
    driver.cast("Abigale, Eloquent First-Year")?;
    // With no other creature the trigger has nothing to aim at and the table never asks for a target.
    for _ in 0..8 {
        assert!(!matches!(
            driver.decision()?.kind,
            DecisionKind::ChooseTargets(_)
        ));
        if driver.state().stack().is_empty() && driver.decision()?.player == pid(0) {
            break;
        }
        driver.pass()?;
    }
    Ok(())
}
