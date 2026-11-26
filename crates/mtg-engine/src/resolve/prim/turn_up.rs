//! `Effect::TurnFaceUp`: face-down permanents are turned face up by an effect (CR 702.37e, 708.8).

use mtg_cards::effect::Effect;

use mtg_core::ids::{ObjectId, PlayerId};

use super::wrong_effect;
use crate::characteristics::controller_of_object;
use crate::choose::{Picked, Selection, pick_selections};
use crate::events::Event;
use crate::object::FaceDown;
use crate::resolve::{Cx, ResolveError, Run};
use crate::zones::Zone;

/// Each face-down permanent the effect names that is still on the battlefield is turned face up by its controller, all
/// together and in the order the reference lists them. Anything else is passed over: a permanent that is face up, a card
/// lying face down in another zone, an object that changed zones since it was named and a player.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::TurnFaceUp { what } = effect else {
        return Err(wrong_effect("TurnFaceUp"));
    };
    let state = cx.state();
    let events: Vec<Event> = cx
        .scope
        .live_objects(what)?
        .into_iter()
        .filter(|object| {
            object.zone() == Zone::Battlefield
                && object
                    .face_down()
                    .is_some_and(FaceDown::shows_characteristics)
                // A face-down instant or sorcery card is revealed and stays face down.
                && !crate::r6008_face_down::stays_face_down(state, object.id())
        })
        .flat_map(|object| {
            let mut events = vec![Event::TurnedFaceUp {
                object: object.id(),
                player: controller_of_object(state, object),
                x: 0,
            }];
            events.extend(crate::announce::turn_up::turned_up_counters(
                state,
                object.id(),
            ));
            events
        })
        .collect();
    r6702_attach(cx, events)
}

/// "As this Aura is turned face up, you may attach it to a creature" (`ReplaceBy::R6702MayAttach`, CR 708.8) and "as this
/// creature is turned face up, you may choose another creature" to copy (`ReplaceBy::AsCopy` turned up): each
/// controller of such an Aura the effect turns up chooses at most one permanent it could enchant face up, judged with
/// every permanent of the effect turned up, and the Aura is attached as it turns.
fn r6702_attach(cx: &Cx<'_>, mut events: Vec<Event>) -> Result<Run, ResolveError> {
    let state = cx.state();
    let auras: Vec<(ObjectId, PlayerId, mtg_cards::refs::Selector)> = events
        .iter()
        .filter_map(|event| match event {
            Event::TurnedFaceUp { object, player, .. } => {
                crate::announce::turn_up::r6702_may_attach(state, *object)
                    .map(|to| (*object, *player, to))
            }
            _ => None,
        })
        .collect();
    // "As this creature enters or is turned face up, you may choose another creature" (CR 708.8).
    let copiers: Vec<(ObjectId, PlayerId, Vec<ObjectId>)> = events
        .iter()
        .filter_map(|event| match event {
            Event::TurnedFaceUp { object, player, .. } => {
                crate::announce::turn_up::r115_copy_options(state, *player, *object)
                    .map(|(options, _)| (*object, *player, options))
            }
            _ => None,
        })
        .collect();
    if auras.is_empty() && copiers.is_empty() {
        return Ok(Run::events(events));
    }
    let mut after = state.clone();
    for event in &events {
        crate::fold::apply_event(&mut after, event)
            .map_err(|error| ResolveError::Invalid(error.to_string()))?;
    }
    let selections: Vec<Selection> = auras
        .iter()
        .map(|(aura, player, to)| Selection {
            one_per_name: false,
            groups: Vec::new(),
            player: *player,
            from: crate::announce::turn_up::r6702_hosts_in(&after, *player, *aura, to),
            min: 0,
            max: 1,
            text: "You may choose a permanent to attach it to.".to_owned(),
        })
        .chain(copiers.iter().map(|(_, player, options)| Selection {
            one_per_name: false,
            groups: Vec::new(),
            player: *player,
            from: options.clone(),
            min: 0,
            max: 1,
            text: "You may choose another creature to copy.".to_owned(),
        }))
        .collect();
    let taken = match pick_selections(cx, &selections)? {
        Picked::Ask(decision) => return Ok(Run::Ask(decision)),
        Picked::Done(taken) => taken,
    };
    let (attached, copied) = taken.split_at(auras.len().min(taken.len()));
    for ((aura, _, _), (_, hosts)) in auras.iter().zip(attached) {
        if let Some(&host) = hosts.first() {
            events.push(Event::Attached {
                object: *aura,
                to: host,
            });
        }
    }
    for ((card, player, _), (_, chosen)) in copiers.iter().zip(copied) {
        if let Some(&of) = chosen.first()
            && let Some(copy) =
                crate::announce::turn_up::r115_turned_up_copy(state, *player, *card, of)
        {
            events.push(copy);
        }
    }
    Ok(Run::events(events))
}

