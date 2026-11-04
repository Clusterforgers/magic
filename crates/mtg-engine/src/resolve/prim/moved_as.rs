//! The `as` name of a `Mill`, `Sacrifice`, `Destroy` or `Move`: the objects the step moved, as the objects they become.

use mtg_cards::effect::Effect;

use crate::choose::Target;
use crate::events::Event;
use crate::object::ObjectRef;
use crate::resolve::{Cx, Next, Run};

/// The name the effect binds its moved objects to, when it has one.
fn name(effect: &Effect) -> Option<&str> {
    match effect {
        Effect::Mill { bind, .. }
        | Effect::Sacrifice { bind, .. }
        | Effect::Destroy { bind, .. }
        | Effect::Move { bind, .. } => bind.as_deref(),
        _ => None,
    }
}

/// Adds the objects a finished step moved to its bound names under the effect's `as`; a step that asks or runs again, or
/// an effect with no name, passes through. Each object is the one it becomes (CR 400.7): a later step reads the card in its
/// new zone.
pub(super) fn bind(cx: &Cx<'_>, effect: &Effect, mut run: Run) -> Run {
    let Some(name) = name(effect) else {
        return run;
    };
    if let Run::Done {
        events,
        bound,
        next,
        ..
    } = &mut run
        && !matches!(next, Next::Again)
    {
        let mut moved: Vec<Target> = Vec::new();
        for event in events.iter() {
            let Event::ZoneChanged { object, .. } = event else {
                continue;
            };
            let Some(live) = cx.state().object(*object) else {
                continue;
            };
            let target = Target::Object(ObjectRef {
                id: *object,
                counter: live.counter() + 1,
            });
            if !moved.contains(&target) {
                moved.push(target);
            }
        }
        bound.push((name.to_owned(), moved));
    }
    run
}

#[cfg(test)]
mod tests {
    //! Goal: a Mill, Sacrifice, Destroy or Move with an `as` name binds exactly the objects it moved, each as the object it becomes (its counter one on), and without a name binds nothing.
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, setup};
    use crate::resolve::{Resolving, Scope, prim};
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);

    fn game() -> GameState {
        let table = setup(
            vec![Vec::new(); 2],
            vec![vec![creature("Source", 1, 1)], Vec::new()],
        );
        GameState::from_setup(&table).unwrap()
    }

    fn bound_by(json: &str) -> (GameState, Vec<Event>, Vec<(String, Vec<Target>)>) {
        let game = game();
        let effect: Effect = serde_json::from_str(json).unwrap();
        let source = game.objects().find(|o| o.card().name == "Source").unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(&game, ME, source.reference()),
            resolving: &resolving,
        };
        let Ok(Run::Done { events, bound, .. }) = prim::run(&cx, &effect) else {
            panic!("the step needs no answer");
        };
        (game.clone(), events, bound)
    }

    #[rstest]
    #[case::mill(r#"{"Mill":{"count":2,"as":"it"}}"#, 2)]
    #[case::mill_nothing(r#"{"Mill":{"count":0,"as":"it"}}"#, 0)]
    #[case::sacrifice(r#"{"Sacrifice":{"filter":[],"as":"it"}}"#, 1)]
    #[case::destroy(r#"{"Destroy":{"what":"This","as":"it"}}"#, 1)]
    #[case::move_to_graveyard(r#"{"Move":{"what":"This","to":"Graveyard","as":"it"}}"#, 1)]
    fn a_named_step_binds_the_objects_it_moved(#[case] json: &str, #[case] count: usize) {
        let (game, events, bound) = bound_by(json);
        let it = bound
            .iter()
            .find(|(name, _)| name == "it")
            .map(|(_, targets)| targets.clone())
            .unwrap();
        assert_eq!(it.len(), count);
        assert_eq!(events.len(), count);
        for (event, target) in events.iter().zip(&it) {
            let Event::ZoneChanged { object, .. } = event else {
                panic!("a zone change");
            };
            let before = game.object(*object).unwrap().counter();
            let renewed = ObjectRef {
                id: *object,
                counter: before + 1,
            };
            assert_eq!(*target, Target::Object(renewed));
        }
    }

    #[rstest]
    #[case::mill(r#"{"Mill":{"count":2}}"#)]
    #[case::destroy(r#"{"Destroy":{"what":"This"}}"#)]
    fn an_unnamed_step_binds_no_moved_objects(#[case] json: &str) {
        let (_, _, bound) = bound_by(json);
        assert!(bound.iter().all(|(name, _)| name != "it"));
    }
}
