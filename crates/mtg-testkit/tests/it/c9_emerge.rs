//! Goal: emerge casts a creature from a hand by sacrificing a creature and paying the emerge cost less that creature's mana value, harmonize casts a card from its owner's graveyard for its harmonize cost less the power of a creature its caster may tap and exiles it as it resolves; the generic mana never goes below nothing, a cast without the mana left is not offered, and the log replays to the same state at 2 to 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::CastWay;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_engine::{Event, PresetPermanent, replay};
use mtg_testkit::cards::{forest, island, mountain};
use mtg_testkit::corpus;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::{choose_way, lands, pay};
use crate::common::{name_of, names, pid};

fn card(name: &str) -> Arc<CardDefinition> {
    corpus::card(name).unwrap()
}

/// Seat 0 holds `hand`, has `graveyard` in its graveyard and controls `creatures` and `count` lands made by `land`.
fn game(
    seats: usize,
    hand: &[&str],
    graveyard: &[&str],
    creatures: &[&str],
    count: usize,
    land: fn() -> Arc<CardDefinition>,
) -> Result<Driver, ScriptError> {
    let battlefield: Vec<PresetPermanent> = creatures
        .iter()
        .map(|name| ready(card(name)))
        .chain(lands(count, land))
        .collect();
    let scenario = (0..seats)
        .fold(Scenario::new(seats), |scenario, seat| {
            scenario.library(seat, (0..8).map(|_| forest()))
        })
        .hand_named(0, hand.to_vec())
        .graveyard_named(0, graveyard.to_vec())
        .battlefield(0, battlefield);
    Driver::from_scenario(scenario)
}

/// The castable card called `name`, if seat 0 may cast it now.
fn castable(driver: &Driver, name: &str) -> bool {
    match &driver.decision().map(|decision| decision.kind.clone()) {
        Ok(DecisionKind::Priority(options)) => options
            .castable
            .iter()
            .any(|id| name_of(driver.state(), *id) == name),
        _ => false,
    }
}

/// The permanent of seat 0 called `name`.
fn permanent(driver: &Driver, name: &str) -> ObjectId {
    driver.id_of(name).unwrap()
}

/// Casts `name` the way whose label starts with `way`, picks `creature` for the cost when asked, aims at `creature`
/// (or nothing for "up to"), pays and lets the stack resolve.
fn cast(driver: &mut Driver, name: &str, way: &str, creature: &str) -> Result<(), ScriptError> {
    let picked = permanent(driver, creature);
    driver.begin_cast(name)?;
    for _ in 0..12 {
        match driver.decision()?.kind.clone() {
            DecisionKind::ChooseOption(_) => choose_way(driver, way)?,
            DecisionKind::ChooseCards(_) => {
                driver.answer(Answer::ChooseCards(CardsChoice(vec![picked])))?;
            }
            DecisionKind::ChooseTargets(spec) => {
                let targets = if spec.min == 0 {
                    Vec::new()
                } else {
                    vec![driver.target_named(creature)?]
                };
                driver.choose_targets(&targets)?;
            }
            DecisionKind::PayMana(_) => {
                pay(driver)?;
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(()),
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

/// The way of every spell cast so far.
fn cast_ways(driver: &Driver) -> Vec<Option<CastWay>> {
    driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::SpellCast { choices, .. } => Some(choices.way),
            _ => None,
        })
        .collect()
}

/// How many lands of seat 0 are tapped.
fn tapped_lands(driver: &Driver, land: &str) -> usize {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| name_of(driver.state(), **id) == land)
        .filter(|id| {
            driver
                .state()
                .object(**id)
                .and_then(|object| object.permanent())
                .is_some_and(|permanent| permanent.tapped())
        })
        .count()
}

/// The log replays to the state the game is in.
fn assert_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

#[rstest]
// Serra Angel (mana value 5) takes the {5} off: {U}{U} is left.
#[case::the_whole_generic_part("Serra Angel", 2)]
// Grizzly Bears (mana value 2) leaves {3}{U}{U}.
#[case::part_of_it("Grizzly Bears", 5)]
// Another Elder Deep-Fiend (mana value 8) takes off no more than the {5} there is.
#[case::more_than_there_is("Elder Deep-Fiend", 2)]
fn emerge_sacrifices_a_creature_and_takes_its_mana_value_off_the_cost(
    #[case] sacrificed: &str,
    #[case] mana: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let other = if sacrificed == "Serra Angel" {
        "Grizzly Bears"
    } else {
        "Serra Angel"
    };
    let creatures = [sacrificed, other];
    let mut driver = game(seats, &["Elder Deep-Fiend"], &[], &creatures, mana, island)?;
    assert!(castable(&driver, "Elder Deep-Fiend"));
    cast(&mut driver, "Elder Deep-Fiend", "Emerge", sacrificed)?;
    assert_eq!(cast_ways(&driver), [Some(CastWay::Emerge)]);
    assert!(names(driver.state(), Zone::Graveyard(pid(0))).contains(&sacrificed.to_owned()));
    assert_eq!(tapped_lands(&driver, "Island"), mana);
    let fiends = names(driver.state(), Zone::Battlefield)
        .iter()
        .filter(|name| *name == "Elder Deep-Fiend")
        .count();
    assert_eq!(fiends, 1);
    assert_replays(&driver);
    Ok(())
}

