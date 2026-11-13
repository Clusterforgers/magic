//! `Effect::FaceDownPile`: objects and the top cards of the controller's library are exiled in a face-down pile, the pile
//! is shuffled, and its cards are manifested, cloaked or put back on top of the library.

use mtg_cards::effect::{Effect, PileFate};
use mtg_core::ids::{ObjectId, PlayerId};

use super::wrong_effect;
use crate::events::{Event, ZoneCause};
use crate::library::top_cards;
use crate::mulligan::shuffled;
use crate::object::{GameObject, ObjectKind};
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::state::GameState;
use crate::zones::Zone;

/// The named objects still where they were named, or one zone change on for a card the triggering event moved (an ability, the resolving spell and a card already in exile are left
/// out) and the top `top` cards of the controller's library each move to exile face down, seen by no player; a
/// `ChosenAtRandom` logs the shuffled order, spending a substream of the generator. Each card then moves in that order:
/// onto the battlefield face down under the controller's control (manifested or cloaked, an `Entered` after it when it
/// enters tapped or under another player's control) or on top of its owner's library. With `whole`, a named object that
/// is gone or a library short of `top` cards makes the effect do nothing.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::FaceDownPile {
        what,
        top,
        then,
        whole,
    } = effect
    else {
        return Err(wrong_effect("FaceDownPile"));
    };
    let state = cx.state();
    let player = cx.controller();
    let named = cx.scope.objects(what)?.len();
    // A card that left the battlefield by the triggering event is followed to where it went ("exile it" after it died).
    let live: Vec<ObjectId> = cx
        .scope
        .live_objects(what)?
        .into_iter()
        .chain(cx.scope.followed_cards(what))
        .filter(|object| {
            let resolving_spell =
                object.zone() == Zone::Stack && object.id() == cx.resolving.object();
            object.kind() != ObjectKind::Ability && !resolving_spell && object.zone() != Zone::Exile
        })
        .map(GameObject::id)
        .collect();
    let wanted = usize::try_from(cx.scope.amount(top)?.max(0)).unwrap_or(usize::MAX);
    let from_library = if state.is_alive(player) {
        top_cards(state, player, wanted)
    } else {
        Vec::new()
    };
    let done = |events| {
        Ok(Run::Done {
            events,
            bound: Vec::new(),
            snapshots: Vec::new(),
            next: Next::Sibling,
        })
    };
    if *whole && (live.len() < named || from_library.len() < wanted) {
        return done(Vec::new());
    }
    let mut events = Vec::new();
    let mut pile = Vec::new();
    for id in live.into_iter().chain(from_library) {
        let Some(object) = state.object(id) else {
            continue;
        };
        if pile.contains(&id) {
            continue;
        }
        events.push(Event::ZoneChanged {
            object: id,
            from: object.zone(),
            to: Zone::Exile,
            cause: ZoneCause::Sealed {
                by: cx.scope.source,
            },
        });
        pile.push(id);
    }
    if pile.is_empty() {
        return done(events);
    }
    let order = shuffled(state, 0, pile);
    events.push(Event::ChosenAtRandom {
        cards: order.clone(),
    });
    events.extend(r5509_place(state, &order, *then, player));
    done(events)
}

/// The moves that place the shuffled pile: onto the battlefield face down under `player`'s control (an `Entered` after a
/// card that enters tapped or under another player's control) or on top of each card's owner's library.
fn r5509_place(
    state: &GameState,
    order: &[ObjectId],
    then: PileFate,
    player: PlayerId,
) -> Vec<Event> {
    let mut events = Vec::new();
    for id in order.iter().copied() {
        let Some(object) = state.object(id) else {
            continue;
        };
        let owner = object.owner();
        match then {
            PileFate::OnTop => events.push(Event::ZoneChanged {
                object: id,
                from: Zone::Exile,
                to: Zone::Library(owner),
                cause: ZoneCause::Moved,
            }),
            PileFate::Manifest | PileFate::Cloak { .. } => {
                let tapped = matches!(then, PileFate::Cloak { tapped: true });
                events.push(Event::ZoneChanged {
                    object: id,
                    from: Zone::Exile,
                    to: Zone::Battlefield,
                    cause: if matches!(then, PileFate::Manifest) {
                        ZoneCause::Manifested { by: player }
                    } else {
                        ZoneCause::Cloaked { by: player }
                    },
                });
                if tapped || owner != player {
                    events.push(Event::Entered {
                        object: id,
                        tapped,
                        controller: player,
                        chosen: None,
                    });
                }
            }
        }
    }
    events
}

