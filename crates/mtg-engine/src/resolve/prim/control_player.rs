//! `Effect::ControlPlayer`: the controller of the effect controls a player during that player's next turn (CR 722).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};

/// One `PlayerControlGained` for each player the effect names who is still in the game, controlled by the controller of
/// the effect; `player_control` starts it with that player's next turn.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::ControlPlayer { who, extra_turn } = effect else {
        return Err(wrong_effect("ControlPlayer"));
    };
    let state = cx.state();
    let by = cx.controller();
    Ok(Run::events(
        cx.scope
            .players(who)?
            .into_iter()
            .filter(|player| state.is_alive(*player))
            .map(|player| Event::PlayerControlGained {
                player,
                by,
                extra_turn: *extra_turn,
            })
            .collect(),
    ))
}

#[cfg(test)]
mod tests {
    //! Goal: a control of a player logs one `PlayerControlGained` for each named player still in the game, with the controller of the effect and the extra turn it asks for, and refuses another effect.
    use mtg_core::ids::PlayerId;

    use super::*;
    use crate::fixtures::{creature, state};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;
    use crate::zones::Zone;

    const ME: PlayerId = PlayerId(0);

    fn game() -> GameState {
        state(
            vec![vec![], vec![], vec![]],
            vec![vec![creature("Source", 1, 1)], vec![], vec![]],
        )
    }

    fn run_on(game: &GameState, json: &str) -> Result<Run, ResolveError> {
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        run(&cx, &serde_json::from_str(json).unwrap())
    }

    #[rstest::rstest]
    #[case(r#"{"ControlPlayer":{"who":"Opponents"}}"#, vec![(1, false), (2, false)])]
    #[case(r#"{"ControlPlayer":{"who":"Opponents","extra_turn":true}}"#, vec![(1, true), (2, true)])]
    #[case(r#"{"ControlPlayer":{"who":"You"}}"#, vec![(0, false)])]
    fn each_named_player_is_controlled(#[case] json: &str, #[case] expected: Vec<(u64, bool)>) {
        let Run::Done { events, .. } = run_on(&game(), json).unwrap() else {
            panic!("a control asks nothing");
        };
        let got: Vec<(u64, bool)> = events
            .iter()
            .map(|event| match event {
                Event::PlayerControlGained {
                    player,
                    by,
                    extra_turn,
                } => {
                    assert_eq!(*by, ME);
                    (player.0, *extra_turn)
                }
                other => panic!("unexpected {other:?}"),
            })
            .collect();
        assert_eq!(got, expected);
    }

    #[test]
    fn another_effect_is_refused() {
        assert!(run_on(&game(), r#"{"Draw":{}}"#).is_err());
    }
}
