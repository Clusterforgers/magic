//! Goal: Serum Powder at the keep-or-mulligan decision (CR 103.5) at two to four seats: its owner may exile the whole
//! hand and draw as many from the top of the library, takes no mulligan by it, is asked again about the new hand, may use
//! a second Powder drawn, and a seat without one is offered nothing; the log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::decision::{Answer, DecisionKind, MulliganChoice, MulliganOptions};
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, replay};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, ScriptError, fuzz_setup};
use rstest::rstest;

/// A stacked opening whose seat 0 library starts with Serum Powder at `places` (from the top) among Forests.
fn r115_setup(seats: usize, places: &[usize]) -> mtg_engine::setup::GameSetup {
    let mut setup = fuzz_setup(seats, 7);
    setup.stacked = true;
    let powder = card("Serum Powder").unwrap();
    let forest = mtg_testkit::cards::forest();
    let library: Vec<Arc<_>> = (0..60)
        .map(|at| {
            if places.contains(&at) {
                Arc::clone(&powder)
            } else {
                Arc::clone(&forest)
            }
        })
        .collect();
    setup.players[0].library = library;
    setup
}

/// The mulligan options asked now, with the seat.
fn r115_asked(driver: &Driver) -> Result<(PlayerId, MulliganOptions), ScriptError> {
    let decision = driver.decision()?;
    match &decision.kind {
        DecisionKind::Mulligan(options) => Ok((decision.player, options.clone())),
        other => panic!("a mulligan decision, not {}", other.name()),
    }
}

/// The ids on top of seat 0's library, top first.
fn r115_tops(driver: &Driver, count: usize) -> Vec<ObjectId> {
    driver
        .state()
        .zone(Zone::Library(PlayerId(0)))
        .iter()
        .rev()
        .take(count)
        .copied()
        .collect()
}

#[rstest]
fn r115_the_powder_exiles_the_hand_and_draws_as_many(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let setup = r115_setup(seats, &[2, 9]);
    let mut driver = Driver::new(Engine::new(setup.clone())?)?;
    let (seat, options) = r115_asked(&driver)?;
    assert_eq!(seat, PlayerId(0));
    assert_eq!(options.r115_powders.len(), 1, "the Powder in the hand");
    let hand = options.hand.clone();
    let next = r115_tops(&driver, hand.len());
    driver.answer(Answer::Mulligan(MulliganChoice::R115ExileHand(
        options.r115_powders[0],
    )))?;
    for id in &hand {
        assert_eq!(driver.state().object(*id).unwrap().zone(), Zone::Exile);
    }
    let (seat, again) = r115_asked(&driver)?;
    assert_eq!(
        seat,
        PlayerId(0),
        "the same seat decides about the new hand"
    );
    assert_eq!(again.hand, next, "the next seven of the library");
    assert_eq!(again.mulligans_taken, 0, "no mulligan is taken");
    assert_eq!(again.r115_powders.len(), 1, "the second Powder was drawn");
    driver.answer(Answer::Mulligan(MulliganChoice::Keep))?;
    let (seat, theirs) = r115_asked(&driver)?;
    assert_eq!(seat, PlayerId(1));
    assert!(theirs.r115_powders.is_empty(), "no Powder, nothing offered");
    let refused = driver.answer(Answer::Mulligan(MulliganChoice::R115ExileHand(
        again.r115_powders[0],
    )));
    assert!(refused.is_err(), "a card not in the hand is not offered");
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}
