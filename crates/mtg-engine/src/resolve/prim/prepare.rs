//! `Effect::Prepare`: prepare cards on the battlefield become prepared.

use mtg_cards::definition::Layout;
use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::zones::Zone;

/// Each permanent the effect names that is still on the battlefield, is a prepare card and is not prepared becomes
/// prepared, in the order the reference lists them. Anything else is passed over: a prepared permanent, a token, a card of
/// another layout, a card in another zone, a player, an object that changed zones since it was named and a name that
/// repeats.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Prepare { what } = effect else {
        return Err(wrong_effect("Prepare"));
    };
    let events = cx
        .scope
        .live_objects(what)?
        .into_iter()
        .filter(|object| {
            object.zone() == Zone::Battlefield
                && object.whole_card().layout == Layout::Prepare
                && object.permanent().is_some_and(|held| !held.prepared())
        })
        .map(|object| Event::Prepared {
            object: object.id(),
        })
        .collect();
    Ok(Run::events(events))
}

#[cfg(test)]
mod tests {
    //! Goal: preparing makes each unprepared prepare card it names on the battlefield prepared, in the order the reference lists them, and passes over a prepared one, a card of another layout, a card in another zone and a player; it binds nothing and goes on to the next effect, and it refuses another effect by name.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_core::card::CardType;
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, setup};
    use crate::fold::apply_event;
    use crate::object::{GameObject, ObjectRef};
    use crate::resolve::{Next, Resolving, Scope};
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);

    /// A creature card whose second face is a sorcery.
    fn prepare_card(name: &str, layout: Layout) -> Arc<CardDefinition> {
        let front = CardDefinition::new(format!("{name} Creature"))
            .with_type(CardType::Creature)
            .with_power_toughness(2, 2);
        let spell = CardDefinition::new(format!("{name} Spell")).with_type(CardType::Sorcery);
        Arc::new(front.clone().with_faces(layout, vec![front, spell]))
    }

    /// Seat 0 controls the Source, two prepare cards, a flip card and a plain Wolf and holds a prepare card; seat 1
    /// controls a prepare card.
    fn game() -> GameState {
        let game = setup(
            vec![vec![prepare_card("Hand", Layout::Prepare)], vec![]],
            vec![
                vec![
                    creature("Source", 1, 1),
                    prepare_card("Owl", Layout::Prepare),
                    prepare_card("Fox", Layout::Prepare),
                    prepare_card("Moth", Layout::Flip),
                    creature("Wolf", 2, 2),
                ],
                vec![prepare_card("Foe", Layout::Prepare)],
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
            panic!("preparing needs no answer");
        };
        assert!(bound.is_empty() && snapshots.is_empty());
        assert_eq!(next, Next::Sibling);
        Ok(events)
    }

    fn names(game: &GameState, events: &[Event]) -> Vec<String> {
        events
            .iter()
            .map(|event| match event {
                Event::Prepared { object } => {
                    game.object(*object).unwrap().whole_card().name.clone()
                }
                other => panic!("unexpected {other:?}"),
            })
            .collect()
    }

    #[rstest]
    #[case::a_name(r#"{"Prepare":{"what":{"Each":[{"Named":"Owl Creature"}]}}}"#, &["Owl Creature"])]
    #[case::every_permanent_you_control(
        r#"{"Prepare":{"what":{"Each":[{"ControlledBy":"You"}]}}}"#,
        &["Owl Creature", "Fox Creature"]
    )]
    #[case::of_one_opponent(
        r#"{"Prepare":{"what":{"Each":[{"ControlledBy":"Opponents"}]}}}"#,
        &["Foe Creature"]
    )]
    #[case::a_flip_card(r#"{"Prepare":{"what":{"Each":[{"Named":"Moth Creature"}]}}}"#, &[])]
    #[case::a_card_with_one_face(r#"{"Prepare":{"what":{"Each":[{"Named":"Wolf"}]}}}"#, &[])]
    #[case::a_card_in_a_hand(r#"{"Prepare":{"what":{"Each":[{"Named":"Hand Creature"}]}}}"#, &[])]
    #[case::the_source_itself(r#"{"Prepare":{"what":"This"}}"#, &[])]
    #[case::a_player(r#"{"Prepare":{"what":"You"}}"#, &[])]
    fn preparing_prepares_the_prepare_cards_it_names(#[case] json: &str, #[case] want: &[&str]) {
        let game = game();
        let events = run_effect(&game, json).unwrap();
        assert_eq!(names(&game, &events), want);
    }

    #[test]
    fn a_prepared_permanent_is_passed_over() {
        let mut game = game();
        let owl = named(&game, "Owl");
        let owls = r#"{"Prepare":{"what":{"Each":[{"Named":"Owl Creature"}]}}}"#;
        apply_event(&mut game, &Event::Prepared { object: owl.id }).unwrap();
        assert!(game.object(owl.id).unwrap().permanent().unwrap().prepared());
        assert_eq!(run_effect(&game, owls).unwrap(), []);
        assert_eq!(
            run_effect(
                &game,
                r#"{"Prepare":{"what":{"Each":[{"Named":"Fox Creature"}]}}}"#
            )
            .unwrap()
            .len(),
            1
        );
    }

    #[test]
    fn the_fold_refuses_what_cannot_be_prepared() {
        let mut game = game();
        let hand = named(&game, "Hand");
        let wolf = named(&game, "Wolf");
        let owl = named(&game, "Owl");
        for object in [hand.id, wolf.id] {
            assert!(apply_event(&mut game, &Event::Prepared { object }).is_err());
        }
        // Only a prepared permanent stops being so.
        assert!(apply_event(&mut game, &Event::Unprepared { object: owl.id }).is_err());
        apply_event(&mut game, &Event::Prepared { object: owl.id }).unwrap();
        assert!(apply_event(&mut game, &Event::Prepared { object: owl.id }).is_err());
        apply_event(&mut game, &Event::Unprepared { object: owl.id }).unwrap();
    }

    #[test]
    fn another_effect_is_refused_by_name() {
        assert_eq!(
            run_effect(&game(), r#"{"Tap":{"what":"This"}}"#).unwrap_err(),
            ResolveError::Invalid("the Prepare primitive was given another effect".into())
        );
    }
}
