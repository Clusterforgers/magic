//! Goal: Ink-Treader Nephilim copies an instant or sorcery spell that targets only the Nephilim once for each other
//! creature the spell could target, each copy aimed at a different one, whoever cast it; a spell that also targets
//! another creature, or targets another creature only, is not copied; at two to four seats, and each log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_engine::decision::{Answer, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::creature;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::common::spell;

const NEPHILIM: &str = "Ink-Treader Nephilim";

/// "Tap target creature." once, or twice with two targets.
fn r6104_zap(name: &str, places: usize) -> Arc<CardDefinition> {
    let targets = vec![r#"{"what":{"Object":[{"Type":"Creature"}]}}"#; places].join(",");
    let effects = (0..places)
        .map(|at| format!(r#"{{"Tap":{{"what":{{"Target":{at}}}}}}}"#))
        .collect::<Vec<_>>()
        .join(",");
    spell(
        name,
        CardType::Instant,
        &format!("[{targets}]"),
        &format!("[{effects}]"),
    )
}

/// Seat 0 controls the Nephilim and two Bears, every other seat two Ogres; `caster` is active and holds the spells.
fn r6104_table(seats: usize, caster: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(2, caster, Step::Main1)
        .hand(
            caster,
            vec![r6104_zap("Zap", 1), r6104_zap("Double Zap", 2)],
        )
        .battlefield(
            0,
            [ready(card(NEPHILIM).unwrap())]
                .into_iter()
                .chain((0..2).map(|_| ready(creature("Bear", 1, &[], 2, 2)))),
        );
    for seat in 1..seats {
        scenario =
            scenario.battlefield(seat, (0..2).map(|_| ready(creature("Ogre", 2, &[], 3, 3))));
    }
    scenario
}

/// The tapped permanents on the battlefield.
fn r6104_tapped(driver: &Driver) -> usize {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            state
                .object(**id)
                .and_then(|o| o.permanent())
                .is_some_and(mtg_engine::object::PermanentState::tapped)
        })
        .count()
}

/// What the spell aims at: the Nephilim, the Nephilim and then a Bear, or a Bear.
#[derive(Clone, Copy, Debug)]
enum Aim {
    Nephilim,
    NephilimAndBear,
    Bear,
}

#[rstest]
#[case::the_nephilim_alone(Aim::Nephilim, true)]
#[case::the_nephilim_and_another_creature(Aim::NephilimAndBear, false)]
#[case::another_creature(Aim::Bear, false)]
fn r6104_a_spell_that_targets_only_the_nephilim_is_copied_for_each_other_creature(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] caster: usize,
    #[case] aim: Aim,
    #[case] copied: bool,
) {
    let scenario = r6104_table(seats, caster);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let nephilim = driver.target_named(NEPHILIM).unwrap();
    let bear = driver.target_named("Bear").unwrap();
    match aim {
        Aim::Nephilim => driver.cast_at("Zap", &[nephilim]).unwrap(),
        Aim::Bear => driver.cast_at("Zap", &[bear]).unwrap(),
        Aim::NephilimAndBear => {
            driver.begin_cast("Double Zap").unwrap();
            driver.choose_targets(&[nephilim]).unwrap();
            driver.choose_targets(&[bear]).unwrap();
            driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap()
        }
    };
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        driver.answer_passively().unwrap();
    }
    let creatures = 3 + 2 * (seats - 1);
    let expected = match aim {
        _ if copied => creatures,
        Aim::NephilimAndBear => 2,
        _ => 1,
    };
    assert_eq!(r6104_tapped(&driver), expected, "{aim:?}");
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}
