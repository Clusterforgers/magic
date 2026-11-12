//! Goal: Class enchantments play as the rules say (CR 716) at tables of two, three and four seats: a Class enters at level 1, its level up abilities are offered at sorcery speed and one level at a time, each level adds its abilities and keeps the lower ones, a Class that leaves and returns starts at level 1 again, and the log of whole games with Classes replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_engine::Event;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::manual::{ManualAction, ManualZone};
use mtg_engine::replay;
use mtg_engine::rng::GameRng;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, free_creature, island, land};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, PlayOptions, Scenario, ScriptError, play_game_with, ready};
use rstest::rstest;

use crate::common::seeds::{for_each_seed, fuzz_games};

/// The id of the card with this name on the battlefield.
fn on_board(driver: &Driver, name: &str) -> ObjectId {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| {
            driver
                .state()
                .object(*id)
                .is_some_and(|object| object.card().name == name)
        })
        .unwrap()
}

fn level(driver: &Driver, name: &str) -> u32 {
    let object = driver.state().object(on_board(driver, name)).unwrap();
    object.permanent().unwrap().level()
}

fn counters(driver: &Driver, name: &str) -> u32 {
    let object = driver.state().object(on_board(driver, name)).unwrap();
    object
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::PlusOnePlusOne)
}

fn hand_size(driver: &Driver, seat: usize) -> usize {
    driver
        .state()
        .zone(Zone::Hand(crate::common::pid(seat)))
        .len()
}

/// Passes until nothing is on the stack.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..60 {
        let busy = !driver.state().stack().is_empty() || driver.state().resolving().is_some();
        if !busy {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Activates the one level up ability the Class offers, paying with `lands` Islands.
fn level_up(driver: &mut Driver, name: &str, lands: usize) -> Result<(), ScriptError> {
    let offered = driver.offered_abilities(name)?;
    assert_eq!(offered.len(), 1, "{name} offers {offered:?}");
    driver.begin_ability(name, offered[0])?;
    driver.pay_tapping(&vec!["Island"; lands], 0)?;
    settle(driver)
}

/// Seat 0 with a Wizard Class, a Bear and eight Islands, in its second main phase.
fn table(seats: usize) -> Scenario {
    let mut board = vec![ready(card("Wizard Class").unwrap())];
    board.push(ready(free_creature("Bear", 2, 2)));
    board.extend((0..8).map(|_| ready(island())));
    Scenario::new(seats)
        .battlefield(0, board)
        .at(1, 0, Step::Main2)
}

#[rstest]
fn a_class_is_at_level_one_and_levels_up_one_level_at_a_time(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats))?;
    assert_eq!(level(&driver, "Wizard Class"), 1);
    // Only the ability for the next level is offered.
    let hand = hand_size(&driver, 0);
    level_up(&mut driver, "Wizard Class", 3)?;
    assert_eq!(level(&driver, "Wizard Class"), 2);
    // "When this Class becomes level 2, draw two cards."
    assert_eq!(hand_size(&driver, 0), hand + 2);
    level_up(&mut driver, "Wizard Class", 5)?;
    assert_eq!(level(&driver, "Wizard Class"), 3);
    // Level 3 is the last: nothing more is offered, and the draw for the level 3 ability has not happened yet.
    assert_eq!(driver.offered_abilities("Wizard Class")?.len(), 0);
    assert_eq!(counters(&driver, "Bear"), 0);
    Ok(())
}

#[rstest]
fn level_up_is_not_offered_in_another_players_turn_or_with_the_stack_busy(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats))?;
    assert_eq!(driver.offered_abilities("Wizard Class")?.len(), 1);
    driver.idle_until(2, Step::Main2)?;
    assert_eq!(driver.offered_abilities("Wizard Class")?.len(), 0);
    Ok(())
}

#[rstest]
fn the_abilities_of_a_level_work_from_that_level_on(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats))?;
    level_up(&mut driver, "Wizard Class", 3)?;
    level_up(&mut driver, "Wizard Class", 5)?;
    // The draw of this seat's next turn puts a +1/+1 counter on its creature: "Whenever you draw a card" (level 3).
    let next = u32::try_from(seats).unwrap() + 1;
    driver.idle_until(next, Step::Main1)?;
    settle(&mut driver)?;
    assert_eq!(level(&driver, "Wizard Class"), 3);
    assert_eq!(counters(&driver, "Bear"), 1);
    Ok(())
}

#[rstest]
fn a_class_that_leaves_and_returns_starts_at_level_one(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats))?;
    level_up(&mut driver, "Wizard Class", 3)?;
    let id = on_board(&driver, "Wizard Class");
    driver.manual(ManualAction::Move {
        object: id,
        to: ManualZone::Exile,
    })?;
    driver.manual(ManualAction::Move {
        object: id,
        to: ManualZone::Battlefield,
    })?;
    assert_eq!(level(&driver, "Wizard Class"), 1);
    // The level 2 ability is the one on offer again.
    assert_eq!(driver.offered_abilities("Wizard Class")?.len(), 1);
    Ok(())
}

#[rstest]
fn a_class_enters_and_its_first_level_ability_works(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .hand(0, [card("Stormchaser's Talent").unwrap()])
        .battlefield(0, [ready(island())])
        .at(1, 0, Step::Main2);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Stormchaser's Talent")?;
    settle(&mut driver)?;
    assert_eq!(level(&driver, "Stormchaser's Talent"), 1);
    // "When this Class enters, create a 1/1 blue and red Otter creature token with prowess."
    on_board(&driver, "Otter");
    Ok(())
}

/// A normal game of `players` seats with Classes, Islands and plain creatures, fully determined by `seed`.
fn setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| {
            let mut rng = GameRng::substream(seed, 29_000 + seat);
            let mut deck: Vec<Arc<CardDefinition>> = (0..26)
                .map(|i| land(if i % 2 == 0 { Color::Blue } else { Color::Red }))
                .collect();
            for name in ["Wizard Class", "Stormchaser's Talent"] {
                deck.extend(std::iter::repeat_n(card(name).unwrap(), 3));
            }
            for slot in 0..28 {
                let power = i32::try_from(1 + rng.index(3)).unwrap_or(1);
                let name = format!("Helper {slot} {power}/{power}");
                deck.push(creature(&name, 1, &[Color::Blue], power, power));
            }
            rng.shuffle(&mut deck);
            deck.truncate(60);
            deck
        })
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}

#[test]
fn whole_games_with_classes_replay_and_the_levels_come_up() {
    let mut gained = [0u64; 4];
    let games = fuzz_games(12);
    let counts = std::sync::Mutex::new(&mut gained);
    for_each_seed(0..games, |seed| {
        for players in [2, 3, 4] {
            let outcome = play_game_with(setup(players, seed), seed, PlayOptions::default())
                .unwrap_or_else(|failure| panic!("{failure}"));
            let replayed = replay(&setup(players, seed), &outcome.events).unwrap();
            assert_eq!(replayed.state_hash(), outcome.final_hash, "seed {seed}");
            let mut counts = counts.lock().unwrap();
            for event in &outcome.events {
                if let Event::ClassLevelGained { level, .. } = event {
                    counts[usize::try_from(*level).unwrap().min(3)] += 1;
                }
            }
            counts[0] += 1;
        }
    });
    // Games played, and levels 2 and 3 gained at least once over the campaign.
    assert!(gained[0] > 0);
    assert!(gained[2] > 0, "no Class reached level 2: {gained:?}");
}
