//! Goal: a mode picked at random (Umaro, Raging Yeti) comes from the game's seeded random source at tables of two, three and four seats: the same seed gives the same mode, every mode comes for some seed, no player is asked, the pick is part of the public event that puts the ability on the stack and spends one substream, and the log replays to the state it was played to; a mode that has not been chosen this turn (Lita, Little Orphan Amphibian) is offered only until the ability has chosen it, a mode chosen twice in a turn is refused by the options, an ability with every mode used is removed, and the tally starts again with the next turn.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use mtg_engine::choose::ModeChoice;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::decision::DecisionKind;
use mtg_engine::{Answer, Event, replay};
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

use crate::common::{names, pid};

/// Seat 0 has Umaro on the battlefield in the first main phase of turn 1, three cards in hand and a library to draw.
fn umaro(seats: usize, seed: u64) -> Scenario {
    Scenario::new(seats)
        .seed(seed)
        .at(1, 0, Step::Main1)
        .battlefield_named(0, ["Umaro, Raging Yeti"])
        .hand_named(0, ["Lightning Bolt", "Giant Growth", "Counterspell"])
        .library_named(0, ["Grizzly Bears"; 12])
}

/// Plays to the end step of the turn and returns the driver.
fn through_combat(seats: usize, seed: u64) -> Result<Driver, ScriptError> {
    let mut driver = Driver::from_scenario(umaro(seats, seed))?;
    driver.idle_until(1, Step::End)?;
    Ok(driver)
}

/// The modes the abilities were put on the stack with, and whether they were drawn at random.
fn stacked_modes(driver: &Driver) -> Vec<(Vec<usize>, bool)> {
    driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::AbilityPutOnStack(put) => {
                Some((put.choices.modes.clone(), put.choices.random_modes))
            }
            _ => None,
        })
        .collect()
}

#[rstest]
fn the_same_seed_gives_the_same_mode_at_every_table_size(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut seen = BTreeSet::new();
    for seed in 0..40 {
        let driver = through_combat(seats, seed)?;
        let picked = stacked_modes(&driver);
        assert_eq!(picked.len(), 1, "one ability, seed {seed}");
        let (modes, random) = &picked[0];
        assert!(random, "marked as drawn at random, seed {seed}");
        assert_eq!(modes.len(), 1, "one mode, seed {seed}");
        assert_eq!(picked, stacked_modes(&through_combat(seats, seed)?), "twice");
        assert_eq!(picked, stacked_modes(&through_combat(2, seed)?), "table size");
        seen.insert(modes[0]);
    }
    assert_eq!(seen, BTreeSet::from([0, 1, 2]), "every mode comes for some seed");
    Ok(())
}

#[rstest]
fn the_pick_spends_one_substream_and_replays(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    for seed in [0, 1, 2, 3, 4, 5] {
        let start = Driver::from_scenario(umaro(seats, seed))?;
        let before = start.state().rng_index();
        let driver = through_combat(seats, seed)?;
        assert_eq!(driver.state().rng_index(), before + 1, "seed {seed}");
        let replayed = replay(&umaro(seats, seed).setup().unwrap(), driver.engine().events())
            .unwrap();
        assert_eq!(replayed.state_hash(), driver.state().state_hash(), "seed {seed}");
    }
    Ok(())
}

#[rstest]
fn no_player_is_asked_for_the_mode(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(umaro(seats, 3))?;
    for _ in 0..200 {
        if driver.state().turn().step() == Step::End {
            break;
        }
        let decision = driver.decision()?.clone();
        assert!(
            !matches!(decision.kind, DecisionKind::ChooseMode(_)),
            "a mode decision at {:?}",
            driver.state().turn().step()
        );
        driver.answer_passively()?;
    }
    Ok(())
}

