//! Goal: the turn view's round counts up only after every seat (every team where teams share their turns) has had a turn, from the first player's, an extra turn leaves it where it was, and it is in the JSON of every seat's view.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::house::HouseRules;
use mtg_core::ids::PlayerId;
use mtg_core::teams::{TeamMode, Teams};
use mtg_engine::{Event, GameState, apply_event};
use mtg_protocol::TableView;
use mtg_testkit::{Driver, Scenario};
use rstest::rstest;

fn seat(index: usize) -> PlayerId {
    PlayerId::from_index(index)
}

fn round(state: &GameState, viewer: usize) -> u32 {
    TableView::of(state, seat(viewer)).turn.round
}

/// Begins the next turn of the table in turn order, or an extra turn of `extra`, and returns who has it.
fn begin(state: &mut GameState, regular: &mut PlayerId, extra: Option<PlayerId>) -> PlayerId {
    let number = state.turn().number() + 1;
    let active = match extra {
        Some(player) => {
            let added = Event::ExtraTurnAdded {
                player,
                skip_untap: false,
                no_power_up: false,
                r6703_end: None,
            };
            apply_event(state, &added).unwrap();
            player
        }
        None => state.next_turn_seat(*regular).unwrap(),
    };
    let began = Event::TurnBegan {
        number,
        active,
        extra: extra.is_some(),
    };
    apply_event(state, &began).unwrap();
    if extra.is_none() {
        *regular = active;
    }
    active
}

#[rstest]
fn the_round_opens_with_the_first_turn_and_every_seat_sees_it(#[values(2, 3, 4)] seats: usize) {
    let driver = Driver::from_scenario(Scenario::new(seats)).unwrap();
    let state = driver.state();
    for viewer in 0..seats {
        assert_eq!(round(state, viewer), 1);
        let json = serde_json::to_value(TableView::of(state, seat(viewer))).unwrap();
        assert_eq!(json["turn"]["round"], 1, "the round is always in the JSON");
        let back: TableView = serde_json::from_value(json).unwrap();
        assert_eq!(back, TableView::of(state, seat(viewer)));
    }
}

#[rstest]
fn the_round_counts_up_after_every_seat_has_had_a_turn(#[values(2, 3, 4, 6)] seats: usize) {
    let driver = Driver::from_scenario(Scenario::new(seats)).unwrap();
    let mut state = driver.state().clone();
    let mut regular = state.turn().active();
    for turn in 1..=seats * 3 {
        begin(&mut state, &mut regular, None);
        let expected = u32::try_from(turn / seats).unwrap() + 1;
        assert_eq!(round(&state, 0), expected, "turn {}", state.turn().number());
    }
    assert_eq!(round(&state, 0), 4);
}

#[rstest]
fn an_extra_turn_does_not_advance_the_round(#[values(2, 3)] seats: usize) {
    let driver = Driver::from_scenario(Scenario::new(seats)).unwrap();
    let mut state = driver.state().clone();
    let mut regular = state.turn().active();
    for _ in 1..seats {
        begin(&mut state, &mut regular, None);
    }
    assert_eq!(round(&state, 0), 1, "the last seat of the first round");
    // An extra turn of the first seat, even one that comes after the last seat of the round.
    begin(&mut state, &mut regular, Some(seat(0)));
    assert!(TableView::of(&state, seat(0)).turn.extra);
    assert_eq!(round(&state, 0), 1);
    // The regular turns go on from the seat they were at, and the next one opens round 2.
    begin(&mut state, &mut regular, None);
    assert_eq!(round(&state, 0), 2);
}

#[rstest]
#[case::two_headed_giant_side_by_side(TeamMode::TwoHeadedGiant, &[0, 0, 1, 1])]
#[case::two_headed_giant_apart(TeamMode::TwoHeadedGiant, &[0, 1, 0, 1])]
fn a_round_of_shared_turns_is_each_team_once(#[case] mode: TeamMode, #[case] assignment: &[u32]) {
    let scenario = Scenario::new(4).house_rules(HouseRules {
        teams: Some(Teams {
            mode,
            seats: assignment.to_vec(),
        }),
        ..HouseRules::default()
    });
    let driver = Driver::from_scenario(scenario).unwrap();
    let mut state = driver.state().clone();
    let mut regular = state.turn().active();
    assert_eq!(round(&state, 0), 1);
    // Turn 1 was the first team's; the other team has the second turn of round 1, then the first team opens round 2.
    for expected in [1, 2, 2, 3, 3, 4] {
        begin(&mut state, &mut regular, None);
        assert_eq!(round(&state, 0), expected, "turn {}", state.turn().number());
    }
}