#[cfg(test)]
mod tests {
    //! Goal: an effect turns each face-down permanent it names face up, all together and in the order the reference lists them, by its controller, and passes over a face-up permanent, a card lying face down in exile and a player; the event leaves the permanent face up, and the effect refuses another effect by name.
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, setup};
    use crate::fold::apply_event;
    use crate::object::{GameObject, ObjectRef};
    use crate::resolve::{Next, Resolving, Scope};
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);
    const FOE: PlayerId = PlayerId(1);

    /// Seat 0 controls the Source, a face-down Rat and a face-up Wolf; seat 1 controls a face-down Foe and a Cloaked one.
    fn game() -> GameState {
        let game = setup(
            vec![vec![], vec![]],
            vec![
                vec![
                    creature("Source", 1, 1),
                    creature("Rat", 2, 2),
                    creature("Wolf", 2, 2),
                ],
                vec![creature("Foe", 2, 2), creature("Cloaked", 2, 2)],
            ],
        );
        let mut game = GameState::from_setup(&game).unwrap();
        for (name, how) in [
            ("Rat", FaceDown::Morph),
            ("Foe", FaceDown::Manifest),
            ("Cloaked", FaceDown::Cloak),
        ] {
            let id = named(&game, name).id;
            game.objects.get_mut(&id).unwrap().face_down = Some(how);
        }
        game
    }

    fn named(game: &GameState, name: &str) -> ObjectRef {
        game.objects()
            .find(|o| o.whole_card().name == name)
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
            panic!("turning face up needs no answer");
        };
        assert!(bound.is_empty() && snapshots.is_empty());
        assert_eq!(next, Next::Sibling);
        Ok(events)
    }

    fn turned(game: &GameState, events: &[Event]) -> Vec<(String, PlayerId)> {
        events
            .iter()
            .map(|event| match event {
                Event::TurnedFaceUp { object, player, .. } => (
                    game.object(*object).unwrap().whole_card().name.clone(),
                    *player,
                ),
                other => panic!("unexpected {other:?}"),
            })
            .collect()
    }

    #[rstest]
    #[case::your_face_down_creature(r#"{"TurnFaceUp":{"what":{"Each":[{"ControlledBy":"You"},"FaceDown"]}}}"#, &[("Rat", ME)])]
    #[case::a_face_down_creature_of_an_opponent(
        r#"{"TurnFaceUp":{"what":{"Each":[{"ControlledBy":"Opponents"},"FaceDown"]}}}"#,
        &[("Foe", FOE), ("Cloaked", FOE)]
    )]
    #[case::only_the_face_down_ones(
        r#"{"TurnFaceUp":{"what":{"Each":[{"ControlledBy":"You"}]}}}"#,
        &[("Rat", ME)]
    )]
    #[case::a_face_up_creature(r#"{"TurnFaceUp":{"what":{"Each":[{"Named":"Wolf"}]}}}"#, &[])]
    #[case::a_player(r#"{"TurnFaceUp":{"what":"You"}}"#, &[])]
    fn turning_up_turns_the_face_down_permanents_it_names(
        #[case] json: &str,
        #[case] want: &[(&str, PlayerId)],
    ) {
        let game = game();
        let events = run_effect(&game, json).unwrap();
        let want: Vec<(String, PlayerId)> = want
            .iter()
            .map(|(name, who)| ((*name).into(), *who))
            .collect();
        assert_eq!(turned(&game, &events), want);
    }

    #[test]
    fn the_event_leaves_the_permanent_face_up() {
        let mut game = game();
        let rat = named(&game, "Rat").id;
        let events = run_effect(
            &game,
            r#"{"TurnFaceUp":{"what":{"Each":[{"ControlledBy":"You"},"FaceDown"]}}}"#,
        )
        .unwrap();
        for event in &events {
            apply_event(&mut game, event).unwrap();
        }
        assert_eq!(game.object(rat).unwrap().face_down(), None);
        // Nothing is left to turn.
        assert_eq!(
            run_effect(
                &game,
                r#"{"TurnFaceUp":{"what":{"Each":[{"ControlledBy":"You"},"FaceDown"]}}}"#
            )
            .unwrap(),
            []
        );
    }

    #[test]
    fn another_effect_is_refused_by_name() {
        assert_eq!(
            run_effect(&game(), r#"{"Tap":{"what":"This"}}"#).unwrap_err(),
            ResolveError::Invalid("the TurnFaceUp primitive was given another effect".into())
        );
    }
}