#[rstest]
// One Island short of {U}{U}, or of {3}{U}{U} with the Bears: the card is not offered.
#[case::short_of_the_colored_mana("Serra Angel", 1)]
#[case::short_of_the_generic_mana("Grizzly Bears", 4)]
fn emerge_is_not_offered_without_the_mana_left(
    #[case] creature: &str,
    #[case] mana: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let driver = game(seats, &["Elder Deep-Fiend"], &[], &[creature], mana, island)?;
    assert!(!castable(&driver, "Elder Deep-Fiend"));
    assert_replays(&driver);
    Ok(())
}

#[rstest]
// Serra Angel (power 4) taps for the {4}: {R} is left.
#[case::tapping_for_the_whole_generic_part("Harmonize {R}", "Serra Angel", 1)]
// Grizzly Bears (power 2) leaves {2}{R}.
#[case::tapping_for_part_of_it("Harmonize {2}{R}", "Grizzly Bears", 3)]
// No creature tapped: the whole {4}{R}.
#[case::tapping_nothing("Harmonize {4}{R}", "Grizzly Bears", 5)]
fn harmonize_casts_from_the_graveyard_less_the_power_tapped_and_exiles_the_spell(
    #[case] way: &str,
    #[case] creature: &str,
    #[case] mana: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let creatures = ["Serra Angel", "Grizzly Bears"];
    let mut driver = game(seats, &[], &["Wild Ride"], &creatures, mana, mountain)?;
    assert!(castable(&driver, "Wild Ride"));
    let way = if mana == 5 { way } else { "Harmonize" };
    cast(&mut driver, "Wild Ride", way, creature)?;
    assert_eq!(cast_ways(&driver), [Some(CastWay::Harmonize)]);
    assert_eq!(tapped_lands(&driver, "Mountain"), mana);
    assert!(names(driver.state(), Zone::Exile).contains(&"Wild Ride".to_owned()));
    assert!(!names(driver.state(), Zone::Graveyard(pid(0))).contains(&"Wild Ride".to_owned()));
    assert_replays(&driver);
    Ok(())
}

#[rstest]
// With no creature to tap, one Mountain does not pay {4}{R}; nor does a card in a hand have the way.
#[case::no_creature_to_tap(&[], &["Wild Ride"], 1)]
#[case::from_a_hand(&["Wild Ride"], &[], 0)]
fn harmonize_is_not_offered_without_the_mana_or_outside_the_graveyard(
    #[case] hand: &[&str],
    #[case] graveyard: &[&str],
    #[case] mana: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let driver = game(seats, hand, graveyard, &[], mana, mountain)?;
    assert!(!castable(&driver, "Wild Ride"));
    assert_replays(&driver);
    Ok(())
}
