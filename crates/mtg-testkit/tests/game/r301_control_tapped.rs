//! Goal: control taken "for as long as you control ~ and ~ remains tapped" (Rubinia Soulsinger) holds while the source stays tapped under its controller, and ends when the source untaps or another player gains control of it, at tables of two to four seats; "can't phase out" (Spatial Binding) keeps a phasing permanent in, and the log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_core::ids::PlayerId;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::Answer;
use mtg_engine::effects::{Duration, Modification, Selector};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{free_creature, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const ME: PlayerId = PlayerId(0);

fn controller(driver: &Driver, name: &str) -> Option<PlayerId> {
    mtg_engine::characteristics::controller_of(driver.state(), driver.id_of(name).ok()?)
}

/// Seat 0 with Rubinia Soulsinger, the last seat with a Bear, at seat 0's main phase.
fn game(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(u32::try_from(seats).unwrap() + 1, 0, Step::Main1)
        .battlefield(0, [ready(card("Rubinia Soulsinger").unwrap())])
        .battlefield(seats - 1, [ready(free_creature("Bear", 2, 2))]);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..20).map(|_| island()));
    }
    scenario
}

/// Rubinia takes the Bear; the ability resolves.
fn take_the_bear(seats: usize) -> Result<Driver, ScriptError> {
    let mut driver = Driver::from_scenario(game(seats))?;
    driver.begin_activation("Rubinia Soulsinger")?;
    let bear = driver.target_named("Bear")?;
    driver.choose_targets(&[bear])?;
    for _ in 0..40 {
        if driver.state().stack().is_empty() {
            break;
        }
        driver.answer_passively()?;
    }
    Ok(driver)
}

#[rstest]
#[case::two_seats_kept_tapped(2, true)]
#[case::two_seats_untapped(2, false)]
#[case::three_seats_kept_tapped(3, true)]
#[case::four_seats_untapped(4, false)]
fn control_lasts_while_the_source_stays_tapped(
    #[case] seats: usize,
    #[case] keep: bool,
) -> Result<(), ScriptError> {
    let setup = game(seats).setup().unwrap();
    let mut driver = take_the_bear(seats)?;
    assert_eq!(controller(&driver, "Bear"), Some(ME));
    let next = u32::try_from(2 * seats).unwrap() + 1;
    driver.idle_until(next, Step::Untap)?;
    let rubinia = driver.id_of("Rubinia Soulsinger")?;
    let kept = if keep { vec![rubinia] } else { Vec::new() };
    driver.answer(Answer::ChooseCards(CardsChoice(kept)))?;
    driver.idle_until(next, Step::Upkeep)?;
    let owner = PlayerId::from_index(seats - 1);
    assert_eq!(
        controller(&driver, "Bear"),
        Some(if keep { ME } else { owner })
    );
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn control_ends_when_another_player_gains_the_tapped_source(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = take_the_bear(seats)?;
    assert_eq!(controller(&driver, "Bear"), Some(ME));
    let thief = PlayerId::from_index(1);
    let rubinia = driver.id_of("Rubinia Soulsinger")?;
    driver.add_effect(
        Selector::Objects(vec![driver.state().object(rubinia).unwrap().reference()]),
        Modification::SetController(thief),
        Duration::Forever,
    )?;
    assert_eq!(controller(&driver, "Rubinia Soulsinger"), Some(thief));
    // Rubinia stays tapped, but its controller is no longer the one who took the Bear.
    let owner = PlayerId::from_index(seats - 1);
    assert_eq!(controller(&driver, "Bear"), Some(owner));
    Ok(())
}

/// A phasing creature, under the rule that it can't phase out or not: at its controller's untap step it stays in or
/// phases out.
#[rstest]
#[case::two_seats_held(2, true)]
#[case::two_seats_free(2, false)]
#[case::four_seats_held(4, true)]
fn a_permanent_that_cant_phase_out_stays_in(
    #[case] seats: usize,
    #[case] held: bool,
) -> Result<(), ScriptError> {
    let phaser = Arc::new(free_creature("Phaser", 1, 1).as_ref().clone().with_rules(
        mtg_cards::ability::Ability::Keyword(mtg_cards::keyword::Keyword::Phasing),
    ));
    let mut scenario = Scenario::new(seats)
        .at(u32::try_from(seats).unwrap() + 1, 0, Step::Main1)
        .battlefield(0, [ready(phaser), ready(card("Spatial Binding").unwrap())]);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..20).map(|_| island()));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    if held {
        driver.begin_activation("Spatial Binding")?;
        let phaser = driver.target_named("Phaser")?;
        driver.choose_targets(&[phaser])?;
        for _ in 0..40 {
            if driver.state().stack().is_empty() {
                break;
            }
            driver.answer_passively()?;
        }
    }
    let next = u32::try_from(2 * seats).unwrap() + 1;
    driver.idle_until(next, Step::Upkeep)?;
    // A phased-out permanent is treated as though it doesn't exist: it is not on the battlefield.
    assert_eq!(driver.id_of("Phaser").is_ok(), held);
    Ok(())
}