#[rstest]
#[case::combine(0)]
#[case::discard_and_draw(1)]
#[case::damage(2)]
fn the_mode_drawn_is_the_one_that_happens(
    #[values(2, 3, 4)] seats: usize,
    #[case] mode: usize,
) -> Result<(), ScriptError> {
    let seed = (0..200)
        .find(|seed| {
            through_combat(seats, *seed)
                .map(|driver| stacked_modes(&driver)[0].0 == [mode])
                .unwrap_or(false)
        })
        .expect("a seed draws the mode");
    let driver = through_combat(seats, seed)?;
    let hand = names(driver.state(), Zone::Hand(pid(0)));
    if mode == 1 {
        assert_eq!(hand.len(), 4, "the hand was discarded and four cards drawn: {hand:?}");
        assert_eq!(names(driver.state(), Zone::Graveyard(pid(0))).len(), 3);
    } else {
        assert!(hand.contains(&"Lightning Bolt".to_owned()), "the hand stays: {hand:?}");
    }
    if mode == 2 {
        let lives: Vec<i32> = (0..seats).map(|seat| driver.state().player(pid(seat)).unwrap().life()).collect();
        assert!(lives.iter().any(|life| *life == 15), "five damage to one player: {lives:?}");
    }
    Ok(())
}

// Lita, Little Orphan Amphibian

/// Lita and eight Forests.
fn lita_and_forests() -> Vec<&'static str> {
    let mut cards = vec!["Lita, Little Orphan Amphibian"];
    cards.extend(["Forest"; 8]);
    cards
}

/// Seat 0 has Lita, eight Forests and four Grizzly Bears in hand, in the first main phase of turn 1.
fn lita(seats: usize) -> Scenario {
    Scenario::new(seats)
        .seed(7)
        .at(1, 0, Step::Main1)
        .battlefield_named(0, lita_and_forests())
        .hand_named(0, ["Grizzly Bears"; 4])
        .library_named(0, ["Grizzly Bears"; 12])
}

/// The modes offered by a pending mode decision.
fn offered(driver: &Driver) -> Option<Vec<usize>> {
    match &driver.decision().ok()?.kind {
        DecisionKind::ChooseMode(options) => Some(options.modes.iter().map(|m| m.index).collect()),
        _ => None,
    }
}

/// Casts a Grizzly Bears and plays until Lita's ability asks for its mode or the stack is empty again; returns what
/// was offered and answers with `mode` when one was.
fn bear(driver: &mut Driver, mode: usize) -> Result<Option<Vec<usize>>, ScriptError> {
    driver.cast("Grizzly Bears")?;
    let mut asked = None;
    for _ in 0..100 {
        if let Some(modes) = offered(driver) {
            asked = Some(modes);
            driver.answer(Answer::ChooseMode(ModeChoice::Modes(vec![mode])))?;
        } else if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(asked);
        } else {
            driver.answer_passively()?;
        }
    }
    Err(ScriptError::Stuck("Lita".into()))
}

fn lita_abilities(driver: &Driver) -> usize {
    driver
        .engine()
        .events()
        .iter()
        .filter(|event| matches!(event, Event::AbilityPutOnStack(put) if !put.choices.modes.is_empty()))
        .count()
}

#[rstest]
fn a_mode_chosen_once_is_not_offered_again_in_the_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(lita(seats))?;
    assert_eq!(bear(&mut driver, 1)?, Some(vec![0, 1, 2]), "all three modes at first");
    assert_eq!(bear(&mut driver, 0)?, Some(vec![0, 2]), "the Food was chosen");
    assert_eq!(bear(&mut driver, 2)?, Some(vec![2]), "only the scry is left");
    assert!(names(driver.state(), Zone::Battlefield).iter().any(|name| name == "Food"));
    assert_eq!(lita_abilities(&driver), 3);
    // Every mode was chosen: the fourth ability is removed, nothing is asked and nothing happens.
    assert_eq!(bear(&mut driver, 0)?, None, "no mode is left to choose");
    assert_eq!(lita_abilities(&driver), 3, "no ability was put on the stack");
    Ok(())
}

#[rstest]
fn the_modes_are_offered_again_in_the_next_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(lita(seats))?;
    for mode in [0, 1, 2] {
        bear(&mut driver, mode)?;
    }
    let next = u32::try_from(seats).unwrap() + 1;
    driver.idle_until(next, Step::Main1)?;
    assert_eq!(bear(&mut driver, 0)?, Some(vec![0, 1, 2]), "the tally began again");
    Ok(())
}

