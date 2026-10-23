//! `Effect::Crewed`: the source Vehicle becomes crewed or the source Mount saddled (CR 702.122, 702.171).

use mtg_cards::effect::Effect;
use mtg_cards::refs::Ref;

use super::explore::actors;
use super::wrong_effect;
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};

/// A source that is on the battlefield becomes crewed (saddled) by the creatures tapped to pay the ability, in one
/// `Crewed`; a source that has left the battlefield does nothing.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Crewed { saddle } = effect else {
        return Err(wrong_effect("Crewed"));
    };
    let Some(source) = actors(cx, &Ref::This)?
        .into_iter()
        .find(|source| source.permanent.is_some())
    else {
        return Ok(Run::events(Vec::new()));
    };
    let creatures = cx
        .scope
        .objects(&Ref::Paid)?
        .into_iter()
        .map(|paid| paid.id)
        .collect();
    Ok(Run::events(vec![Event::Crewed {
        object: source.object,
        player: source.player,
        creatures,
        saddle: *saddle,
    }]))
}

#[cfg(test)]
mod tests {
    //! Goal: a crew or saddle effect on a source that is on the battlefield logs one `Crewed` for its controller, saddled only for a saddle, with no creature when the cost picked none; on a source that has left the battlefield it does nothing; and it refuses another effect.
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, setup};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);

    /// The Source is on the battlefield of seat 0, or in its hand.
    fn game(on_board: bool) -> GameState {
        let source = vec![creature("Source", 1, 1)];
        let (hand, board) = if on_board {
            (vec![], source)
        } else {
            (source, vec![])
        };
        GameState::from_setup(&setup(vec![hand, vec![]], vec![board, vec![]])).unwrap()
    }

    fn run_with(game: &GameState, json: &str) -> Result<Run, ResolveError> {
        let source = game.objects().find(|o| o.card().name == "Source").unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        run(&cx, &serde_json::from_str(json).unwrap())
    }

    fn events(result: Result<Run, ResolveError>) -> Vec<Event> {
        match result.unwrap() {
            Run::Done { events, .. } => events,
            _ => panic!("done"),
        }
    }

    #[rstest]
    #[case::crew(r#"{"Crewed":{}}"#, false)]
    #[case::saddle(r#"{"Crewed":{"saddle":true}}"#, true)]
    fn a_source_on_the_battlefield_becomes_crewed_or_saddled(
        #[case] json: &str,
        #[case] saddled: bool,
    ) {
        let game = game(true);
        let source = game.objects().find(|o| o.card().name == "Source").unwrap();
        assert_eq!(
            events(run_with(&game, json)),
            [Event::Crewed {
                object: source.id(),
                player: ME,
                creatures: Vec::new(),
                saddle: saddled,
            }]
        );
    }

    #[test]
    fn a_source_that_has_left_the_battlefield_does_nothing() {
        let game = game(false);
        assert_eq!(events(run_with(&game, r#"{"Crewed":{}}"#)), []);
    }

    #[test]
    fn another_effect_is_refused() {
        let game = game(true);
        assert!(run_with(&game, r#"{"Learn":{}}"#).is_err());
    }
}
