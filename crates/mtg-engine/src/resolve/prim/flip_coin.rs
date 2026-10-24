//! `Effect::FlipCoin`: a player flips a coin (CR 705) and the resolution goes on with the list for the result.

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::events::Event;
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::rng::GameRng;

/// The first player the effect names who is still in the game flips: even odds from the next substream of the game's
/// random stream, logged as `CoinFlipped`, and the step goes on into `win` or `lose`. Nobody in the game to flip is no flip.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::FlipCoin { who, .. } = effect else {
        return Err(wrong_effect("FlipCoin"));
    };
    let state = cx.state();
    let Some(player) = cx
        .scope
        .players(who)?
        .into_iter()
        .find(|player| state.is_alive(*player))
    else {
        return Ok(Run::events(Vec::new()));
    };
    let won = GameRng::substream(state.seed(), state.rng_index()).below(2) == 0;
    Ok(Run::Done {
        events: vec![Event::CoinFlipped { player, won }],
        bound: Vec::new(),
        snapshots: Vec::new(),
        next: Next::Enter(usize::from(!won)),
    })
}

#[cfg(test)]
mod tests {
    //! Goal: a flip is won or lost from the game's random stream (the same seed gives the same flip, the seeds give both results), logged for the player who flipped and entering the list for the result; a player who has left the game flips nothing, and another effect is refused by name.
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, state};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;
    use crate::zones::Zone;

    const ME: PlayerId = PlayerId(0);

    fn game(seed: u64) -> GameState {
        let mut game = state(
            vec![vec![], vec![]],
            vec![vec![creature("Source", 1, 1)], vec![]],
        );
        game.seed = seed;
        game
    }

    fn flip(game: &GameState, json: &str) -> Result<Run, ResolveError> {
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        run(&cx, &serde_json::from_str(json).unwrap())
    }

    const FLIP: &str = r#"{"FlipCoin":{"win":[{"Draw":{}}],"lose":[{"Shuffle":{}}]}}"#;

    /// The result of the flip the game makes.
    fn result(game: &GameState) -> bool {
        let Run::Done { events, next, .. } = flip(game, FLIP).unwrap() else {
            panic!("a flip asks nothing");
        };
        let [Event::CoinFlipped { player, won }] = events.as_slice() else {
            panic!("one flip is logged: {events:?}");
        };
        assert_eq!(*player, ME);
        assert_eq!(
            next,
            Next::Enter(usize::from(!*won)),
            "the list for the result"
        );
        *won
    }

    #[test]
    fn the_same_seed_flips_the_same_and_the_seeds_give_both_results() {
        let results: Vec<bool> = (0..32).map(|seed| result(&game(seed))).collect();
        assert_eq!(
            results,
            (0..32).map(|seed| result(&game(seed))).collect::<Vec<_>>()
        );
        assert!(results.contains(&true) && results.contains(&false));
    }

    #[rstest]
    #[case::the_player_who_left(r#"{"FlipCoin":{"who":"Opponents","win":[{"Draw":{}}]}}"#)]
    fn a_player_who_has_left_flips_nothing(#[case] json: &str) {
        let mut game = game(1);
        crate::fold::apply_event(
            &mut game,
            &Event::PlayerLost {
                player: PlayerId(1),
                reason: crate::player::LossReason::Conceded,
            },
        )
        .unwrap();
        let Run::Done { events, next, .. } = flip(&game, json).unwrap() else {
            panic!("a flip asks nothing");
        };
        assert!(events.is_empty());
        assert_eq!(next, Next::Sibling);
    }

    #[test]
    fn another_effect_is_refused_by_name() {
        let error = flip(&game(1), r#"{"Draw":{}}"#).unwrap_err();
        assert!(error.to_string().contains("FlipCoin"), "{error}");
    }
}
