//! Goal: a spell whose additional cost is "waterbend {N}" or "waterbend {X}" is castable (Avatar): X is announced with the spell, its caster's untapped artifacts and creatures may each pay {1} of the waterbend part, the cast counts as waterbending, and Water Whip, Crashing Wave and Waterbender's Restoration do what they say, at tables of two to four seats; a spell short of the mana and helpers is not offered.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_engine::choose::{CardsChoice, NumberChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::events::Bending;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{free_creature, island};
use mtg_testkit::corpus;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{names, pid};

fn card(name: &str) -> Arc<CardDefinition> {
    corpus::card(name).unwrap()
}

/// Seat 0 (on turn) holds `spell`, controls two Islands and `bears` 2/2 Bears; seat 1 controls two 3/3 Ogres.
fn r109_game(seats: usize, spell: &str, bears: usize) -> Result<Driver, ScriptError> {
    let mut board = vec![ready(island()), ready(island())];
    board.extend((1..=bears).map(|n| ready(free_creature(&format!("Bear {n}"), 2, 2))));
    let ogres = (1..=2).map(|n| ready(free_creature(&format!("Ogre {n}"), 3, 3)));
    let scenario = (0..seats).fold(
        Scenario::new(seats)
            .battlefield(0, board)
            .battlefield(1, ogres)
            .hand(0, [card(spell)])
            .at(2, 0, Step::Main1),
        |scenario, seat| scenario.library(seat, vec![island(); 8]),
    );
    Driver::from_scenario(scenario)
}

fn r109_castable(driver: &Driver, name: &str) -> bool {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => options
            .castable
            .iter()
            .any(|id| driver.state().object(*id).unwrap().card().name == name),
        _ => false,
    }
}

fn r109_tapped(driver: &Driver, name: &str) -> bool {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .tapped()
}

fn r109_stun(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::Other("stun".to_owned()))
}

/// Begins casting `name` with X = `x`, answers targets with `targets` and pays with both Islands and `helpers` tapped
/// for the waterbend part; then resolves, choosing the first offered for each choice.
fn r109_cast(
    driver: &mut Driver,
    name: &str,
    x: Option<u32>,
    targets: &[&str],
    helpers: &[&str],
) -> Result<(), ScriptError> {
    driver.begin_cast(name)?;
    for _ in 0..40 {
        match driver.decision()?.kind.clone() {
            DecisionKind::ChooseNumber(_) => {
                driver.answer(Answer::ChooseNumber(NumberChoice::Number(x.unwrap())))?;
            }
            DecisionKind::ChooseTargets(_) => {
                let named = targets
                    .iter()
                    .map(|target| driver.target_named(target))
                    .collect::<Result<Vec<_>, _>>()?;
                driver.choose_targets(&named)?;
            }
            DecisionKind::PayMana(_) => {
                driver.pay_helped_by(&["Island", "Island"], &[], helpers, &[], 0)?;
            }
            DecisionKind::ChooseCards(options) => {
                let first = options.from.first().copied().into_iter().collect();
                driver.answer(Answer::ChooseCards(CardsChoice(first)))?;
            }
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().pending_triggers().is_empty() =>
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
    Ok(())
}

fn r109_bent(driver: &Driver) -> usize {
    driver
        .engine()
        .events()
        .iter()
        .filter(|event| {
            matches!(
                event,
                Event::Bent {
                    element: Bending::Water,
                    ..
                }
            )
        })
        .count()
}

fn r109_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

#[rstest]
// {U}{U} and waterbend {5}: five creatures help, four are too few.
#[case::five_helpers(5, true)]
#[case::four_are_short(4, false)]
fn water_whip_is_castable_with_enough_helpers(
    #[values(2, 3, 4)] seats: usize,
    #[case] bears: usize,
    #[case] castable: bool,
) -> Result<(), ScriptError> {
    let driver = r109_game(seats, "Water Whip", bears)?;
    assert_eq!(r109_castable(&driver, "Water Whip"), castable);
    Ok(())
}

#[rstest]
fn water_whip_bends_and_bounces(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let mut driver = r109_game(seats, "Water Whip", 5)?;
    let bears = ["Bear 1", "Bear 2", "Bear 3", "Bear 4", "Bear 5"];
    r109_cast(
        &mut driver,
        "Water Whip",
        None,
        &["Ogre 1", "Ogre 2"],
        &bears,
    )?;
    assert!(bears.iter().all(|bear| r109_tapped(&driver, bear)));
    let hand = names(driver.state(), Zone::Hand(pid(1)));
    assert!(hand.contains(&"Ogre 1".to_owned()) && hand.contains(&"Ogre 2".to_owned()));
    assert_eq!(r109_bent(&driver), 1);
    r109_replays(&driver);
    Ok(())
}

#[rstest]
fn crashing_wave_taps_x_and_stuns_tapped_opponents(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = r109_game(seats, "Crashing Wave", 2)?;
    r109_cast(
        &mut driver,
        "Crashing Wave",
        Some(2),
        &["Ogre 1", "Ogre 2"],
        &["Bear 1", "Bear 2"],
    )?;
    assert!(r109_tapped(&driver, "Ogre 1") && r109_tapped(&driver, "Ogre 2"));
    assert_eq!(
        r109_stun(&driver, "Ogre 1") + r109_stun(&driver, "Ogre 2"),
        3
    );
    assert_eq!(r109_bent(&driver), 1);
    r109_replays(&driver);
    Ok(())
}

#[rstest]
fn crashing_wave_with_x_zero_stuns_nothing_untapped(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = r109_game(seats, "Crashing Wave", 0)?;
    r109_cast(&mut driver, "Crashing Wave", Some(0), &[], &[])?;
    assert!(!r109_tapped(&driver, "Ogre 1"));
    assert_eq!(
        r109_stun(&driver, "Ogre 1") + r109_stun(&driver, "Ogre 2"),
        0
    );
    r109_replays(&driver);
    Ok(())
}

#[rstest]
fn waterbenders_restoration_exiles_x_until_the_end_step(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = r109_game(seats, "Waterbender's Restoration", 3)?;
    r109_cast(
        &mut driver,
        "Waterbender's Restoration",
        Some(1),
        &["Bear 3"],
        &["Bear 1"],
    )?;
    assert!(names(driver.state(), Zone::Exile).contains(&"Bear 3".to_owned()));
    driver.idle_until(3, Step::Upkeep)?;
    assert!(names(driver.state(), Zone::Battlefield).contains(&"Bear 3".to_owned()));
    r109_replays(&driver);
    Ok(())
}
