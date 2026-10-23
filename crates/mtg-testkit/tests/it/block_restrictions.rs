//! Goal: the block restrictions read from card text keep the blockers they name from blocking and offer every other one, at two, three and four seats: creatures with less (or greater) power than the attacker (Aura Gnarlid), judged as the layers leave both creatures when blockers are declared, so a pump of either side moves the line; the same line for every creature of its controller (Champion of Lambholt) and for no one else's; "except by creatures with flying" (Silhana Ledgewalker) and "Slivers can't be blocked except by Slivers" (Shifting Sliver); several attackers at once.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;
use common::combat::to_blockers;
use common::keywords::{Unit, card as unit_card};
use common::name_of;
use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_engine::PresetPermanent;
use mtg_engine::decision::DecisionKind;
use mtg_engine::effects::{Duration, Modification};
use mtg_engine::turn::Step;
use mtg_testkit::cards::free_creature;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;
use std::sync::Arc;

/// The corpus card of that name.
fn card(name: &str) -> Result<Arc<CardDefinition>, ScriptError> {
    mtg_testkit::corpus::card(name).map_err(|error| ScriptError::Stuck(error.to_string()))
}

/// The blockers the pending decision offers against the attacker `attacker`, sorted.
fn offered(driver: &Driver, attacker: &str) -> Vec<String> {
    let Ok(decision) = driver.decision() else {
        return Vec::new();
    };
    let DecisionKind::DeclareBlockers(options) = &decision.kind else {
        return Vec::new();
    };
    let mut names: Vec<String> = options
        .blockers
        .iter()
        .filter(|blocker| {
            blocker
                .can_block
                .iter()
                .any(|id| name_of(driver.state(), *id) == attacker)
        })
        .map(|blocker| name_of(driver.state(), blocker.object))
        .collect();
    names.sort();
    names
}

fn units(list: &[Unit]) -> Vec<PresetPermanent> {
    list.iter().map(|unit| ready(unit_card(*unit))).collect()
}

/// Seat 0 about to attack with `attackers`; seat 1 holds `blockers`; the other seats hold a bystander of power 1.
fn table(
    seats: usize,
    attackers: Vec<PresetPermanent>,
    blockers: Vec<PresetPermanent>,
) -> Result<Driver, ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, attackers)
        .battlefield(1, blockers);
    for seat in 2..seats {
        scenario = scenario.battlefield(seat, units(&[("Bystander", 1, 1, "")]));
    }
    Driver::from_scenario(scenario)
}

fn pump(driver: &mut Driver, name: &str, power: i32) -> Result<(), ScriptError> {
    let id = driver.id_of(name)?;
    driver.add_effect(
        driver.on(id),
        Modification::ModifyPowerToughness {
            power,
            toughness: 0,
        },
        Duration::UntilEndOfTurn,
    )?;
    Ok(())
}

/// Blockers of every power against Aura Gnarlid (2/2: creatures with power less than its power can't block it).
#[rstest]
#[case::nothing_smaller_blocks(0, 0, &[("P1", 1), ("P2", 2), ("P3", 3)], &["P2", "P3"])]
#[case::the_attacker_grows(0, 1, &[("P1", 1), ("P2", 2), ("P3", 3)], &["P3"])]
#[case::the_attacker_shrinks(0, -1, &[("P1", 1), ("P2", 2), ("P3", 3)], &["P1", "P2", "P3"])]
#[case::a_counter_on_the_attacker(1, 0, &[("P1", 1), ("P2", 2), ("P3", 3)], &["P3"])]
#[case::no_blocker_is_big_enough(2, 0, &[("P1", 1), ("P2", 2)], &[])]
#[case::an_equal_power_blocks(0, 0, &[("P2", 2)], &["P2"])]
fn creatures_with_less_power_than_aura_gnarlid_cannot_block_it(
    #[values(2, 3, 4)] seats: usize,
    #[case] counters: u32,
    #[case] pumped: i32,
    #[case] powers: &[(&'static str, i32)],
    #[case] expected: &[&str],
) -> Result<(), ScriptError> {
    let gnarlid = with_counters(
        ready(card("Aura Gnarlid")?),
        CounterKind::PlusOnePlusOne,
        counters,
    );
    let blockers = powers
        .iter()
        .map(|(name, power)| ready(free_creature(name, *power, 5)))
        .collect();
    let mut driver = table(seats, vec![gnarlid], blockers)?;
    driver.attack(&[("Aura Gnarlid", 1)])?;
    pump(&mut driver, "Aura Gnarlid", pumped)?;
    to_blockers(&mut driver, 1)?;
    assert_eq!(offered(&driver, "Aura Gnarlid"), expected);
    Ok(())
}

/// A pump of a blocker after the attack moves it across the line.
#[rstest]
#[case::grown_past_the_line(1, 1, true)]
#[case::shrunk_below_the_line(2, -1, false)]
#[case::grown_but_still_below(1, 0, false)]
fn the_power_of_a_blocker_is_judged_when_blockers_are_declared(
    #[case] power: i32,
    #[case] change: i32,
    #[case] offered_to_block: bool,
) -> Result<(), ScriptError> {
    let mut driver = table(
        2,
        vec![ready(card("Aura Gnarlid")?)],
        vec![ready(free_creature("B", power, 5))],
    )?;
    driver.attack(&[("Aura Gnarlid", 1)])?;
    pump(&mut driver, "B", change)?;
    to_blockers(&mut driver, 1)?;
    let expected: &[&str] = if offered_to_block { &["B"] } else { &[] };
    assert_eq!(offered(&driver, "Aura Gnarlid"), expected);
    Ok(())
}