#[cfg(test)]
mod tests {
    //! Goal: a face-down pile exiles the named object and the top cards of the controller's library face down, logs the
    //! shuffle, then manifests or cloaks each card (tapped when asked) or puts it back on top of the library, and with
    //! `whole` does nothing when the library is short; at two to four seats.
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, state};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);

    fn game(seats: usize) -> GameState {
        let mut battlefields = vec![vec![creature("Source", 1, 1)]];
        battlefields.resize(seats, vec![]);
        state(vec![vec![]; seats], battlefields)
    }

    fn r5509_run(game: &GameState, json: &str) -> Vec<Event> {
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        let effect: Effect = serde_json::from_str(json).unwrap();
        let Run::Done { events, next, .. } = run(&cx, &effect).unwrap() else {
            panic!("a pile asks nothing");
        };
        assert_eq!(next, Next::Sibling);
        events
    }

    fn r5509_moves(events: &[Event], to: Zone) -> Vec<(ObjectId, ZoneCause)> {
        events
            .iter()
            .filter_map(|event| match event {
                Event::ZoneChanged {
                    object,
                    to: there,
                    cause,
                    ..
                } if *there == to => Some((*object, *cause)),
                _ => None,
            })
            .collect()
    }

    #[rstest]
    #[case::manifest_two_seats(2, r#""Manifest""#, Zone::Battlefield)]
    #[case::manifest_four_seats(4, r#""Manifest""#, Zone::Battlefield)]
    #[case::cloak_three_seats(3, r#"{"Cloak":{}}"#, Zone::Battlefield)]
    #[case::on_top_two_seats(2, r#""OnTop""#, Zone::Library(ME))]
    #[case::on_top_four_seats(4, r#""OnTop""#, Zone::Library(ME))]
    fn r5509_the_pile_is_exiled_shuffled_and_then_placed(
        #[case] seats: usize,
        #[case] then: &str,
        #[case] lands: Zone,
    ) {
        let game = game(seats);
        let source = game.zone(Zone::Battlefield)[0];
        let top: Vec<ObjectId> = game
            .zone(Zone::Library(ME))
            .iter()
            .rev()
            .take(2)
            .copied()
            .collect();
        let events = r5509_run(
            &game,
            &format!(r#"{{"FaceDownPile":{{"what":"This","top":2,"then":{then}}}}}"#),
        );
        let exiled = r5509_moves(&events, Zone::Exile);
        assert_eq!(exiled.len(), 3);
        assert!(
            exiled
                .iter()
                .all(|(_, cause)| matches!(cause, ZoneCause::Sealed { .. }))
        );
        let shuffled: Vec<ObjectId> = events
            .iter()
            .find_map(|event| match event {
                Event::ChosenAtRandom { cards } => Some(cards.clone()),
                _ => None,
            })
            .unwrap();
        let mut pile = vec![source];
        pile.extend(&top);
        let mut sorted = shuffled.clone();
        sorted.sort();
        pile.sort();
        assert_eq!(sorted, pile, "the whole pile is shuffled");
        let placed: Vec<ObjectId> = r5509_moves(&events, lands)
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        assert_eq!(
            placed, shuffled,
            "the cards are placed in the shuffled order"
        );
        if lands == Zone::Battlefield {
            let cloak = then.contains("Cloak");
            assert!(
                r5509_moves(&events, Zone::Battlefield)
                    .iter()
                    .all(|(_, cause)| {
                        if cloak {
                            matches!(cause, ZoneCause::Cloaked { by: ME })
                        } else {
                            matches!(cause, ZoneCause::Manifested { by: ME })
                        }
                    })
            );
        }
    }

    #[rstest]
    #[case::two_seats(2)]
    #[case::three_seats(3)]
    #[case::four_seats(4)]
    fn r5509_a_cloaked_pile_enters_tapped(#[case] seats: usize) {
        let game = game(seats);
        let events = r5509_run(
            &game,
            r#"{"FaceDownPile":{"what":"This","top":1,"then":{"Cloak":{"tapped":true}}}}"#,
        );
        let tapped = events
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    Event::Entered {
                        tapped: true,
                        controller: ME,
                        ..
                    }
                )
            })
            .count();
        assert_eq!(tapped, 2);
    }

    #[rstest]
    #[case::short_two_seats(2, 50, true, 0)]
    #[case::short_four_seats(4, 50, true, 0)]
    #[case::short_but_not_whole(3, 50, false, 1)]
    fn r5509_a_whole_pile_needs_every_card(
        #[case] seats: usize,
        #[case] top: u32,
        #[case] whole: bool,
        #[case] at_least: usize,
    ) {
        let game = game(seats);
        let events = r5509_run(
            &game,
            &format!(
                r#"{{"FaceDownPile":{{"what":"This","top":{top},"then":"OnTop","whole":{whole}}}}}"#
            ),
        );
        assert_eq!(events.is_empty(), at_least == 0);
        assert!(r5509_moves(&events, Zone::Exile).len() >= at_least);
    }
}
