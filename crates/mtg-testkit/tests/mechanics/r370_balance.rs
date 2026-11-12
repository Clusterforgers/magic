//! Goal: a balance (`Effect::Balance`, Restore Balance) leaves every player at any table size with as many lands,
//! creatures and cards in hand as the player with the fewest had at each step, each player choosing what they keep,
//! and the rest go to the graveyard together.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_engine::characteristics::controller_of;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, cost, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// The corpus card with no mana cost, so it is cast without lands.
fn free(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        card(name)
            .unwrap()
            .as_ref()
            .clone()
            .with_mana_cost(cost(0, &[])),
    )
}

/// A land with its own name.
fn named_land(name: &str) -> Arc<CardDefinition> {
    Arc::new(CardDefinition::new(name).with_type(CardType::Land))
}

/// A vanilla card for a hand.
fn filler() -> Arc<CardDefinition> {
    bear()
}

/// Passes priority until the stack is empty; a choice of cards by `seat` keeps `keep`, every other question is answered
/// passively.
fn settle(driver: &mut Driver, keep: Option<(usize, &str)>) -> Result<(), ScriptError> {
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        match decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty() && driver.state().resolving().is_none() =>
            {
                return Ok(());
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::ChooseCards(_)
                if keep.is_some_and(|(seat, _)| decision.player == pid(seat))
                    && driver.state().zone(Zone::Battlefield).iter().any(|id| {
                        driver.state().object(*id).unwrap().card().name == keep.unwrap().1
                    }) =>
            {
                let id = driver.id_of(keep.unwrap().1)?;
                driver.answer(Answer::ChooseCards(CardsChoice(vec![id])))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// How many permanents of the type the seat controls.
fn count(driver: &Driver, seat: usize, card_type: CardType) -> usize {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| controller_of(state, **id) == Some(pid(seat)))
        .filter(|id| {
            state
                .object(**id)
                .unwrap()
                .card()
                .types
                .contains(&card_type)
        })
        .count()
}

#[rstest]
fn r370_every_player_keeps_the_fewest(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    // Seat 0 has the fewest lands (2) and the most creatures; the last seat has the fewest creatures (1). After the cast
    // seat 0 holds one card, the fewest.
    let mut scenario = Scenario::new(seats);
    for seat in 0..seats {
        let lands = if seat == 0 { 2 } else { 2 + seat };
        let creatures = if seat == 0 {
            3
        } else if seat == seats - 1 {
            1
        } else {
            2
        };
        let mut permanents: Vec<_> = (0..lands).map(|_| ready(plains())).collect();
        permanents.extend((0..creatures).map(|_| ready(bear())));
        scenario = scenario.battlefield(seat, permanents);
        let hand: Vec<_> = if seat == 0 {
            vec![free("Restore Balance"), filler()]
        } else {
            (0..3).map(|_| filler()).collect()
        };
        scenario = scenario.hand(seat, hand);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Restore Balance")?;
    settle(&mut driver, None)?;
    for seat in 0..seats {
        assert_eq!(count(&driver, seat, CardType::Land), 2, "seat {seat} lands");
        assert_eq!(
            count(&driver, seat, CardType::Creature),
            1,
            "seat {seat} creatures"
        );
        assert_eq!(
            driver.state().zone(Zone::Hand(pid(seat))).len(),
            1,
            "seat {seat} hand"
        );
    }
    let graveyards: usize = (0..seats)
        .map(|seat| driver.state().zone(Zone::Graveyard(pid(seat))).len())
        .sum();
    // The lands, creatures and cards over the fewest, and Restore Balance itself.
    let over: usize = (1..seats)
        .map(|seat| seat + usize::from(seat != seats - 1) + 2)
        .sum::<usize>()
        + 2
        + 1;
    assert_eq!(graveyards, over);
    Ok(())
}

#[rstest]
fn r370_the_player_chooses_what_they_keep(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let last = seats - 1;
    let mut scenario = Scenario::new(seats).hand(0, [free("Restore Balance")]);
    for seat in 0..seats {
        let permanents = if seat == last {
            vec![ready(plains()), ready(named_land("Keep")), ready(plains())]
        } else {
            vec![ready(plains())]
        };
        scenario = scenario.battlefield(seat, permanents);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Restore Balance")?;
    settle(&mut driver, Some((last, "Keep")))?;
    assert_eq!(count(&driver, last, CardType::Land), 1);
    assert!(driver.id_of("Keep").is_ok(), "the chosen land stays");
    Ok(())
}
