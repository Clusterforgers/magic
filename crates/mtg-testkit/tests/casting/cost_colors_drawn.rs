//! Goal: under the rule that draws the colors of costs anew (`GameRule::CostColorsDrawn`, the Delves uber boss), at
//! tables of two to four seats every hero's spell inside their commander's identity keeps its mana value, stays inside
//! the identity and is cast with the deck's lands at the drawn cost, the draw is the same for the same seed and moves
//! with the seed and the round, and the boss's own costs stay as printed.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::common;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::color::Color;
use mtg_core::mana::ManaCost;
use mtg_engine::cost_colors::{drawn, redrawn};
use mtg_engine::decision::PayChoice;
use mtg_engine::format::Commander;
use mtg_engine::turn::Step;
use mtg_engine::w6b_emblems::emblem_definition;
use mtg_engine::{Answer, Event};
use mtg_testkit::cards::{creature, land};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The commanders' identities of the heroes, by seat: two, three and five colors.
const IDENTITIES: [&[Color]; 3] = [
    &[Color::White, Color::Blue],
    &[Color::Black, Color::Red, Color::Green],
    &[
        Color::White,
        Color::Blue,
        Color::Black,
        Color::Red,
        Color::Green,
    ],
];

/// The boss's emblem: the costs of its opponents' spells have their colors drawn anew.
fn unmaker() -> Arc<CardDefinition> {
    let rule: Ability =
        serde_json::from_str(r#"{"Rule":{"rule":{"CostColorsDrawn":"Opponents"}}}"#).unwrap();
    Arc::new(emblem_definition("The Unmaker", &[rule]))
}

/// The spell in the hand of the hero at `seat`: one generic, two of its first color and one of its second.
fn spell_of(seat: usize) -> Arc<CardDefinition> {
    let identity = IDENTITIES[seat];
    creature(
        &format!("Spell {seat}"),
        1,
        &[identity[0], identity[0], identity[1]],
        2,
        2,
    )
}

/// `seats` seats: the heroes first, each with a commander of their identity, their spell and three lands of each of its
/// colors; the boss last with the emblem, a black spell and Swamps.
fn table(seats: usize, seed: u64) -> Scenario {
    let boss = seats - 1;
    let mut scenario = Scenario::new(seats)
        .seed(seed)
        .format(Arc::new(Commander))
        .emblems(boss, [unmaker()])
        .hand(boss, [creature("Boss Spell", 1, &[Color::Black], 2, 2)])
        .battlefield(boss, (0..4).map(|_| ready(land(Color::Black))))
        .library(boss, (0..30).map(|_| land(Color::Black)));
    for (seat, identity) in IDENTITIES.iter().enumerate().take(boss) {
        scenario = scenario
            .commanders(
                seat,
                [creature(&format!("Leader {seat}"), 0, identity, 2, 2)],
            )
            .hand(seat, [spell_of(seat)])
            .battlefield(
                seat,
                identity
                    .iter()
                    .flat_map(|color| (0..3).map(|_| ready(land(*color)))),
            )
            .library(seat, (0..30).map(|_| land(identity[0])));
    }
    scenario
}

/// The printed cost of the hero at `seat`'s spell.
fn printed(seat: usize) -> ManaCost {
    spell_of(seat).mana_cost.clone().unwrap()
}

#[rstest]
fn every_hero_spell_keeps_its_value_and_identity(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] seed: u64,
) -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(table(seats, seed))?;
    let state = driver.state();
    for (seat, identity) in IDENTITIES.iter().enumerate().take(seats - 1) {
        let printed = printed(seat);
        let now = redrawn(state, pid(seat), &printed);
        assert_ne!(now, printed, "seat {seat}: every color moves");
        assert_eq!(now.mana_value(), printed.mana_value());
        assert!(
            now.colors().iter().all(|c| identity.contains(&c)),
            "seat {seat}: {now} stays inside the identity"
        );
    }
    let boss = pid(seats - 1);
    assert_eq!(drawn(state, boss), None, "the boss's own costs stay");
    Ok(())
}

#[rstest]
fn the_active_hero_casts_at_the_drawn_cost(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] seed: u64,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, seed))?;
    driver.pass_until(Step::Main1)?;
    let now = redrawn(driver.state(), pid(0), &printed(0));
    driver.begin_cast("Spell 0")?;
    assert_eq!(driver.pay_options()?.cost, now);
    let events = driver.answer(Answer::PayMana(PayChoice::Auto))?;
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Event::SpellCast { player, .. } if *player == pid(0))),
        "the spell is cast with the deck's lands"
    );
    Ok(())
}

#[rstest]
fn the_draw_follows_the_seed_and_the_round(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let hero = pid(seats - 2);
    let mut by_seed = BTreeSet::new();
    for seed in 0..12u64 {
        let first = Driver::from_scenario(table(seats, seed))?;
        let again = Driver::from_scenario(table(seats, seed))?;
        let map = drawn(first.state(), hero).unwrap();
        assert_eq!(
            Some(&map),
            drawn(again.state(), hero).as_ref(),
            "seed {seed}"
        );
        by_seed.insert(format!("{map:?}"));
    }
    if seats > 2 {
        assert!(by_seed.len() > 1, "the seed moves the draw: {by_seed:?}");
    }
    let mut driver = Driver::from_scenario(table(seats, 7))?;
    let mut by_round = BTreeSet::new();
    for turn in 2..=u32::try_from(seats * 4).unwrap() {
        driver.idle_until(turn, Step::Upkeep)?;
        let round = driver.state().turn().round();
        let map = drawn(driver.state(), hero).unwrap();
        by_round.insert((round, format!("{map:?}")));
    }
    let rounds: BTreeSet<u32> = by_round.iter().map(|(round, _)| *round).collect();
    assert_eq!(by_round.len(), rounds.len(), "one draw a round");
    if seats > 2 {
        let draws: BTreeSet<&String> = by_round.iter().map(|(_, map)| map).collect();
        assert!(draws.len() > 1, "the round moves the draw");
    }
    Ok(())
}
