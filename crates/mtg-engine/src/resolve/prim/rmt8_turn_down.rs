//! `Effect::TurnFaceDown`: permanents are turned face down by an effect (CR 708.2a): each becomes a nameless,
//! colorless 2/2 creature with no abilities, turned face up again for its morph cost.

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::characteristics::controller_of_object;
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::zones::Zone;

/// Each permanent the effect names that is still on the battlefield and face up is turned face down by its controller, all
/// together and in the order the reference lists them. Anything else is passed over: a permanent that is face down
/// already, a card in another zone, an object that changed zones since it was named and a player.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::TurnFaceDown { what } = effect else {
        return Err(wrong_effect("TurnFaceDown"));
    };
    let state = cx.state();
    let events = cx
        .scope
        .live_objects(what)?
        .into_iter()
        .filter(|object| object.zone() == Zone::Battlefield && object.face_down().is_none())
        .map(|object| Event::TurnedFaceDown {
            object: object.id(),
            player: controller_of_object(state, object),
        })
        .collect();
    Ok(Run::events(events))
}

/// `Effect::Harness`: each permanent the effect names that is still on the battlefield and not harnessed yet becomes
/// harnessed; anything else is passed over.
pub(super) fn harness(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Harness { what } = effect else {
        return Err(wrong_effect("Harness"));
    };
    let events = cx
        .scope
        .live_objects(what)?
        .into_iter()
        .filter(|object| {
            object.zone() == Zone::Battlefield
                && object.permanent().is_some_and(|held| !held.harnessed())
        })
        .map(|object| Event::Harnessed {
            object: object.id(),
        })
        .collect();
    Ok(Run::events(events))
}

