//! Goal: each player's count of turns begun (`Amount::TurnsTaken`) follows the turn order from the set-up turn, counts
//! extra turns, and makes "your first, second, or third turn of the game" true exactly in those turns; each player's
//! life total is kept as each turn begins (`Amount::LifeAsTurnBegan`); at 2 to 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::{Amount, Condition};
use mtg_core::ids::PlayerId;
use rstest::rstest;

use crate::events::Event;
use crate::fixtures::setup;
use crate::fold::apply_event;
use crate::resolve::Scope;
use crate::state::GameState;

/// "It's your first, second, or third turn of the game", read for `player`.
const EARLY: &str =
    r#"{"All":["YourTurn",{"Compare":{"left":{"TurnsTaken":"You"},"op":"Le","right":3}}]}"#;

fn game(seats: usize) -> GameState {
    GameState::from_setup(&setup(vec![vec![]; seats], vec![vec![]; seats])).unwrap()
}

fn scope(game: &GameState, player: PlayerId) -> Scope<'_> {
    let source = crate::object::ObjectRef {
        id: mtg_core::ids::ObjectId(0),
        counter: 0,
    };
    Scope::announcing(game, player, source)
}

fn taken(game: &GameState, player: PlayerId) -> i64 {
    scope(game, player)
        .amount(&Amount::TurnsTaken(mtg_cards::refs::Ref::You))
        .unwrap()
}

fn early(game: &GameState, player: PlayerId) -> bool {
    let condition: Condition = serde_json::from_str(EARLY).unwrap();
    scope(game, player).holds(&condition).unwrap()
}

fn begin(game: &mut GameState, number: u32, active: PlayerId, extra: bool) {
    apply_event(
        game,
        &Event::TurnBegan {
            number,
            active,
            extra,
        },
    )
    .unwrap();
}

#[rstest]
fn each_seat_counts_its_own_turns_in_order(#[values(2, 3, 4)] seats: usize) {
    let mut game = game(seats);
    assert_eq!(taken(&game, PlayerId(0)), 1);
    for seat in 1..seats {
        assert_eq!(taken(&game, PlayerId::from_index(seat)), 0);
    }
    for number in 2..=u32::try_from(seats * 3 + 1).unwrap() {
        let active = PlayerId::from_index((usize::try_from(number).unwrap() - 1) % seats);
        begin(&mut game, number, active, false);
        let rounds = i64::from(number - 1) / i64::try_from(seats).unwrap();
        let expected = rounds + 1;
        assert_eq!(taken(&game, active), expected, "turn {number}");
        assert_eq!(early(&game, active), expected <= 3, "turn {number}");
        // Another seat is never in its early turns during this one.
        let other = PlayerId::from_index((active.index() + 1) % seats);
        assert!(!early(&game, other), "turn {number}");
    }
}

#[rstest]
#[case::two_seats(2)]
#[case::four_seats(4)]
fn an_extra_turn_is_one_of_the_players_turns(#[case] seats: usize) {
    let mut game = game(seats);
    apply_event(
        &mut game,
        &Event::ExtraTurnAdded {
            player: PlayerId(0),
            skip_untap: false,
        },
    )
    .unwrap();
    begin(&mut game, 2, PlayerId(0), true);
    assert_eq!(taken(&game, PlayerId(0)), 2);
    assert_eq!(taken(&game, PlayerId(1)), 0);
    begin(&mut game, 3, PlayerId(1), false);
    begin(&mut game, 4, PlayerId(0), false);
    // The fourth turn of the first seat is its third regular one, but no longer an early turn.
    assert_eq!(taken(&game, PlayerId(0)), 3);
    assert!(early(&game, PlayerId(0)));
}

#[rstest]
#[case::start_of_the_game(1, 0, &[1, 0, 0])]
#[case::a_later_turn(5, 1, &[2, 2, 1])]
fn a_game_set_up_at_a_later_turn_counts_the_turns_before_it(
    #[case] turn: u32,
    #[case] active: usize,
    #[case] expected: &[u32],
) {
    let turns = crate::rmt8_turns::turns_before(
        turn,
        PlayerId::from_index((active + 3 - usize::try_from(turn - 1).unwrap() % 3) % 3),
        3,
    );
    for (seat, want) in expected.iter().enumerate() {
        assert_eq!(
            turns.get(&PlayerId::from_index(seat)).copied().unwrap_or(0),
            *want,
            "seat {seat}"
        );
    }
}

#[rstest]
fn the_life_totals_are_kept_as_each_turn_begins(#[values(2, 3, 4)] seats: usize) {
    let mut game = game(seats);
    let as_begun = |game: &GameState, seat: usize| {
        scope(game, PlayerId::from_index(seat))
            .amount(&Amount::LifeAsTurnBegan(mtg_cards::refs::Ref::You))
            .unwrap()
    };
    let start = i64::from(game.player(PlayerId(1)).unwrap().life());
    apply_event(
        &mut game,
        &Event::LifeLost {
            player: PlayerId(1),
            amount: 5,
        },
    )
    .unwrap();
    // The turn's start is what counts, not the life now.
    assert_eq!(as_begun(&game, 1), start);
    begin(&mut game, 2, PlayerId(1), false);
    assert_eq!(as_begun(&game, 1), start - 5);
    assert_eq!(as_begun(&game, 0), start);
}
