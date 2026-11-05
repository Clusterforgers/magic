//! `Effect::Remember` and `Effect::Forget`: the source remembers objects by slot.

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::choose::Target;
use crate::events::Event;
use crate::memory::{Held, Remembered};
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
            values: cx
                .scope
                .refs(what)?
                .into_iter()
                .filter_map(|named| match named {
                    Target::Player(player) => Some(Held::Player(player)),
                    Target::Object(_) => None,
                })
                .collect(),
            replace: false,
        },
        Effect::RememberValue { value, slot } => Remembered {
            source,
            slot: slot.clone(),
            objects: Vec::new(),
            values: vec![Held::Number(cx.scope.amount(value)?)],
            replace: false,
        },
        Effect::Forget { slot } => Remembered {
            source,
            slot: slot.clone(),
            objects: Vec::new(),
            values: Vec::new(),
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
            values: Vec::new(),
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

    /// Runs the effects in order on one source, then reads the amount and the players the source remembers.
    fn after(effects: &[&str], leave: Option<&str>, read: &str) -> (i64, usize) {
        let mut game = state(
            vec![vec![], vec![]],
            vec![vec![creature("Source", 1, 1), creature("A", 1, 1)], vec![]],
        );
        let source = named(&game, "Source");
        for json in effects {
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
        }
        if let Some(name) = leave {
            let object = named(&game, name);
            let event = Event::ZoneChanged {
                object: object.id,
                from: crate::zones::Zone::Battlefield,
                to: crate::zones::Zone::Exile,
                cause: crate::events::ZoneCause::Moved,
            };
            apply_event(&mut game, &event).unwrap();
        }
        let scope = Scope::announcing(&game, PlayerId(0), source);
        let on = Box::new(mtg_cards::refs::Ref::This);
        let amount = serde_json::from_str(read).unwrap();
        let players = scope
            .refs(&mtg_cards::refs::Ref::Remembered {
                on,
                slot: "s".to_owned(),
            })
            .unwrap()
            .iter()
            .filter(|named| matches!(named, Target::Player(_)))
            .count();
        (scope.amount(&amount).unwrap(), players)
    }

    const SEVEN: &str = r#"{"RememberValue":{"value":7,"slot":"s"}}"#;
    const THREE: &str = r#"{"RememberValue":{"value":3,"slot":"s"}}"#;
    const YOU: &str = r#"{"Remember":{"what":"You","slot":"s"}}"#;
    const READ: &str = r#"{"Remembered":{"on":"This","slot":"s"}}"#;

    #[rstest]
    #[case::nothing_held(&[], None, 0, 0)]
    #[case::a_number(&[SEVEN], None, 7, 0)]
    #[case::numbers_add_up(&[SEVEN, THREE, SEVEN], None, 17, 0)]
    #[case::a_player_is_held_once(&[YOU, YOU], None, 0, 1)]
    #[case::both_kinds(&[YOU, THREE], None, 3, 1)]
    #[case::another_object_leaving_keeps_it(&[SEVEN, YOU], Some("A"), 7, 1)]
    #[case::the_source_leaving_forgets_it(&[SEVEN, YOU], Some("Source"), 0, 0)]
    fn numbers_and_players_are_remembered_by_slot(
        #[case] effects: &[&str],
        #[case] leave: Option<&str>,
        #[case] number: i64,
        #[case] players: usize,
    ) {
        assert_eq!(after(effects, leave, READ), (number, players));
    }

    #[test]
    fn forget_empties_numbers_too() {
        let forget = r#"{"Forget":{"slot":"s"}}"#;
        assert_eq!(after(&[SEVEN, forget], None, READ), (0, 0));
    }
}
