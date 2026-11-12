//! Goal: a cost that discards a card at random lets the ability read the discarded card (`Paid`): Pyromancy deals damage equal to its mana value and Stormscale Anarch deals 4 instead of 2 only when it was multicolored, at tables of two to four seats, and the log replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_core::mana::ManaCost;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{GameSetup, replay};
use mtg_testkit::cards::{creature, free_creature, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, corpus, ready};
use rstest::rstest;

use crate::common::pid;

fn replays(driver: &Driver, setup: &GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

/// Seat 0 controls `source` and four Mountains in its first main phase with `hand` in hand; every library has cards.
fn table(seats: usize, source: &str, hand: Vec<Arc<CardDefinition>>) -> Scenario {
    let mut battlefield = vec![ready(corpus::card(source).unwrap())];
    battlefield.extend((0..4).map(|_| ready(mountain())));
    (0..seats).fold(
        Scenario::new(seats)
            .at(2, 0, Step::Main1)
            .battlefield(0, battlefield)
            .hand(0, hand),
        |scenario, seat| {
            scenario.library(
                seat,
                (0..3).map(|i| free_creature(&format!("Pad {i}"), 1, 1)),
            )
        },
    )
}

/// Activates the ability of `source` at seat 1, pays it with Mountains and lets the stack resolve.
fn activate_at_seat_one(driver: &mut Driver, source: &str) -> Result<(), ScriptError> {
    driver.begin_activation(source)?;
    let target = driver.target_player(1);
    driver.choose_targets(&[target])?;
    if matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
        let lands = ["Mountain", "Mountain", "Mountain"];
        driver.pay_tapping(&lands, 0)?;
    }
    for _ in 0..40 {
        if driver.state().zone(Zone::Stack).is_empty() {
            break;
        }
        driver.pass()?;
    }
    Ok(())
}

/// The name and mana value of the card seat 0 discarded.
fn discarded(driver: &Driver) -> (String, u32) {
    let state = driver.state();
    let card = state
        .zone(Zone::Graveyard(pid(0)))
        .iter()
        .filter_map(|id| state.object(*id))
        .find(|object| object.card().name.starts_with("Card"))
        .expect("a card was discarded");
    (
        card.card().name.clone(),
        card.card()
            .mana_cost
            .as_ref()
            .map_or(0, ManaCost::mana_value),
    )
}

#[rstest]
#[case::one_card_of_two(vec![2])]
#[case::one_card_of_five(vec![5])]
#[case::a_free_card(vec![0])]
#[case::one_of_three_cards(vec![1, 3, 6])]
fn pyromancy_deals_the_mana_value_of_the_discarded_card(
    #[case] costs: Vec<u32>,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let hand = || -> Vec<Arc<CardDefinition>> {
        costs
            .iter()
            .enumerate()
            .map(|(i, generic)| creature(&format!("Card {i}"), *generic, &[], 1, 1))
            .collect()
    };
    let setup = table(seats, "Pyromancy", hand()).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, "Pyromancy", hand()))?;
    let before = driver.state().player(pid(1)).unwrap().life();
    activate_at_seat_one(&mut driver, "Pyromancy")?;
    let (_, value) = discarded(&driver);
    assert_eq!(
        driver.state().zone(Zone::Hand(pid(0))).len(),
        costs.len() - 1,
        "one card was discarded"
    );
    assert_eq!(
        driver.state().player(pid(1)).unwrap().life(),
        before - i32::try_from(value).unwrap()
    );
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::multicolored_deals_four(&[Color::Red, Color::Green], 4)]
#[case::monocolored_deals_two(&[Color::Red, Color::Red], 2)]
#[case::colorless_deals_two(&[], 2)]
fn stormscale_anarch_deals_four_only_for_a_multicolored_card(
    #[case] colors: &[Color],
    #[case] damage: i32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let hand = || vec![creature("Card", 1, colors, 1, 1)];
    let setup = table(seats, "Stormscale Anarch", hand()).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, "Stormscale Anarch", hand()))?;
    let before = driver.state().player(pid(1)).unwrap().life();
    activate_at_seat_one(&mut driver, "Stormscale Anarch")?;
    assert_eq!(discarded(&driver).0, "Card");
    assert_eq!(
        driver.state().player(pid(1)).unwrap().life(),
        before - damage
    );
    replays(&driver, &setup);
    Ok(())
}
