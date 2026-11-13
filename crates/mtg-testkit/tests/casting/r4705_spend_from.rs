//! Goal: "Spend only mana produced by basic lands to cast this spell." (Imperiosaur) and "... produced by creatures ..."
//! (Myr Superion) are offered and paid only with mana of those sources, from mana abilities or from the pool, never with
//! a nonbasic land's, a creature's or a land's mana that the rule leaves out, at tables of two to four seats; the log
//! replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::{AbilityDef, CardDefinition, ManaAbility};
use mtg_core::card::CardType;
use mtg_core::mana::ManaType;
use mtg_engine::decision::{Answer, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{forest, island};
use mtg_engine::PresetPermanent;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// A nonbasic land that taps for {G}.
fn grove() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Grove")
            .with_type(CardType::Land)
            .with_ability(AbilityDef::Mana(ManaAbility {
                produces: ManaType::Green,
            })),
    )
}

/// A creature that taps for {G}.
fn dork() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Dork")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_ability(AbilityDef::Mana(ManaAbility {
                produces: ManaType::Green,
            })),
    )
}

/// The permanents of seat 0, by a letter each: `F` a Forest, `G` a Grove, `D` a Dork.
fn permanents(spec: &str) -> Vec<PresetPermanent> {
    spec.chars()
        .map(|letter| match letter {
            'F' => ready(forest()),
            'G' => ready(grove()),
            _ => ready(dork()),
        })
        .collect()
}

/// Whether the card can be cast after tapping `floated` for mana first; when it can, it is cast and resolves.
fn cast(seats: usize, card: &str, spec: &str, floated: Option<&str>) -> bool {
    let game = || {
        let scenario = Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .hand_named(0, [card])
            .battlefield(0, permanents(spec));
        (0..seats).fold(scenario, |scenario, seat| {
            scenario.library(seat, (0..8).map(|_| island()))
        })
    };
    let mut driver = Driver::from_scenario(game()).unwrap();
    if let Some(source) = floated {
        driver.tap_for_mana(source).unwrap();
    }
    if driver.begin_cast(card).is_err() {
        return false;
    }
    driver.expect_decision(0, "pay-mana").unwrap();
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    for _ in 0..12 {
        if driver.state().stack().is_empty() {
            break;
        }
        driver.pass().unwrap();
    }
    assert!(driver.id_of(card).is_ok(), "{card} resolved");
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    true
}

#[rstest]
#[case::four_forests("Imperiosaur", "FFFF", None, true)]
#[case::a_nonbasic_land("Imperiosaur", "FFFG", None, false)]
#[case::a_creature("Imperiosaur", "FFFD", None, false)]
#[case::forest_mana_floating("Imperiosaur", "FFFF", Some("Forest"), true)]
#[case::grove_mana_floating("Imperiosaur", "FFFGG", Some("Grove"), false)]
#[case::grove_mana_left_over("Imperiosaur", "FFFFG", Some("Grove"), true)]
#[case::two_creatures("Myr Superion", "DD", None, true)]
#[case::two_lands("Myr Superion", "FG", None, false)]
#[case::a_creature_and_a_land("Myr Superion", "DF", None, false)]
#[case::creature_mana_floating("Myr Superion", "DD", Some("Dork"), true)]
#[case::land_mana_floating("Myr Superion", "DF", Some("Forest"), false)]
fn the_spell_spends_only_mana_of_its_sources(
    #[values(2, 3, 4)] seats: usize,
    #[case] card: &str,
    #[case] spec: &str,
    #[case] floated: Option<&str>,
    #[case] castable: bool,
) {
    assert_eq!(cast(seats, card, spec, floated), castable);
}
