//! `Effect::Shuffle`: players shuffle their libraries (CR 701.24).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::events::Event;
use crate::mulligan::shuffled;
use crate::resolve::{Cx, ResolveError, Run};
use crate::zones::Zone;

/// Each player the effect names who is still in the game shuffles their library, in turn order. The new order comes
/// from the game's random stream and is logged whole, so a replay needs no randomness; the k-th shuffle of a step uses
/// the k-th substream after the ones already used. An empty library is shuffled too: it still happened.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Shuffle { who } = effect else {
        return Err(wrong_effect("Shuffle"));
    };
    let state = cx.state();
    let players = cx
        .scope
        .players(who)?
        .into_iter()
        .filter(|player| state.is_alive(*player));
    Ok(Run::events(
        players
            .zip(0u64..)
            .map(|(player, offset)| Event::LibraryShuffled {
                player,
                order: shuffled(state, offset, state.zone(Zone::Library(player)).to_vec()),
            })
            .collect(),
    ))
}

#[cfg(test)]
mod tests {
    //! Goal: a shuffle puts every card of each library the effect names, and only those, into a new order from the game's random stream, the same order for the same seed and a different one for another, the k-th shuffle of a step on its own substream, an empty library still shuffled, and a player who has left the game left alone.
    use mtg_core::ids::{ObjectId, PlayerId};
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, setup, state};
    use crate::resolve::{Next, Resolving, Scope};
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);
    const FOE: PlayerId = PlayerId(1);

    /// Two seats with five cards in each library; the Source is on the battlefield.
    fn game(seed: u64) -> GameState {
        let mut game = state(
            vec![vec![], vec![]],
            vec![vec![creature("Source", 1, 1)], vec![]],
        );
        game.seed = seed;
        game
    }

    fn effect(json: &str) -> Effect {
        serde_json::from_str(json).unwrap()
    }

    fn shuffles(game: &GameState, json: &str) -> Vec<(PlayerId, Vec<ObjectId>)> {
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        let Run::Done {
            events,
            bound,
            snapshots,
            next,
        } = run(&cx, &effect(json)).unwrap()
        else {
            panic!("a shuffle asks nothing");
        };
        assert!(bound.is_empty() && snapshots.is_empty() && next == Next::Sibling);
        events
            .into_iter()
            .map(|event| match event {
                Event::LibraryShuffled { player, order } => (player, order),
                other => panic!("only shuffles were expected, got {other:?}"),
            })
            .collect()
    }

    fn sorted(mut cards: Vec<ObjectId>) -> Vec<ObjectId> {
        cards.sort_unstable();
        cards
    }

    #[rstest]
    #[case::you(r#"{"Shuffle":{}}"#, &[0])]
    #[case::an_opponent(r#"{"Shuffle":{"who":"Opponents"}}"#, &[1])]
    #[case::each_player_in_turn_order(r#"{"Shuffle":{"who":"EachPlayer"}}"#, &[0, 1])]
    fn each_library_named_is_shuffled_whole(#[case] json: &str, #[case] seats: &[usize]) {
        let game = game(7);
        let done = shuffles(&game, json);
        assert_eq!(
            done.iter().map(|(p, _)| p.index()).collect::<Vec<_>>(),
            seats
        );
        for (player, order) in done {
            let library = game.zone(Zone::Library(player)).to_vec();
            assert_eq!(sorted(order), sorted(library));
        }
    }

    #[test]
    fn the_same_seed_gives_the_same_order_and_another_seed_another() {
        let json = r#"{"Shuffle":{}}"#;
        let first = shuffles(&game(7), json);
        assert_eq!(first, shuffles(&game(7), json));
        assert_ne!(first, shuffles(&game(8), json));
        let library = game(7).zone(Zone::Library(ME)).to_vec();
        assert_ne!(first[0].1, library, "the order changed");
    }

    #[test]
    fn the_second_shuffle_of_a_step_uses_the_next_substream() {
        let game = game(11);
        let both = shuffles(&game, r#"{"Shuffle":{"who":"EachPlayer"}}"#);
        let mut after_one = game.clone();
        after_one.rng_index += 1;
        let alone = shuffles(&after_one, r#"{"Shuffle":{"who":"Opponents"}}"#);
        assert_eq!(both[1], alone[0], "what the first used the second does not");
    }

    #[test]
    fn an_empty_library_is_shuffled_and_a_player_who_left_is_not() {
        let mut empty = setup(
            vec![vec![], vec![]],
            vec![vec![creature("Source", 1, 1)], vec![]],
        );
        empty.players[0].library.clear();
        let mut game = GameState::from_setup(&empty).unwrap();
        assert_eq!(shuffles(&game, r#"{"Shuffle":{}}"#), [(ME, Vec::new())]);
        crate::fold::apply_event(
            &mut game,
            &Event::PlayerLost {
                player: FOE,
                reason: crate::player::LossReason::Conceded,
            },
        )
        .unwrap();
        assert_eq!(shuffles(&game, r#"{"Shuffle":{"who":"Opponents"}}"#), []);
    }

    #[test]
    fn another_effect_is_a_bug_in_the_dispatch() {
        let game = game(1);
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(&game, ME, source.reference()),
            resolving: &resolving,
        };
        assert!(matches!(
            run(&cx, &effect(r#"{"Draw":{}}"#)),
            Err(ResolveError::Invalid(_))
        ));
    }
}
