//! `Effect::BecomeMonarch`: a player becomes the monarch (CR 724).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::monarch::become_monarch;
use crate::resolve::{Cx, ResolveError, Run};

/// The last player the effect names who is still in the game becomes the monarch, and the one before stops being it
/// (CR 724.2). A player who already is the monarch changes nothing and no event is logged.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::BecomeMonarch { who } = effect else {
        return Err(wrong_effect("BecomeMonarch"));
    };
    let state = cx.state();
    let chosen = cx
        .scope
        .players(who)?
        .into_iter()
        .rfind(|player| state.is_alive(*player));
    Ok(Run::events(
        chosen
            .and_then(|player| become_monarch(state, player))
            .into_iter()
            .collect(),
    ))
}

#[cfg(test)]
mod tests {
    //! Goal: becoming the monarch logs one `MonarchChanged` for the player the effect names, nothing for a player who already is the monarch or one who has left the game, and refuses another effect; the fold makes that player the only monarch.
    use mtg_core::ids::PlayerId;

    use super::*;
    use crate::events::Event;
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;
    use crate::resolve::{Next, Resolving, Scope};
    use crate::state::GameState;
    use crate::zones::Zone;

    const ME: PlayerId = PlayerId(0);
    const FOE: PlayerId = PlayerId(1);

    fn game() -> GameState {
        state(
            vec![vec![], vec![]],
            vec![vec![creature("Source", 1, 1)], vec![]],
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

    fn events(run: Run) -> Vec<Event> {
        let Run::Done { events, next, .. } = run else {
            panic!("becoming the monarch asks nothing");
        };
        assert_eq!(next, Next::Sibling);
        events
    }

    #[test]
    fn the_controller_becomes_the_monarch_and_a_second_time_changes_nothing() {
        let mut game = game();
        let done = events(run_on(&game, r#"{"BecomeMonarch":{}}"#).unwrap());
        assert_eq!(done, [Event::MonarchChanged { player: ME }]);
        apply_event(&mut game, &done[0]).unwrap();
        assert_eq!(game.monarch(), Some(ME));
        assert_eq!(
            events(run_on(&game, r#"{"BecomeMonarch":{}}"#).unwrap()),
            []
        );
    }

    #[test]
    fn another_player_takes_the_designation_from_the_monarch() {
        let mut game = game();
        apply_event(&mut game, &Event::MonarchChanged { player: ME }).unwrap();
        let done = events(run_on(&game, r#"{"BecomeMonarch":{"who":"Opponents"}}"#).unwrap());
        assert_eq!(done, [Event::MonarchChanged { player: FOE }]);
        apply_event(&mut game, &done[0]).unwrap();
        assert_eq!(game.monarch(), Some(FOE));
    }

    #[test]
    fn a_player_who_has_left_the_game_does_not_become_the_monarch() {
        let mut game = game();
        apply_event(
            &mut game,
            &Event::PlayerLost {
                player: FOE,
                reason: crate::player::LossReason::Conceded,
            },
        )
        .unwrap();
        assert_eq!(
            events(run_on(&game, r#"{"BecomeMonarch":{"who":"Opponents"}}"#).unwrap()),
            []
        );
    }

    #[test]
    fn another_effect_is_a_bug_in_the_dispatch() {
        let refused = run_on(&game(), r#"{"Draw":{}}"#);
        assert!(matches!(refused, Err(ResolveError::Invalid(_))));
    }
}
