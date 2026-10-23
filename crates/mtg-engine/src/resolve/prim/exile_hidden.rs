//! `Effect::ExileHidden`: objects are exiled face down, linked to the permanent that hides them (hideaway, CR 702.75a).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::events::{Event, ZoneCause};
use crate::object::ObjectKind;
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::zones::Zone;

/// Each object the effect names that is still where it was named moves to exile with `ZoneCause::Hideaway` naming the
/// source of the effect, so it lies face down there and the source is the permanent its abilities read it by. An
/// ability, the resolving spell, an object already in exile, one that has changed zones since it was named and a name
/// that repeats are passed over.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::ExileHidden { what } = effect else {
        return Err(wrong_effect("ExileHidden"));
    };
    let mut events = Vec::new();
    for object in cx.scope.live_objects(what)? {
        let resolving_spell = object.zone() == Zone::Stack && object.id() == cx.resolving.object();
        if object.kind() == ObjectKind::Ability || resolving_spell || object.zone() == Zone::Exile {
            continue;
        }
        events.push(Event::ZoneChanged {
            object: object.id(),
            from: object.zone(),
            to: Zone::Exile,
            cause: ZoneCause::Hideaway {
                by: cx.scope.source,
            },
        });
    }
    Ok(Run::Done {
        events,
        bound: Vec::new(),
        snapshots: Vec::new(),
        next: Next::Sibling,
    })
}

#[cfg(test)]
mod tests {
    //! Goal: hiding exiles each card the effect names from where it is, face down and linked to the source of the effect (the cause names it), passes over what is already in exile and an ability, and the fold lays the card face down in exile with the link kept until it changes zones.
    use mtg_core::ids::{ObjectId, PlayerId};
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, state};
    use crate::object::FaceDown;
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);

    fn game() -> GameState {
        state(
            vec![vec![], vec![]],
            vec![vec![creature("Source", 1, 1)], vec![]],
        )
    }

    fn hidden(game: &GameState, json: &str) -> Vec<Event> {
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        let effect: Effect = serde_json::from_str(json).unwrap();
        let Run::Done { events, next, .. } = run(&cx, &effect).unwrap() else {
            panic!("hiding asks nothing");
        };
        assert_eq!(next, Next::Sibling);
        events
    }

    #[rstest]
    #[case::the_library(
        r#"{"ExileHidden":{"what":{"Each":[{"In":"Library"},{"OwnedBy":"You"}]}}}"#,
        5
    )]
    #[case::the_battlefield(r#"{"ExileHidden":{"what":{"Each":[{"Type":"Creature"}]}}}"#, 1)]
    #[case::cards_already_in_exile(r#"{"ExileHidden":{"what":{"Each":[{"In":"Exile"}]}}}"#, 0)]
    fn each_card_named_is_exiled_with_the_source_as_its_hider(
        #[case] json: &str,
        #[case] n: usize,
    ) {
        let game = game();
        let source = game.zone(Zone::Battlefield)[0];
        let events = hidden(&game, json);
        assert_eq!(events.len(), n);
        for event in &events {
            let Event::ZoneChanged {
                to: Zone::Exile,
                cause: ZoneCause::Hideaway { by },
                ..
            } = event
            else {
                panic!("not a hidden exile: {event:?}");
            };
            assert_eq!(by.id, source);
        }
    }

    #[test]
    fn the_fold_lays_the_card_face_down_in_exile_linked_to_the_source() {
        let mut game = game();
        let source = game
            .object(game.zone(Zone::Battlefield)[0])
            .unwrap()
            .reference();
        let card: ObjectId = *game.zone(Zone::Library(ME)).last().unwrap();
        let events = hidden(
            &game,
            r#"{"ExileHidden":{"what":{"Each":[{"In":"Library"},{"OwnedBy":"You"}]}}}"#,
        );
        for event in &events {
            crate::fold::apply_event(&mut game, event).unwrap();
        }
        let object = game.object(card).unwrap();
        assert_eq!(object.zone(), Zone::Exile);
        assert_eq!(object.face_down(), Some(FaceDown::Hideaway { by: source }));
    }
}