#[cfg(test)]
mod tests {
    //! Goal: an effect turns face-up permanents face down as nameless 2/2 creatures, each by its controller, at tables of two, three and four seats; a face-down one and a player are passed over, the fold refuses to turn a face-down one down again, and a card turned face down is turned back up for its morph cost.
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_core::card::CardType;
    use mtg_core::ids::{ObjectId, PlayerId};
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;
    use crate::object::FaceDown;
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);
    const FOE: PlayerId = PlayerId(1);

    fn permanent(name: &str, kind: CardType) -> Arc<CardDefinition> {
        Arc::new(CardDefinition::new(name).with_type(kind))
    }

    fn morph(name: &str) -> Arc<CardDefinition> {
        let rules: mtg_cards::ability::Ability =
            serde_json::from_str(r#"{"Keyword":{"Morph":"{1}{U}"}}"#).unwrap();
        Arc::new((*creature(name, 3, 3)).clone().with_rules(rules))
    }

    /// Seat 0 holds Bear, Shifter (with morph) and a Guest; seat 1 holds the battle Siege, the planeswalker Liliana and a
    /// Guest; every other seat a Guest.
    fn table(seats: usize) -> GameState {
        let field = |seat: usize| {
            let mut cards = vec![creature(&format!("Guest {seat}"), 2, 2)];
            if seat == 0 {
                cards.extend([creature("Bear", 2, 2), morph("Shifter")]);
            }
            if seat == 1 {
                cards.extend([
                    permanent("Siege", CardType::Battle),
                    permanent("Liliana", CardType::Planeswalker),
                ]);
            }
            cards
        };
        state(
            (0..seats).map(|_| Vec::new()).collect(),
            (0..seats).map(field).collect(),
        )
    }

    fn id(state: &GameState, name: &str) -> ObjectId {
        state
            .objects()
            .find(|o| o.whole_card().name == name)
            .map(crate::object::GameObject::id)
            .unwrap()
    }

    /// The events of the effect, run by seat 0 with Guest 0 as its source.
    fn turned_down(game: &GameState, json: &str) -> Vec<Event> {
        let effect: Effect = serde_json::from_str(json).unwrap();
        let source = game.object(id(game, "Guest 0")).unwrap().reference();
        let resolving = Resolving::begin(source.id);
        let cx = Cx {
            scope: Scope::announcing(game, ME, source),
            resolving: &resolving,
        };
        let Ok(Run::Done { events, .. }) = run(&cx, &effect) else {
            panic!("turning face down needs no answer");
        };
        events
    }

    #[rstest]
    #[case::each_of_yours(
        r#"{"TurnFaceDown":{"what":{"Each":[{"Named":"Bear"}]}}}"#,
        &["Bear"]
    )]
    #[case::theirs_by_their_controller(
        r#"{"TurnFaceDown":{"what":{"Each":[{"Type":"Battle"}]}}}"#,
        &["Siege"]
    )]
    #[case::a_player(r#"{"TurnFaceDown":{"what":"You"}}"#, &[])]
    fn an_effect_turns_permanents_face_down(
        #[values(2, 3, 4)] seats: usize,
        #[case] json: &str,
        #[case] expected: &[&str],
    ) {
        let mut game = table(seats);
        let events = turned_down(&game, json);
        let named: Vec<&str> = events
            .iter()
            .map(|event| match event {
                Event::TurnedFaceDown { object, player } => {
                    let name = game.object(*object).unwrap().whole_card().name.as_str();
                    let owner = if name == "Siege" { FOE } else { ME };
                    assert_eq!(*player, owner, "its controller turns it");
                    name
                }
                other => panic!("{other:?}"),
            })
            .map(|name| expected.iter().copied().find(|e| *e == name).unwrap())
            .collect();
        assert_eq!(named, expected);
        for event in &events {
            apply_event(&mut game, event).unwrap();
        }
        for name in expected {
            let object = game.object(id(&game, name)).unwrap();
            assert_eq!(object.face_down(), Some(FaceDown::Morph));
            assert_eq!(object.card().name, "", "a face-down permanent has no name");
            assert_eq!(object.card().power, Some(2));
        }
        // A face-down permanent is passed over the next time.
        assert!(turned_down(&game, json).is_empty());
    }

    #[rstest]
    fn a_card_turned_face_down_is_turned_up_for_its_morph_cost(#[values(2, 3, 4)] seats: usize) {
        let mut game = table(seats);
        let shifter = id(&game, "Shifter");
        for event in turned_down(
            &game,
            r#"{"TurnFaceDown":{"what":{"Each":[{"Named":"Shifter"}]}}}"#,
        ) {
            apply_event(&mut game, &event).unwrap();
        }
        let (mana, megamorph) = crate::face_down::turn_up_cost(&game, shifter).unwrap();
        assert_eq!(mana.to_string(), "{1}{U}");
        assert!(!megamorph);
        // Turning it face down again is refused by the fold.
        assert!(
            apply_event(
                &mut game,
                &Event::TurnedFaceDown {
                    object: shifter,
                    player: ME
                }
            )
            .is_err()
        );
    }

    #[rstest]
    fn a_permanent_is_harnessed_once(#[values(2, 3, 4)] seats: usize) {
        let mut game = table(seats);
        let effect: Effect =
            serde_json::from_str(r#"{"Harness":{"what":{"Each":[{"Named":"Bear"}]}}}"#).unwrap();
        let source = game.object(id(&game, "Guest 0")).unwrap().reference();
        let resolving = Resolving::begin(source.id);
        let events = {
            let cx = Cx {
                scope: Scope::announcing(&game, ME, source),
                resolving: &resolving,
            };
            let Ok(Run::Done { events, .. }) = harness(&cx, &effect) else {
                panic!("harnessing needs no answer");
            };
            events
        };
        let bear = id(&game, "Bear");
        assert_eq!(events, vec![Event::Harnessed { object: bear }]);
        for event in &events {
            apply_event(&mut game, event).unwrap();
        }
        assert!(game.object(bear).unwrap().permanent().unwrap().harnessed());
        let cx = Cx {
            scope: Scope::announcing(&game, ME, source),
            resolving: &resolving,
        };
        let Ok(Run::Done { events, .. }) = harness(&cx, &effect) else {
            panic!("harnessing needs no answer");
        };
        assert!(events.is_empty(), "a harnessed permanent is passed over");
        assert!(apply_event(&mut game, &Event::Harnessed { object: bear }).is_err());
    }
}
