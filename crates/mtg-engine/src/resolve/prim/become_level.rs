//! `Effect::BecomeLevel`: the source Class becomes a higher level (CR 716.2).

use mtg_cards::effect::Effect;
use mtg_cards::refs::Ref;

use super::explore::actors;
use super::wrong_effect;
use crate::events::Event;
use crate::object::{GameObject, PermanentState};
use crate::resolve::{Cx, ResolveError, Run};

/// A source on the battlefield that is below the level gains it as one `ClassLevelGained`. A source that has left the
/// battlefield or is at that level or above does nothing.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::BecomeLevel { level } = effect else {
        return Err(wrong_effect("BecomeLevel"));
    };
    let mut events = Vec::new();
    for source in actors(cx, &Ref::This)? {
        let Some(holder) = source.permanent else {
            continue;
        };
        let reached = cx
            .state()
            .object(holder.id)
            .and_then(GameObject::permanent)
            .is_none_or(|permanent| PermanentState::level(permanent) >= *level);
        if !reached {
            events.push(Event::ClassLevelGained {
                object: source.object,
                player: source.player,
                level: *level,
            });
        }
    }
    Ok(Run::events(events))
}

#[cfg(test)]
mod tests {
    //! Goal: a Class below the level becomes it with one `ClassLevelGained` for its controller; a Class at that level or above, or one that has left the battlefield, does nothing, and another effect is refused.
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, setup};
    use crate::fold::apply_event;
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
    #[case::the_second_level(2, 0)]
    #[case::the_third_level(3, 0)]
    #[case::the_third_from_the_second(3, 2)]
    fn a_class_below_the_level_becomes_it(#[case] level: u32, #[case] held: u32) {
        let mut game = game(true);
        if held > 0 {
            let object = game.objects().find(|o| o.card().name == "Source").unwrap();
            let (object, player) = (object.id(), ME);
            apply_event(
                &mut game,
                &Event::ClassLevelGained {
                    object,
                    player,
                    level: held,
                },
            )
            .unwrap();
        }
        let found = events(run_with(
            &game,
            &format!(r#"{{"BecomeLevel":{{"level":{level}}}}}"#),
        ));
        assert!(matches!(
            found.as_slice(),
            [Event::ClassLevelGained { player, level: reached, .. }] if *player == ME && *reached == level
        ));
    }

    #[rstest]
    #[case::the_same_level(2, 2)]
    #[case::a_higher_level(2, 3)]
    #[case::the_first_level_it_has(1, 0)]
    fn a_class_at_the_level_or_above_does_nothing(#[case] level: u32, #[case] held: u32) {
        let mut game = game(true);
        if held > 0 {
            let object = game.objects().find(|o| o.card().name == "Source").unwrap();
            let (object, player) = (object.id(), ME);
            apply_event(
                &mut game,
                &Event::ClassLevelGained {
                    object,
                    player,
                    level: held,
                },
            )
            .unwrap();
        }
        let json = format!(r#"{{"BecomeLevel":{{"level":{level}}}}}"#);
        assert_eq!(events(run_with(&game, &json)).len(), 0);
    }

    #[test]
    fn a_source_that_has_left_the_battlefield_does_nothing() {
        let game = game(false);
        assert_eq!(
            events(run_with(&game, r#"{"BecomeLevel":{"level":2}}"#)).len(),
            0
        );
    }

    #[test]
    fn another_effect_is_refused() {
        let game = game(true);
        assert!(run_with(&game, r#"{"Adapt":{"count":3}}"#).is_err());
    }
}
