//! `Effect::Flip`: flip cards on the battlefield are flipped (CR 710).

use mtg_cards::definition::Layout;
use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::zones::Zone;

/// Each permanent the effect names that is still on the battlefield, is a flip card and is not flipped yet is flipped, all
/// together and in the order the reference lists them. Anything else is passed over: a flipped permanent stays flipped
/// (CR 710.4), a token, a card of another layout, a card in another zone, a player, an object that changed zones since it
/// was named and a name that repeats.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Flip { what } = effect else {
        return Err(wrong_effect("Flip"));
    };
    let events = cx
        .scope
        .live_objects(what)?
        .into_iter()
        .filter(|object| {
            object.zone() == Zone::Battlefield
                && object.whole_card().layout == Layout::Flip
                && object.face().is_none()
        })
        .map(|object| Event::Flipped {
            object: object.id(),
        })
        .collect();
    Ok(Run::events(events))
}

#[cfg(test)]
mod tests {
    //! Goal: a flip turns each unflipped flip card it names on the battlefield to its bottom half, all together and in the order the reference lists them, and passes over a flipped one, a transforming card, a card with one face, a card in another zone and a player; it binds nothing and goes on to the next effect, and it refuses another effect by name.
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

    /// A creature card with two faces of the layout.
    fn two_faced(name: &str, layout: Layout) -> Arc<CardDefinition> {
        let front = CardDefinition::new(format!("{name} Top"))
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1);
        let back = CardDefinition::new(format!("{name} Bottom"))
            .with_type(CardType::Creature)
            .with_power_toughness(4, 4);
        Arc::new(front.clone().with_faces(layout, vec![front, back]))
    }

    /// Seat 0 controls the Source, two flip cards, a transforming card and a plain Wolf, and holds a flip card; seat 1
    /// controls a flip card.
    fn game() -> GameState {
        let game = setup(
            vec![vec![two_faced("Hand", Layout::Flip)], vec![]],
            vec![
                vec![
                    creature("Source", 1, 1),
                    two_faced("Rat", Layout::Flip),
                    two_faced("Monk", Layout::Flip),
                    two_faced("Moth", Layout::Transform),
                    creature("Wolf", 2, 2),
                ],
                vec![two_faced("Foe", Layout::Flip)],
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
            panic!("a flip needs no answer");
        };
        assert!(bound.is_empty() && snapshots.is_empty());
        assert_eq!(next, Next::Sibling);
        Ok(events)
    }

    fn names(game: &GameState, events: &[Event]) -> Vec<String> {
        events
            .iter()
            .map(|event| match event {
                Event::Flipped { object } => {
                    game.object(*object).unwrap().whole_card().name.clone()
                }
                other => panic!("unexpected {other:?}"),
            })
            .collect()
    }

    #[rstest]
    #[case::a_name(r#"{"Flip":{"what":{"Each":[{"Named":"Rat Top"}]}}}"#, &["Rat Top"])]
    #[case::every_permanent_you_control(
        r#"{"Flip":{"what":{"Each":[{"ControlledBy":"You"}]}}}"#,
        &["Rat Top", "Monk Top"]
    )]
    #[case::of_one_opponent(
        r#"{"Flip":{"what":{"Each":[{"ControlledBy":"Opponents"}]}}}"#,
        &["Foe Top"]
    )]
    #[case::a_transforming_card(r#"{"Flip":{"what":{"Each":[{"Named":"Moth Top"}]}}}"#, &[])]
    #[case::a_card_with_one_face(r#"{"Flip":{"what":{"Each":[{"Named":"Wolf"}]}}}"#, &[])]
    #[case::a_card_in_a_hand(r#"{"Flip":{"what":{"Each":[{"Named":"Hand Top"}]}}}"#, &[])]
    #[case::the_source_itself(r#"{"Flip":{"what":"This"}}"#, &[])]
    #[case::a_player(r#"{"Flip":{"what":"You"}}"#, &[])]
    fn a_flip_flips_the_flip_cards_it_names(#[case] json: &str, #[case] want: &[&str]) {
        let game = game();
        let events = run_effect(&game, json).unwrap();
        assert_eq!(names(&game, &events), want);
    }

    #[test]
    fn a_flipped_permanent_stays_flipped() {
        let mut game = game();
        let rat = named(&game, "Rat");
        let flip = r#"{"Flip":{"what":{"Each":[{"Named":"Rat Bottom"}]}}}"#;
        apply_event(&mut game, &Event::Flipped { object: rat.id }).unwrap();
        assert_eq!(game.object(rat.id).unwrap().face(), Some(1));
        assert_eq!(run_effect(&game, flip).unwrap(), []);
        assert_eq!(
            run_effect(
                &game,
                r#"{"Flip":{"what":{"Each":[{"Named":"Monk Top"}]}}}"#
            )
            .unwrap()
            .len(),
            1
        );
    }

    #[test]
    fn a_card_that_is_not_a_permanent_cannot_be_flipped_by_an_event() {
        let mut game = game();
        let hand = named(&game, "Hand");
        assert!(apply_event(&mut game, &Event::Flipped { object: hand.id }).is_err());
        let wolf = named(&game, "Wolf");
        assert!(apply_event(&mut game, &Event::Flipped { object: wolf.id }).is_err());
    }

    #[test]
    fn another_effect_is_refused_by_name() {
        assert_eq!(
            run_effect(&game(), r#"{"Tap":{"what":"This"}}"#).unwrap_err(),
            ResolveError::Invalid("the Flip primitive was given another effect".into())
        );
    }
}
