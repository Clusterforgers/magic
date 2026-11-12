//! Goal: the r30-2b spells with a lasting part play as printed at 2 to 4 seats: Tasha's Hideous Laughter and Dream
//! Harvest exile from each opponent's library until the cards that player exiled reach the total mana value, and Dream
//! Harvest lets its caster cast those cards free this turn; Reckless Blaze adds {R} for each creature its caster
//! controls that it dealt damage and that dies; Glyph of Life gains life for the damage attacking creatures deal to
//! the Wall, and not for an unblocked attacker.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_core::mana::ManaType;
use mtg_engine::choose::{Target, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, island, mountain, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{names, pid};

/// Passes until the stack is empty, answering yes and paying automatically.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..60 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::Priority(_)) && driver.state().stack().is_empty() {
            return Ok(());
        }
        let answer = match &decision.kind {
            DecisionKind::Priority(_) => {
                driver.pass()?;
                continue;
            }
            DecisionKind::YesNo(o) => Answer::YesNo(YesNoChoice(!o.only_no)),
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// A creature card of mana value `value`.
fn costing(name: &str, value: u32) -> Arc<CardDefinition> {
    creature(name, value - 1, &[Color::Red], 2, 2)
}

#[rstest]
#[case::two_seats(2)]
#[case::four_seats(4)]
fn tashas_hideous_laughter_exiles_until_twenty(#[case] seats: usize) -> Result<(), ScriptError> {
    let mut table = Scenario::new(seats)
        .hand(0, [card("Tasha's Hideous Laughter").unwrap()])
        .battlefield(0, (0..3).map(|_| ready(island())))
        .library(0, (0..6).map(|_| costing("Mine", 5)));
    for seat in 1..seats {
        table = table.library(seat, (0..6).map(|_| costing("Five", 5)));
    }
    let mut driver = Driver::from_scenario(table)?;
    driver.cast("Tasha's Hideous Laughter")?;
    settle(&mut driver)?;
    let state = driver.state();
    for seat in 1..seats {
        assert_eq!(
            state.zone(Zone::Library(pid(seat))).len(),
            2,
            "seat {seat} exiled four cards of five"
        );
    }
    assert_eq!(state.zone(Zone::Exile).len(), 4 * (seats - 1));
    assert_eq!(state.zone(Zone::Library(pid(0))).len(), 6);
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
fn dream_harvest_lets_its_caster_cast_the_exiled_cards_free(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let mut table = Scenario::new(seats)
        .hand(0, [card("Dream Harvest").unwrap()])
        .battlefield(0, (0..7).map(|_| ready(island())));
    for seat in 1..seats {
        table = table.library(
            seat,
            [costing("Ogre", 3), costing("Bear", 2), costing("Giant", 6)],
        );
    }
    let mut driver = Driver::from_scenario(table)?;
    driver.cast("Dream Harvest")?;
    settle(&mut driver)?;
    assert_eq!(
        driver.state().zone(Zone::Library(pid(1))).len(),
        1,
        "two cards reach five"
    );
    driver.cast("Ogre")?;
    settle(&mut driver)?;
    let board = names(driver.state(), Zone::Battlefield);
    assert!(
        board.contains(&"Ogre".to_owned()),
        "cast with no mana left: {board:?}"
    );
    Ok(())
}

#[rstest]
#[case::two_seats(2, 1)]
#[case::four_seats(4, 1)]
fn reckless_blaze_adds_red_for_each_creature_of_its_caster_that_dies(
    #[case] seats: usize,
    #[case] red: u32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [card("Reckless Blaze").unwrap()])
            .battlefield(
                0,
                (0..6).map(|_| ready(mountain())).chain([
                    ready(creature("Bear", 1, &[Color::Red], 2, 2)),
                    ready(creature("Giant", 1, &[Color::Red], 7, 7)),
                ]),
            )
            .battlefield(1, [ready(creature("Ogre", 1, &[Color::Red], 3, 3))]),
    )?;
    driver.cast("Reckless Blaze")?;
    settle(&mut driver)?;
    let state = driver.state();
    let board = names(state, Zone::Battlefield);
    assert!(!board.contains(&"Bear".to_owned()) && !board.contains(&"Ogre".to_owned()));
    assert!(board.contains(&"Giant".to_owned()));
    assert_eq!(state.player(pid(0)).unwrap().mana().get(ManaType::Red), red);
    assert_eq!(state.player(pid(1)).unwrap().mana().total(), 0);
    Ok(())
}

/// Seat 1 attacks seat 0 with an Ogre and a Bear; seat 0's Wall blocks `blocked` after Glyph of Life when `glyph`.
fn glyph_combat(seats: usize, glyph: bool, blocked: Option<&str>) -> Result<i32, ScriptError> {
    let wall = Arc::new(
        CardDefinition::clone(&creature("Wall", 1, &[Color::White], 0, 6)).with_subtype("Wall"),
    );
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 1, Step::Main1)
            .hand(0, [card("Glyph of Life").unwrap()])
            .battlefield(0, [ready(plains()), ready(wall)])
            .battlefield(
                1,
                [
                    ready(creature("Ogre", 1, &[Color::Red], 3, 3)),
                    ready(creature("Bear", 1, &[Color::Red], 2, 2)),
                ],
            ),
    )?;
    if glyph {
        while driver.decision()?.player != pid(0) {
            driver.pass()?;
        }
        let wall = driver.id_of("Wall")?;
        let at = Target::Object(driver.state().object(wall).unwrap().reference());
        driver.cast_at("Glyph of Life", &[at])?;
        settle(&mut driver)?;
    }
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Ogre", 0), ("Bear", 0)])?;
    driver.pass_until(Step::DeclareBlockers)?;
    match blocked {
        Some(attacker) => driver.block(&[("Wall", attacker)])?,
        None => driver.block(&[])?,
    };
    driver.idle_until(2, Step::EndCombat)?;
    Ok(driver.state().player(pid(0)).unwrap().life())
}

#[rstest]
#[case::the_ogre_blocked(2, true, Some("Ogre"), 21)]
#[case::the_bear_blocked(4, true, Some("Bear"), 19)]
#[case::nothing_blocked(3, true, None, 15)]
#[case::no_glyph(2, false, Some("Ogre"), 18)]
fn glyph_of_life_gains_what_the_attackers_deal_to_the_wall(
    #[case] seats: usize,
    #[case] glyph: bool,
    #[case] blocked: Option<&str>,
    #[case] life: i32,
) -> Result<(), ScriptError> {
    assert_eq!(glyph_combat(seats, glyph, blocked)?, life);
    Ok(())
}
