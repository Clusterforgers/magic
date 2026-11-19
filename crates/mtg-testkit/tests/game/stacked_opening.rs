//! Goal: a stacked opening deals each library in its given order with seat 0 first, a mulligan still shuffles, and a setup without it opens as before.

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::Engine;
use mtg_engine::decision::{Answer, DecisionKind, MulliganChoice};
use mtg_engine::setup::GameSetup;
use mtg_engine::state::GameState;
use mtg_engine::zones::Zone;
use mtg_testkit::{Driver, ScriptError, fuzz_setup};
use rstest::rstest;

/// The opening hand the mulligan decision shows, with its seat.
fn asked(driver: &Driver) -> Result<(PlayerId, Vec<ObjectId>), ScriptError> {
    let decision = driver.decision()?;
    match &decision.kind {
        DecisionKind::Mulligan(options) => Ok((decision.player, options.hand.clone())),
        other => Err(ScriptError::UnexpectedDecision {
            wanted: "mulligan",
            got: other.name().to_owned(),
        }),
    }
}

/// Seven cards of each seat's library as `setup` builds it, top first, after the first `skip`.
fn tops(setup: &GameSetup, skip: usize) -> Result<Vec<Vec<ObjectId>>, ScriptError> {
    let state = GameState::from_setup(setup).map_err(mtg_engine::EngineError::from)?;
    Ok((0..setup.players.len())
        .map(|seat| {
            let library = state.zone(Zone::Library(PlayerId::from_index(seat)));
            library.iter().rev().skip(skip).take(7).copied().collect()
        })
        .collect())
}

fn stacked(players: usize, seed: u64) -> GameSetup {
    let mut setup = fuzz_setup(players, seed);
    setup.stacked = true;
    setup
}

#[rstest]
fn a_stacked_opening_deals_the_library_tops_with_seat_zero_first(
    #[values(2, 3, 4)] players: usize,
    #[values(1, 2, 3)] seed: u64,
) -> Result<(), ScriptError> {
    let setup = stacked(players, seed);
    let want = tops(&setup, 0)?;
    let mut driver = Driver::new(Engine::new(setup)?)?;
    assert_eq!(driver.state().first_player(), PlayerId(0));
    for (seat, top) in want.iter().enumerate() {
        let (player, hand) = asked(&driver)?;
        assert_eq!(
            player,
            PlayerId::from_index(seat),
            "seat 0 first, then turn order"
        );
        assert_eq!(&hand, top, "seat {seat} holds its library's top seven");
        driver.answer(Answer::Mulligan(MulliganChoice::Keep))?;
    }
    Ok(())
}

#[rstest]
fn a_mulligan_in_a_stacked_opening_still_shuffles(
    #[values(2, 3, 4)] players: usize,
    #[values(1, 2, 3)] seed: u64,
) -> Result<(), ScriptError> {
    let setup = stacked(players, seed);
    let (first, next) = (tops(&setup, 0)?, tops(&setup, 7)?);
    let mut driver = Driver::new(Engine::new(setup)?)?;
    driver.answer(Answer::Mulligan(MulliganChoice::Mulligan))?;
    for _ in 1..players {
        driver.answer(Answer::Mulligan(MulliganChoice::Keep))?;
    }
    let (player, hand) = asked(&driver)?;
    assert_eq!(player, PlayerId(0));
    assert_ne!(hand, first[0], "a mulligan does not deal the same hand");
    assert_ne!(
        hand, next[0],
        "a mulligan shuffles rather than dealing the next seven"
    );
    Ok(())
}

#[rstest]
fn a_setup_without_it_shuffles_and_rolls_as_before(
    #[values(2, 3, 4)] players: usize,
    #[values(1, 2, 3)] seed: u64,
) -> Result<(), ScriptError> {
    let setup = fuzz_setup(players, seed);
    assert!(!setup.stacked);
    let want = tops(&setup, 0)?;
    let plain = Driver::new(Engine::new(setup.clone())?)?;
    let again = Driver::new(Engine::new(setup)?)?;
    assert_eq!(plain.state().first_player(), again.state().first_player());
    let (player, hand) = asked(&plain)?;
    assert_eq!(player, plain.state().first_player());
    assert_ne!(hand, want[player.index()], "the library was shuffled");
    assert_eq!(asked(&again)?.1, hand, "the same seed deals the same hand");
    Ok(())
}
