//! Goal: the turn view of every seat and of a spectator says when a turn goes on without its active player (CR 800.4): the flag is set from the concession of the active player in every step to the end of that turn, `active` keeps naming the seat that left, another seat leaving never sets it, a game of two ends without it, the next turn (whichever seats left before) clears it, a batch and a game resumed from its log carry it, and the field is left out of the JSON while it is false.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::state::Stage;
use mtg_engine::turn::Step;
use mtg_engine::{Engine, Event, GameResult};
use mtg_protocol::{StateView, TableView, project_events};
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn seat(index: usize) -> PlayerId {
    PlayerId::from_index(index)
}

/// Every viewer of a table of `seats`: each seat and one spectator.
fn viewers(seats: usize) -> Vec<PlayerId> {
    (0..=seats).map(seat).collect()
}

/// The table as each viewer sees it.
fn tables(driver: &Driver, seats: usize) -> Vec<TableView> {
    viewers(seats)
        .into_iter()
        .map(|viewer| TableView::of(driver.state(), viewer))
        .collect()
}

/// Seat `owner` holds a Bear and a land and moves in `step` of turn 2.
fn table(seats: usize, owner: usize, step: Step) -> Scenario {
    Scenario::new(seats)
        .at(2, owner, step)
        .battlefield(owner, [ready(free_creature("Bear", 2, 2)), ready(forest())])
        .hand(owner, [forest()])
}

/// Every viewer's turn view of `driver`'s state: the turn, the seat it belongs to and whether it lacks its player.
fn assert_turn(driver: &Driver, seats: usize, turn: u32, owner: usize, left: bool, context: &str) {
    for (viewer, view) in tables(driver, seats).iter().enumerate() {
        let at = format!("{context}, viewer {viewer}");
        assert_eq!(view.turn.number, turn, "{at}");
        assert_eq!(view.turn.active, seat(owner), "{at}");
        assert_eq!(view.turn.active_left, left, "{at}");
        let json = serde_json::to_value(view).unwrap();
        assert_eq!(
            json["turn"].get("active_left").is_some(),
            left,
            "{at}: the field is there only while set"
        );
        let back: TableView = serde_json::from_value(json).unwrap();
        assert_eq!(&back, view, "{at}: round trip");
    }
}

#[rstest]
fn the_active_player_conceding_sets_the_flag_for_the_rest_of_the_turn(
    #[values(3, 4)] seats: usize,
    #[values(0, 1, 2, 3)] owner: usize,
    #[values(
        Step::Upkeep,
        Step::Draw,
        Step::Main1,
        Step::BeginCombat,
        Step::EndCombat,
        Step::Main2,
        Step::End
    )]
    step: Step,
) -> Result<(), ScriptError> {
    if owner >= seats {
        return Ok(());
    }
    let mut driver = Driver::from_scenario(table(seats, owner, step))?;
    assert_turn(&driver, seats, 2, owner, false, "before");
    driver.concede(seat(owner))?;
    assert_eq!(driver.state().turn().step(), step);
    let mut answers = 0;
    while driver.state().turn().number() == 2 {
        assert_turn(&driver, seats, 2, owner, true, &format!("answer {answers}"));
        driver.answer_passively()?;
        answers += 1;
    }
    let next = (owner + 1) % seats;
    assert_turn(&driver, seats, 3, next, false, "the next turn");
    Ok(())
}

#[rstest]
fn another_seat_conceding_never_sets_the_flag(
    #[values(3, 4)] seats: usize,
    #[values(1, 2, 3)] leaver: usize,
    #[values(Step::Main1, Step::EndCombat, Step::End)] step: Step,
) -> Result<(), ScriptError> {
    if leaver >= seats {
        return Ok(());
    }
    let mut driver = Driver::from_scenario(table(seats, 0, step))?;
    driver.concede(seat(leaver))?;
    while driver.state().turn().number() == 2 {
        assert_turn(&driver, seats, 2, 0, false, "turn 2");
        driver.answer_passively()?;
    }
    let next = (1..seats).find(|s| *s != leaver).unwrap_or(0);
    assert_turn(&driver, seats, 3, next, false, "turn 3");
    Ok(())
}

#[rstest]
fn a_game_of_two_ends_without_a_turn_that_lacks_its_player(
    #[values(0, 1)] owner: usize,
    #[values(Step::Upkeep, Step::Main1, Step::EndCombat, Step::End)] step: Step,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(2, owner, step))?;
    driver.concede(seat(owner))?;
    assert_eq!(
        driver.engine().result(),
        Some(GameResult::Winner(seat(1 - owner)))
    );
    assert_eq!(driver.state().stage(), Stage::Finished);
    assert_turn(&driver, 2, 2, owner, false, "after the game");
    for view in tables(&driver, 2) {
        assert!(view.result.is_some());
    }
    Ok(())
}

