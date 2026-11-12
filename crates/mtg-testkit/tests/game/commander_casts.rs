//! Goal: the times a player has cast their commanders from the command zone (`Amount::CommanderCasts`) are read exactly by a static ability (Commander's Insignia pumps a creature once for each cast) and by a trigger that resolves (Jyoti, Moag Ancient makes a token for each cast) at 2 to 4 seats after 0, 1 and 2 casts with the tax paid, a pair of partners counts together, and another seat's commander or Insignia never counts for the controller.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_engine::characteristics;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::format::Commander;
use mtg_engine::manual::{ManualAction, ManualZone};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, forest, island, mountain};
use mtg_testkit::commander_cards::{legend, partner_legend};
use mtg_testkit::corpus;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const SEAT: PlayerId = PlayerId(0);

fn insignia() -> Arc<CardDefinition> {
    corpus::card("Commander's Insignia").unwrap()
}

/// A Commander table where seat 0 has `commanders` in its command zone, a Bear with Commander's Insignia, lands for
/// the casts and Jyoti, Moag Ancient in its hand, and seat 1 has a Bear and an Insignia of its own.
fn table(seats: usize, commanders: Vec<Arc<CardDefinition>>) -> Scenario {
    let mut mine = vec![
        ready(creature("Bear A", 2, &[], 2, 2)),
        ready(insignia()),
        ready(forest()),
        ready(island()),
    ];
    mine.extend((0..14).map(|_| ready(mountain())));
    Scenario::new(seats)
        .format(Arc::new(Commander))
        .commanders(0, commanders)
        .battlefield(0, mine)
        .battlefield(
            1,
            [ready(creature("Bear B", 2, &[], 2, 2)), ready(insignia())],
        )
        .hand_named(0, ["Jyoti, Moag Ancient"])
}

pub(crate) fn single() -> Vec<Arc<CardDefinition>> {
    vec![legend("Cmd", 1, &[Color::Red], 2, 2)]
}

fn pair() -> Vec<Arc<CardDefinition>> {
    vec![
        partner_legend("Cmd", 1, &[Color::Red], 2, 2),
        partner_legend("Mate", 1, &[Color::Red], 2, 2),
    ]
}

/// Lets everything on the stack resolve.
pub(crate) fn resolve(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        if driver.state().stack().is_empty() {
            return Ok(());
        }
        driver.pass()?;
    }
    panic!("the stack never emptied");
}

/// Casts the commander `name` from the command zone for its cost and tax (paid from Mountains, so the Forest and the
/// Island stay for Jyoti), lets it resolve and, unless `last`, puts it back into the command zone.
fn cast_commander(
    driver: &mut Driver,
    name: &str,
    earlier: u32,
    last: bool,
) -> Result<(), ScriptError> {
    let taps = vec!["Mountain"; 2 + 2 * earlier as usize];
    driver.cast_tapping(name, &taps)?;
    resolve(driver)?;
    let id = driver.id_of(name)?;
    if !last {
        driver.manual(ManualAction::Move {
            object: id,
            to: ManualZone::Graveyard,
        })?;
        assert!(matches!(driver.decision()?.kind, DecisionKind::YesNo(_)));
        driver.answer(Answer::YesNo(YesNoChoice(true)))?;
        assert_eq!(
            driver.state().object(id).unwrap().zone(),
            Zone::Command(SEAT)
        );
    }
    Ok(())
}

/// Casts `name` from the command zone `times` times, the last time leaving it on the battlefield.
pub(crate) fn cast_times(driver: &mut Driver, name: &str, times: u32) -> Result<(), ScriptError> {
    for earlier in 0..times {
        cast_commander(driver, name, earlier, earlier + 1 == times)?;
    }
    Ok(())
}

fn power(driver: &Driver, name: &str) -> i32 {
    let id = driver.id_of(name).unwrap();
    characteristics(driver.state(), id).unwrap().power.unwrap()
}

fn tokens(driver: &Driver, name: &str) -> usize {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            let object = driver.state().object(**id).unwrap();
            object.card().name == name && object.controller() == SEAT
        })
        .count()
}

#[rstest]
fn a_static_ability_counts_the_casts_of_the_commander(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] casts: u32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, single()))?;
    assert_eq!(power(&driver, "Bear A"), 2);
    cast_times(&mut driver, "Cmd", casts)?;
    let state = driver.state();
    assert_eq!(state.commander_casts_by(SEAT), casts);
    assert_eq!(state.commander_casts_by(PlayerId(1)), 0);
    // Insignia gives +1/+1 for each cast to its controller's creatures; the Insignia of seat 1 counts seat 1's casts.
    assert_eq!(power(&driver, "Bear A"), 2 + i32::try_from(casts).unwrap());
    assert_eq!(power(&driver, "Bear B"), 2);
    Ok(())
}

#[rstest]
#[case::one_each(1, 1)]
#[case::two_and_one(2, 1)]
#[case::only_the_second(0, 2)]
fn the_partners_of_a_pair_count_together(
    #[values(2, 3, 4)] seats: usize,
    #[case] first: u32,
    #[case] second: u32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, pair()))?;
    // The Mountains are tapped by the first partner's casts, so the second is cast from what is left.
    cast_times(&mut driver, "Cmd", first)?;
    cast_times(&mut driver, "Mate", second)?;
    assert_eq!(driver.state().commander_casts_by(SEAT), first + second);
    assert_eq!(
        power(&driver, "Bear A"),
        2 + i32::try_from(first + second).unwrap()
    );
    Ok(())
}

#[rstest]
fn a_trigger_that_resolves_counts_the_casts_of_the_commander(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] casts: u32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, single()))?;
    cast_times(&mut driver, "Cmd", casts)?;
    driver.cast_tapping(
        "Jyoti, Moag Ancient",
        &["Forest", "Island", "Mountain", "Mountain"],
    )?;
    resolve(&mut driver)?;
    assert_eq!(tokens(&driver, "Jyoti, Moag Ancient"), 1);
    assert_eq!(tokens(&driver, "Forest Dryad"), casts as usize);
    Ok(())
}
