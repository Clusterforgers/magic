//! Goal: "if that creature had to attack this combat" (`Condition::P158HadToAttack`) holds for an attacker a requirement
//! made attack as attackers were declared (a goaded creature, one that attacks each combat if able) and for no attacker
//! that attacked freely, so Firkraag, Cunning Instigator gets one counter and one card for each such creature that deals
//! combat damage to one of its controller's opponents and nothing for damage to its controller; at three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{GameSetup, replay};
use mtg_testkit::cards::free_creature;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, attached_to, ready};
use rstest::rstest;

use crate::common::{names, pid};

/// A 2/2 that attacks each combat if able.
fn brute() -> Arc<CardDefinition> {
    let rule: Ability = serde_json::from_str(r#"{"Rule":{"rule":{"MustAttack":"This"}}}"#).unwrap();
    Arc::new(
        CardDefinition::new("Brute")
            .with_type(CardType::Creature)
            .with_power_toughness(2, 2)
            .with_rules(rule),
    )
}

fn resolve_all(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        if driver.state().turn().step() == Step::Main2 && driver.state().stack().is_empty() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("combat".into()))
}

fn counters(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .counters()
        .filter(|(kind, _)| **kind == CounterKind::PlusOnePlusOne)
        .map(|(_, n)| n)
        .sum()
}

fn replays(driver: &Driver, setup: &GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

/// Seat 1 attacks with `attackers`, each at the seat named; the Bear is goaded by seat 0's Psychic Impetus when `goaded`.
#[rstest]
#[case::a_creature_that_must_attack(&[("Brute", 2)], false, 1)]
#[case::one_that_must_and_one_free(&[("Brute", 2), ("Bear", 2)], false, 1)]
#[case::a_goaded_creature(&[("Brute", 2), ("Bear", 2)], true, 2)]
#[case::damage_to_firkraags_controller(&[("Brute", 0)], false, 0)]
fn p158_firkraag_rewards_creatures_that_had_to_attack(
    #[values(3, 4)] seats: usize,
    #[case] attackers: &[(&str, usize)],
    #[case] goaded: bool,
    #[case] rewarded: u32,
) -> Result<(), ScriptError> {
    let game = || {
        let mut field = vec![ready(card("Firkraag, Cunning Instigator").unwrap())];
        if goaded {
            field.push(attached_to(ready(card("Psychic Impetus").unwrap()), 1, 1));
        }
        let mut scenario = Scenario::new(seats)
            .at(2, 1, Step::DeclareAttackers)
            .battlefield(0, field)
            .battlefield(1, [ready(brute()), ready(free_creature("Bear", 2, 2))]);
        for seat in 0..seats {
            scenario = scenario.library(seat, (0..8).map(|_| free_creature("Filler", 1, 1)));
        }
        scenario
    };
    let setup = game().setup().unwrap();
    let mut driver = Driver::from_scenario(game())?;
    let hand = names(driver.state(), Zone::Hand(pid(0))).len();
    driver.attack(attackers)?;
    resolve_all(&mut driver)?;
    assert_eq!(counters(&driver, "Firkraag, Cunning Instigator"), rewarded);
    assert_eq!(
        names(driver.state(), Zone::Hand(pid(0))).len(),
        hand + usize::try_from(rewarded).unwrap()
    );
    replays(&driver, &setup);
    Ok(())
}
