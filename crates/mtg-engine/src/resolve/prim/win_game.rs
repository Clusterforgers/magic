//! `Effect::WinGame`: a player wins the game (CR 104.2b).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::events::Event;
use crate::player::LossReason;
use crate::resolve::{Cx, ResolveError, Run};

/// Why the opponents of a winner lose: the rules read a win by an effect as the last opponent leaving (CR 104.2a).
const OPPONENT_WON: &str = "an opponent won the game";

/// Every opponent of each player the effect names, who is in the game, loses at once, in turn order from the
/// active player; the engine's check after the resolution then ends the game when one player is left (and as a draw
/// when none is). A winner who has left the game, or one nobody names, makes no one lose. Two winners are opponents
/// of each other, so naming two players leaves neither standing.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::WinGame { who } = effect else {
        return Err(wrong_effect("WinGame"));
    };
    let state = cx.state();
    let winners: Vec<_> = cx
        .scope
        .players(who)?
        .into_iter()
        .filter(|player| state.is_alive(*player))
        .collect();
    let events = state
        .turn_order_from(state.turn().active())
        .into_iter()
        .filter(|player| winners.iter().any(|winner| winner != player))
        .map(|player| Event::PlayerLost {
            player,
            reason: LossReason::Format(OPPONENT_WON.into()),
        })
        .collect();
    Ok(Run::events(events))
}

#[cfg(test)]
mod tests {
    //! Goal: a win makes every opponent of the winner in the game lose, in turn order from the active player, at tables of two, three and four seats (a seat that has left is not made to lose again, a winner who has left wins nothing, an effect that names an opponent as the winner makes the controller lose, two winners leave no one standing), and it logs nothing else, binds nothing and goes on to the next effect.
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, setup};
    use crate::fold::apply_event;
    use crate::resolve::{Next, Resolving, Scope};
    use crate::setup::Start;
    use crate::state::GameState;
    use crate::turn::Step;

    fn seat(index: usize) -> PlayerId {
        PlayerId::from_index(index)
    }

    /// `seats` seats; seat 0 controls the Source and it is `active`'s turn; the seats in `out` have left the game.
    fn game(seats: usize, active: usize, out: &[usize]) -> GameState {
        let mut battlefield = vec![vec![creature("Source", 1, 1)]];
        battlefield.extend((1..seats).map(|_| Vec::new()));
        let mut setup = setup(vec![Vec::new(); seats], battlefield);
        setup.start = Start::At {
            turn: 1,
            active: seat(active),
            step: Step::Main1,
        };
        let mut game = GameState::from_setup(&setup).unwrap();
        for &index in out {
            apply_event(
                &mut game,
                &Event::PlayerLost {
                    player: seat(index),
                    reason: LossReason::Conceded,
                },
            )
            .unwrap();
        }
        game
    }

    fn lost(run: Run) -> Vec<usize> {
        let Run::Done {
            events,
            bound,
            snapshots,
            next,
        } = run
        else {
            panic!("a win needs no answer");
        };
        assert!(bound.is_empty() && snapshots.is_empty());
        assert_eq!(next, Next::Sibling);
        events
            .iter()
            .map(|event| match event {
                Event::PlayerLost { player, reason } => {
                    assert_eq!(*reason, LossReason::Format(OPPONENT_WON.into()));
                    player.index()
                }
                other => panic!("unexpected {other:?}"),
            })
            .collect()
    }

    fn wins(game: &GameState, json: &str) -> Result<Vec<usize>, ResolveError> {
        let effect: Effect = serde_json::from_str(json).unwrap();
        let source = game.objects().next().unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(game, seat(0), source.reference()),
            resolving: &resolving,
        };
        run(&cx, &effect).map(lost)
    }

    #[rstest]
    #[case::two_seats(2, 0, &[], r#"{"WinGame":{}}"#, &[1])]
    #[case::three_seats(3, 0, &[], r#"{"WinGame":{}}"#, &[1, 2])]
    #[case::four_seats(4, 0, &[], r#"{"WinGame":{}}"#, &[1, 2, 3])]
    #[case::in_turn_order_from_the_active_player(4, 2, &[], r#"{"WinGame":{}}"#, &[2, 3, 1])]
    #[case::the_active_player_is_the_only_opponent(2, 1, &[], r#"{"WinGame":{}}"#, &[1])]
    #[case::a_seat_that_left_is_not_made_to_lose_again(4, 0, &[2], r#"{"WinGame":{}}"#, &[1, 3])]
    #[case::the_last_opponent_only(3, 0, &[1], r#"{"WinGame":{}}"#, &[2])]
    #[case::nobody_is_left_to_lose(2, 0, &[1], r#"{"WinGame":{}}"#, &[])]
    #[case::you_named_out_loud(3, 0, &[], r#"{"WinGame":{"who":"You"}}"#, &[1, 2])]
    #[case::an_opponent_wins_so_the_controller_loses_with_the_rest(
        2, 0, &[], r#"{"WinGame":{"who":"Opponents"}}"#, &[0]
    )]
    #[case::two_winners_leave_no_one(
        3, 0, &[], r#"{"WinGame":{"who":"Opponents"}}"#, &[0, 1, 2]
    )]
    #[case::everyone_wins_so_everyone_loses(
        3, 0, &[], r#"{"WinGame":{"who":"EachPlayer"}}"#, &[0, 1, 2]
    )]
    #[case::nobody_is_named(
        3, 0, &[1], r#"{"WinGame":{"who":{"Each":[{"Named":"Nobody"}]}}}"#, &[]
    )]
    fn every_opponent_of_the_winner_loses(
        #[case] seats: usize,
        #[case] active: usize,
        #[case] out: &[usize],
        #[case] json: &str,
        #[case] want: &[usize],
    ) {
        assert_eq!(wins(&game(seats, active, out), json).unwrap(), want);
    }

    #[test]
    fn a_player_who_left_cannot_win() {
        // Seat 1 left; naming it as the winner (`Opponents` is seat 1 alone at two seats) makes no one lose.
        assert_eq!(
            wins(&game(2, 0, &[1]), r#"{"WinGame":{"who":"Opponents"}}"#).unwrap(),
            Vec::<usize>::new()
        );
    }

    #[test]
    fn another_effect_is_refused_by_name() {
        assert_eq!(
            wins(&game(2, 0, &[]), r#"{"Draw":{}}"#).unwrap_err(),
            ResolveError::Invalid("the WinGame primitive was given another effect".into())
        );
    }
}
