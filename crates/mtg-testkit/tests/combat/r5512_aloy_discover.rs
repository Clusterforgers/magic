//! Goal: "whenever one or more artifact creatures you control attack, discover X, where X is the greatest power among
//! them" (Aloy) triggers once for a group of attacking artifact creatures and reads the greatest power among those
//! attackers alone, not among other attackers, and not at all without an artifact attacker, at two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::forest;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// A vanilla creature of the power given, an artifact one when asked.
fn r5512_body(name: &str, power: i32, artifact: bool) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new(name).with_type(CardType::Creature);
    if artifact {
        card = card.with_type(CardType::Artifact);
    }
    Arc::new(card.with_power_toughness(power, 4))
}

/// Priority passes until the stack is empty, payments made automatically and other questions answered passively.
fn r5512_settle(driver: &mut Driver) {
    for _ in 0..80 {
        match driver.decision().unwrap().kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::Priority(_) => {
                driver.pass().unwrap();
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
}

#[rstest]
#[case::greatest_of_two_artifacts(&["Big Construct", "Small Construct"], true)]
#[case::one_small_artifact(&["Small Construct"], false)]
#[case::a_big_attacker_that_is_no_artifact(&["Small Construct", "Big Brute"], false)]
#[case::no_artifact_attacks(&["Big Brute"], false)]
fn r5512_aloy_discovers_the_greatest_power_among_its_artifacts(
    #[values(2, 3, 4)] seats: usize,
    #[case] attackers: &[&str],
    #[case] found: bool,
) {
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card("Aloy, Savior of Meridian").unwrap()),
                ready(r5512_body("Big Construct", 3, true)),
                ready(r5512_body("Small Construct", 1, true)),
                ready(r5512_body("Big Brute", 5, false)),
            ],
        )
        .library(0, [card("Grizzly Bears").unwrap(), forest(), forest()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.pass_until(Step::DeclareAttackers).unwrap();
    let attacks: Vec<(&str, usize)> = attackers.iter().map(|name| (*name, 1)).collect();
    driver.attack(&attacks).unwrap();
    r5512_settle(&mut driver);
    let me = driver.state().players()[0].id();
    let state = driver.state();
    let in_library = state.zone(Zone::Library(me)).iter().any(|id| {
        state
            .object(*id)
            .is_some_and(|object| object.card().name == "Grizzly Bears")
    });
    assert_eq!(!in_library, found, "{attackers:?}");
}
