//! Goal: the promoted replacement cards of t20b play at two, three and four seats as the rules say: City on Fire and Fiery Emancipation triple the damage their controller's sources deal, once each, and in an order the affected player chooses beside a Furnace of Rath; Notion Thief skips the draws of the other players beyond their draw step and its controller draws for each, and that draw is changed by the controller's own Thought Reflection.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{names, pid};
use mtg_engine::choose::ReplacementChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, free_creature, island, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The permanents of these cards, untapped.
fn permanents(names: &[&str]) -> Vec<mtg_engine::PresetPermanent> {
    names
        .iter()
        .map(|name| ready(card(name).unwrap()))
        .collect()
}

/// Passes priority until the stack is empty, ordering the replacement effects as they come; returns how many times the
/// affected player was asked.
fn settle(driver: &mut Driver) -> Result<u32, ScriptError> {
    let mut asked = 0;
    for _ in 0..400 {
        let Some(decision) = driver.engine().decision().cloned() else {
            return Ok(asked);
        };
        match decision.kind {
            DecisionKind::ChooseReplacement(_) => {
                asked += 1;
                driver.answer(Answer::ChooseReplacement(ReplacementChoice(0)))?;
            }
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty() && driver.state().resolving().is_none() =>
            {
                return Ok(asked);
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn hand(driver: &Driver, seat: usize) -> usize {
    names(driver.state(), Zone::Hand(pid(seat))).len()
}

fn replays(driver: &Driver, setup: &mtg_engine::GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

// Damage

#[rstest]
#[case::city_on_fire(&["City on Fire"], 9, 0)]
#[case::fiery_emancipation(&["Fiery Emancipation"], 9, 0)]
#[case::both_triple_again(&["City on Fire", "Fiery Emancipation"], 27, 1)]
#[case::with_a_furnace(&["City on Fire", "Furnace of Rath"], 18, 1)]
#[case::with_a_furnace_and_both_triplers(
    &["City on Fire", "Fiery Emancipation", "Furnace of Rath"],
    54,
    2
)]
fn damage_my_source_deals_is_tripled_once_by_each_card_in_the_order_the_player_chooses(
    #[values(2, 3, 4)] seats: usize,
    #[case] effects: &[&str],
    #[case] damage: i32,
    #[case] asked_at_least: u32,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(seats).hand(0, [burn("Bolt", 3)]).battlefield(
            0,
            permanents(effects).into_iter().chain([ready(mountain())]),
        )
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let foe = driver.target_player(1);
    driver.cast_at("Bolt", &[foe])?;
    let asked = settle(&mut driver)?;
    assert_eq!(life(&driver, 1), 20 - damage);
    assert!(asked >= asked_at_least, "asked {asked} times");
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn damage_to_another_player_than_the_target_is_not_changed(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats).hand(0, [burn("Bolt", 3)]).battlefield(
            0,
            permanents(&["City on Fire"])
                .into_iter()
                .chain([ready(mountain())]),
        ),
    )?;
    let foe = driver.target_player(1);
    driver.cast_at("Bolt", &[foe])?;
    settle(&mut driver)?;
    assert_eq!(life(&driver, 0), 20);
    assert!((2..seats).all(|seat| life(&driver, seat) == 20));
    Ok(())
}

// Draws

#[rstest]
#[case::alone(&["Notion Thief"], 1)]
#[case::with_thought_reflection(&["Notion Thief", "Thought Reflection"], 2)]
fn the_cards_the_others_draw_beyond_the_draw_step_are_drawn_by_my_controller_instead(
    #[values(2, 3, 4)] seats: usize,
    #[case] effects: &[&str],
    #[case] per_draw: usize,
) -> Result<(), ScriptError> {
    // Windfall: each player discards their hand, then draws as many cards as the most any player discarded.
    let scenario = || {
        let mut scenario = Scenario::new(seats)
            .hand(0, [card("Windfall").unwrap()])
            .battlefield(
                0,
                permanents(effects).into_iter().chain([
                    ready(island()),
                    ready(island()),
                    ready(island()),
                ]),
            );
        for seat in 0..seats {
            scenario = scenario.library(
                seat,
                (0..30).map(|n| free_creature(&format!("Card {seat}-{n}"), 1, 1)),
            );
        }
        scenario.hand(1, [free_creature("A", 1, 1), free_creature("B", 1, 1)])
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.cast("Windfall")?;
    settle(&mut driver)?;
    // Two cards were discarded at most, so each seat draws two: seat 0 its own and one for each draw the others skip.
    assert_eq!(hand(&driver, 0), per_draw * 2 * seats);
    for seat in 1..seats {
        assert_eq!(hand(&driver, seat), 0, "seat {seat}");
        assert_eq!(driver.state().zone(Zone::Library(pid(seat))).len(), 30);
    }
    replays(&driver, &setup);
    Ok(())
}