/// Champion of Lambholt (1/1 and a counter per other creature that enters): creatures with less power than its own
/// can't block the creatures its controller controls, whichever of them attacks, and no one else's.
#[rstest]
#[case::power_one_blocks_nothing_away(0, &[("P0", 0), ("P1", 1), ("P3", 3)], &["P1", "P3"])]
#[case::a_counter(1, &[("P1", 1), ("P2", 2), ("P3", 3)], &["P2", "P3"])]
#[case::two_counters(2, &[("P1", 1), ("P2", 2), ("P3", 3)], &["P3"])]
fn champion_of_lambholt_covers_the_creatures_of_its_controller(
    #[values(2, 3, 4)] seats: usize,
    #[case] counters: u32,
    #[case] powers: &[(&'static str, i32)],
    #[case] expected: &[&str],
) -> Result<(), ScriptError> {
    let champion = with_counters(
        ready(card("Champion of Lambholt")?),
        CounterKind::PlusOnePlusOne,
        counters,
    );
    let blockers = powers
        .iter()
        .map(|(name, power)| ready(free_creature(name, *power, 5)))
        .collect();
    let attackers = vec![champion, ready(free_creature("Bear", 2, 2))];
    let mut driver = table(seats, attackers, blockers)?;
    driver.attack(&[("Bear", 1), ("Champion of Lambholt", 1)])?;
    to_blockers(&mut driver, 1)?;
    // Champion of Lambholt is a creature its controller controls too.
    assert_eq!(offered(&driver, "Bear"), expected);
    assert_eq!(offered(&driver, "Champion of Lambholt"), expected);
    Ok(())
}

#[rstest]
fn a_champion_of_another_player_covers_nothing_of_mine(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let blockers = vec![
        ready(card("Champion of Lambholt")?),
        ready(free_creature("P0", 0, 5)),
    ];
    let mut driver = table(seats, vec![ready(free_creature("Bear", 2, 2))], blockers)?;
    driver.attack(&[("Bear", 1)])?;
    to_blockers(&mut driver, 1)?;
    assert_eq!(offered(&driver, "Bear"), ["Champion of Lambholt", "P0"]);
    Ok(())
}

/// Silhana Ledgewalker can't be blocked except by creatures with flying.
#[rstest]
#[case::the_ground(&[("G", 3, 3, "")], &[])]
#[case::reach_is_not_flying(&[("R", 3, 3, "reach")], &[])]
#[case::a_flier(&[("F", 1, 1, "flying")], &["F"])]
#[case::fliers_among_the_ground(&[("G1", 3, 3, ""), ("F1", 1, 1, "flying"), ("F2", 2, 2, "flying reach"), ("G2", 1, 1, "")], &["F1", "F2"])]
fn silhana_ledgewalker_is_blocked_only_by_fliers(
    #[values(2, 3, 4)] seats: usize,
    #[case] blockers: &[Unit],
    #[case] expected: &[&str],
) -> Result<(), ScriptError> {
    let mut driver = table(
        seats,
        vec![ready(card("Silhana Ledgewalker")?)],
        units(blockers),
    )?;
    driver.attack(&[("Silhana Ledgewalker", 1)])?;
    to_blockers(&mut driver, 1)?;
    assert_eq!(offered(&driver, "Silhana Ledgewalker"), expected);
    Ok(())
}

/// Shifting Sliver: every Sliver can be blocked only by Slivers, the controller's own included, and a creature that
/// is not a Sliver is blocked as usual.
#[rstest]
#[case::a_sliver_attacks("Shifting Sliver", &["Gemhide Sliver"])]
#[case::another_sliver_attacks("Manaweft Sliver", &["Gemhide Sliver"])]
#[case::a_bear_attacks("Bear", &["Bear Blocker", "Gemhide Sliver"])]
fn slivers_are_blocked_only_by_slivers(
    #[values(2, 3, 4)] seats: usize,
    #[case] attacker: &str,
    #[case] expected: &[&str],
) -> Result<(), ScriptError> {
    let attackers = vec![
        ready(card("Shifting Sliver")?),
        ready(card("Manaweft Sliver")?),
        ready(free_creature("Bear", 2, 2)),
    ];
    let blockers = vec![
        ready(card("Gemhide Sliver")?),
        ready(free_creature("Bear Blocker", 2, 2)),
    ];
    let mut driver = table(seats, attackers, blockers)?;
    driver.attack(&[(attacker, 1)])?;
    to_blockers(&mut driver, 1)?;
    assert_eq!(offered(&driver, attacker), expected);
    Ok(())
}

/// Without Shifting Sliver, a Sliver is blocked as usual.
#[test]
fn without_shifting_sliver_a_sliver_is_blocked_by_anything() -> Result<(), ScriptError> {
    let mut driver = table(
        2,
        vec![ready(card("Manaweft Sliver")?)],
        vec![
            ready(card("Gemhide Sliver")?),
            ready(free_creature("Bear", 2, 2)),
        ],
    )?;
    driver.attack(&[("Manaweft Sliver", 1)])?;
    to_blockers(&mut driver, 1)?;
    assert_eq!(
        offered(&driver, "Manaweft Sliver"),
        ["Bear", "Gemhide Sliver"]
    );
    Ok(())
}