/// The next turn goes to the next living seat in turn order, whichever seats left before, and has its player.
#[rstest]
#[case::three_seats_first(3, &[], 0, 1)]
#[case::three_seats_last_wraps(3, &[], 2, 0)]
#[case::four_seats_middle(4, &[], 1, 2)]
#[case::four_seats_last_wraps(4, &[], 3, 0)]
#[case::skips_a_seat_that_left_before(4, &[1], 0, 2)]
#[case::wraps_over_a_seat_that_left_before(4, &[0], 3, 1)]
#[case::skips_two_seats(5, &[1, 2], 0, 3)]
#[case::wraps_over_two_seats(5, &[4, 0], 3, 1)]
fn the_next_turn_has_an_active_player_again(
    #[case] seats: usize,
    #[case] gone: &[usize],
    #[case] owner: usize,
    #[case] next: usize,
) -> Result<(), ScriptError> {
    let scenario = gone
        .iter()
        .fold(table(seats, owner, Step::Main1), |s, out| s.life(*out, 0));
    let mut driver = Driver::from_scenario(scenario)?;
    assert_turn(&driver, seats, 2, owner, false, "before");
    driver.concede(seat(owner))?;
    while driver.state().turn().number() == 2 {
        assert_turn(&driver, seats, 2, owner, true, "turn 2");
        driver.answer_passively()?;
    }
    assert_turn(&driver, seats, 3, next, false, "turn 3");
    // The turn after it is like any other: the flag does not come back by itself.
    while driver.state().turn().number() == 3 {
        driver.answer_passively()?;
    }
    assert_eq!(driver.state().turn().number(), 4);
    for view in tables(&driver, seats) {
        assert!(!view.turn.active_left);
    }
    Ok(())
}

#[rstest]
fn the_batches_of_a_concession_and_of_the_end_of_the_turn_carry_the_flag(
    #[values(3, 4)] seats: usize,
    #[values(0, 2)] owner: usize,
    #[values(Step::Main1, Step::Cleanup)] step: Step,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, owner, step))?;
    let all = viewers(seats);
    let before = driver.state().clone();
    let events = driver.concede(seat(owner))?;
    // Nobody is asked anything in cleanup, so the concession there runs the turn out in the same batch.
    let runs_on = driver.state().turn().number() == 2;
    assert_eq!(runs_on, step != Step::Cleanup);
    let batches = project_events(&before, &events, &all, 0).unwrap();
    for (batch, viewer) in batches.iter().zip(&all) {
        assert_eq!(batch.table.turn.active_left, runs_on, "{viewer:?}");
        assert_eq!(*batch.table, TableView::of(driver.state(), *viewer));
    }
    if !runs_on {
        let next = (owner + 1) % seats;
        assert_turn(&driver, seats, 3, next, false, "the turn after cleanup");
        return Ok(());
    }
    let during = driver.state().clone();
    let mut rest = Vec::new();
    while driver.state().turn().number() == 2 {
        rest.extend(driver.answer_passively()?);
    }
    assert!(rest.iter().any(|e| matches!(e, Event::TurnBegan { .. })));
    let batches = project_events(&during, &rest, &all, events.len() as u64).unwrap();
    for (batch, viewer) in batches.iter().zip(&all) {
        assert!(!batch.table.turn.active_left, "{viewer:?}");
        assert_eq!(batch.table.turn.active, seat((owner + 1) % seats));
        assert_eq!(*batch.table, TableView::of(driver.state(), *viewer));
    }
    Ok(())
}

#[rstest]
fn a_game_resumed_from_its_log_shows_the_same_turn(
    #[values(3, 4)] seats: usize,
    #[values(0, 1)] owner: usize,
    #[values(Step::Upkeep, Step::Main1, Step::End)] step: Step,
    #[values(0, 3)] answers_after: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats, owner, step);
    let setup = scenario.clone().setup()?;
    let mut driver = Driver::from_scenario(scenario)?;
    driver.concede(seat(owner))?;
    for _ in 0..answers_after {
        if driver.state().turn().number() == 2 {
            driver.answer_passively()?;
        }
    }
    let resumed = Engine::resume(setup, driver.engine().events().to_vec())?;
    let still_in_turn = driver.state().turn().number() == 2;
    for viewer in viewers(seats) {
        let live = StateView::for_player(driver.state(), viewer);
        let rebuilt = StateView::for_player(resumed.state(), viewer);
        assert_eq!(rebuilt, live, "viewer {viewer:?}");
        assert_eq!(rebuilt.table.turn.active_left, still_in_turn);
    }
    Ok(())
}

#[test]
fn the_flag_is_left_out_until_the_turns_begin() -> Result<(), ScriptError> {
    let engine = Engine::new(mtg_testkit::fuzz_setup(3, 5))?;
    assert_eq!(engine.state().stage(), Stage::Mulligan);
    let mut engine = engine;
    let first = engine.state().first_player();
    engine.concede(first)?;
    for viewer in viewers(3) {
        let view = TableView::of(engine.state(), viewer);
        assert!(!view.turn.active_left, "{viewer:?} in the mulligan stage");
    }
    Ok(())
}
