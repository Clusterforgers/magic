//! `Effect::Transform`: transforming double-faced permanents are turned over (CR 701.28).

use mtg_cards::definition::Layout;
use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::day_night::turned_over;
use crate::events::Event;
use crate::object::GameObject;
use crate::resolve::{Cx, ResolveError, Run};
use crate::zones::Zone;

/// Each permanent the effect names that is still on the battlefield and is a transforming double-faced card is turned
/// to its other face, all together, in the order the reference lists them (CR 701.28a). Anything else is passed over: a
/// token, a card of another layout, a card in another zone, a player, an object that changed zones since it was named and
/// a name that repeats (CR 701.28c). An ability of a permanent that tries to transform it does nothing when the
/// permanent has transformed since the ability was put on the stack (CR 701.28f).
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Transform { what } = effect else {
        return Err(wrong_effect("Transform"));
    };
    let mut events = Vec::new();
    for object in cx.scope.live_objects(what)? {
        if object.zone() == Zone::Battlefield
            && object.whole_card().layout == Layout::Transform
            && !turned_since_stacked(cx, object)
        {
            events.push(Event::Transformed {
                object: object.id(),
                face: turned_over(object.face()),
            });
        }
    }
    Ok(Run::events(events))
}

/// Whether `object` is the source of the ability that resolves and has been turned over since the ability was put on the
/// stack: it is not the face the ability's snapshot of it shows.
fn turned_since_stacked(cx: &Cx<'_>, object: &GameObject) -> bool {
    let reference = object.reference();
    if cx.scope.source != reference {
        return false;
    }
    let Some(then) = cx
        .scope
        .snapshots
        .iter()
        .find(|snapshot| snapshot.reference() == reference)
    else {
        return false;
    };
    then.face != object.face()
}

#[cfg(test)]
mod tests {
    //! Goal: a transform turns each transforming double-faced permanent it names to its other face, all together and in the order the reference lists them, and passes over permanents of any other kind (a card with one face, a modal double-faced card, a token), cards in other zones, players and objects that changed zones; it keeps the object, its counters and its tapped state, binds nothing and goes on to the next effect; the source of an ability that has transformed since the ability went on the stack is not turned over again; and it refuses another effect by name.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_cards::definition::Layout;
    use mtg_core::card::CardType;
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, setup};
    use crate::fold::apply_event;
    use crate::object::ObjectRef;
    use crate::resolve::{Next, Resolving, Scope};
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);

    /// A creature card with two faces of the layout.
    fn two_faced(name: &str, layout: Layout) -> Arc<CardDefinition> {
        let front = CardDefinition::new(format!("{name} Front"))
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1);
        let back = CardDefinition::new(format!("{name} Back"))
            .with_type(CardType::Creature)
            .with_power_toughness(4, 4);
        Arc::new(front.clone().with_faces(layout, vec![front, back]))
    }

    /// Seat 0 controls the Source and two transforming cards, a modal double-faced card and a plain Wolf; seat 1 a
    /// transforming card.
    fn game() -> GameState {
        let game = setup(
            vec![vec![], vec![]],
            vec![
                vec![
                    creature("Source", 1, 1),
                    two_faced("Moth", Layout::Transform),
                    two_faced("Egg", Layout::Transform),
                    two_faced("Modal", Layout::ModalDfc),
                    creature("Wolf", 2, 2),
                ],
                vec![two_faced("Foe", Layout::Transform)],
            ],
        );
        GameState::from_setup(&game).unwrap()
    }

    fn named(game: &GameState, name: &str) -> ObjectRef {
        game.objects()
            .find(|o| o.whole_card().name.starts_with(name))
            .map(GameObject::reference)
            .unwrap()
    }

    fn run_effect(game: &GameState, json: &str) -> Result<Vec<Event>, ResolveError> {
        let effect: Effect = serde_json::from_str(json).unwrap();
        let source = named(game, "Source");
        let resolving = Resolving::begin(source.id);
        let cx = Cx {
            scope: Scope::announcing(game, ME, source),
            resolving: &resolving,
        };
        let Run::Done {
            events,
            bound,
            snapshots,
            next,
        } = run(&cx, &effect)?
        else {
            panic!("a transform needs no answer");
        };
        assert!(bound.is_empty() && snapshots.is_empty());
        assert_eq!(next, Next::Sibling);
        Ok(events)
    }

    fn names(game: &GameState, events: &[Event]) -> Vec<String> {
        events
            .iter()
            .map(|event| match event {
                Event::Transformed { object, .. } => {
                    game.object(*object).unwrap().whole_card().name.clone()
                }
                other => panic!("unexpected {other:?}"),
            })
            .collect()
    }

    #[rstest]
    #[case::a_name(r#"{"Transform":{"what":{"Each":[{"Named":"Moth Front"}]}}}"#, &["Moth Front"])]
    #[case::every_permanent_you_control(
        r#"{"Transform":{"what":{"Each":[{"ControlledBy":"You"}]}}}"#,
        &["Moth Front", "Egg Front"]
    )]
    #[case::of_one_opponent(
        r#"{"Transform":{"what":{"Each":[{"ControlledBy":"Opponents"}]}}}"#,
        &["Foe Front"]
    )]
    #[case::a_modal_card(r#"{"Transform":{"what":{"Each":[{"Named":"Modal Front"}]}}}"#, &[])]
    #[case::a_card_with_one_face(r#"{"Transform":{"what":{"Each":[{"Named":"Wolf"}]}}}"#, &[])]
    #[case::the_source_itself(r#"{"Transform":{"what":"This"}}"#, &[])]
    #[case::a_player(r#"{"Transform":{"what":"You"}}"#, &[])]
    fn a_transform_turns_the_double_faced_permanents_it_names(
        #[case] json: &str,
        #[case] want: &[&str],
    ) {
        let game = game();
        let events = run_effect(&game, json).unwrap();
        assert_eq!(names(&game, &events), want);
    }

    #[rstest]
    #[case::front_up(None, "Moth Front", Some(1))]
    #[case::back_up(Some(1), "Moth Back", None)]
    fn the_face_it_turns_to_is_the_other_one(
        #[case] now: Option<u8>,
        #[case] name: &str,
        #[case] then: Option<u8>,
    ) {
        let mut game = game();
        let moth = named(&game, "Moth");
        apply_event(
            &mut game,
            &Event::Transformed {
                object: moth.id,
                face: now,
            },
        )
        .unwrap();
        // A name is the name of the face that is up.
        let json = format!(r#"{{"Transform":{{"what":{{"Each":[{{"Named":"{name}"}}]}}}}}}"#);
        let events = run_effect(&game, &json).unwrap();
        assert_eq!(
            events,
            [Event::Transformed {
                object: moth.id,
                face: then
            }]
        );
    }

    #[test]
    fn another_effect_is_refused_by_name() {
        assert_eq!(
            run_effect(&game(), r#"{"Tap":{"what":"This"}}"#).unwrap_err(),
            ResolveError::Invalid("the Transform primitive was given another effect".into())
        );
    }
}
