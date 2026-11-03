//! `Effect::Remember` and `Effect::Forget`: the source remembers objects by slot.

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::events::Event;
use crate::memory::Remembered;
use crate::resolve::{Cx, ResolveError, Run};

/// `Remember` adds the objects `what` names (still the objects named) to the slot of the source; `Forget` empties the slot.
/// A source that has left its zone remembers nothing, since its memory ended with it.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let source = cx.scope.source;
    let here = cx
        .state()
        .object(source.id)
        .is_some_and(|object| object.counter() == source.counter);
    let change = match effect {
        Effect::Remember { what, slot } => Remembered {
            source,
            slot: slot.clone(),
            objects: cx
                .scope
                .live_objects(what)?
                .into_iter()
                .map(crate::object::GameObject::reference)
                .collect(),
            replace: false,
        },
        Effect::Forget { slot } => Remembered {
            source,
            slot: slot.clone(),
            objects: Vec::new(),
            replace: true,
        },
        _ => return Err(wrong_effect("Remember")),
    };
    let events = if here {
        vec![Event::Remembered(Box::new(change))]
    } else {
        Vec::new()
    };
    Ok(Run::events(events))
}

#[cfg(test)]
mod tests {
    //! Goal: Remember adds the objects it names to the slot of the source, Forget empties the slot, and a source that has left remembers nothing.
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;
    use crate::memory::recall;
    use crate::object::ObjectRef;
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;

    fn named(game: &GameState, name: &str) -> ObjectRef {
        game.objects()
            .find(|object| object.card().name == name)
            .map(crate::object::GameObject::reference)
            .unwrap()
    }

    #[rstest]
    #[case::one(r#"{"Remember":{"what":{"Each":[{"Named":"A"}]},"slot":"s"}}"#, &["A", "B"])]
    #[case::a_group(r#"{"Remember":{"what":{"Each":[{"Type":"Creature"}]},"slot":"s"}}"#, &["A", "B", "Source"])]
    #[case::nothing(r#"{"Remember":{"what":{"Each":[{"Named":"Z"}]},"slot":"s"}}"#, &["B"])]
    #[case::forget(r#"{"Forget":{"slot":"s"}}"#, &[])]
    #[case::forget_another_slot(r#"{"Forget":{"slot":"t"}}"#, &["B"])]
    fn the_effects_change_the_memory_of_the_source(#[case] json: &str, #[case] expected: &[&str]) {
        let mut game = state(
            vec![vec![], vec![]],
            vec![
                vec![
                    creature("Source", 1, 1),
                    creature("A", 1, 1),
                    creature("B", 1, 1),
                ],
                vec![],
            ],
        );
        let source = named(&game, "Source");
        let first = Remembered {
            source,
            slot: "s".to_owned(),
            objects: vec![named(&game, "B")],
            replace: false,
        };
        apply_event(&mut game, &Event::Remembered(Box::new(first))).unwrap();
        let effect: Effect = serde_json::from_str(json).unwrap();
        let resolving = Resolving::begin(source.id);
        let cx = Cx {
            scope: Scope::announcing(&game, PlayerId(0), source),
            resolving: &resolving,
        };
        let Run::Done { events, .. } = run(&cx, &effect).unwrap() else {
            panic!("the effect is done");
        };
        for event in &events {
            apply_event(&mut game, event).unwrap();
        }
        let mut seen: Vec<String> = recall(&game, source, "s")
            .into_iter()
            .map(|object| game.object(object.id).unwrap().card().name.clone())
            .collect();
        seen.sort();
        assert_eq!(seen, expected);
    }
}
