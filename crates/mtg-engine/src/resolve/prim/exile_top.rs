//! `Effect::ExileTop`: cards are exiled from the top of a library, face up, and named for the steps after it (CR 406.3).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::choose::Target;
use crate::events::{Event, ZoneCause};
use crate::library::top_cards;
use crate::object::ObjectRef;
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::zones::Zone;

/// Each player the effect names who is in the game exiles `count` cards from the top of their library, the top card
/// first, face up (exile is open to every seat). A library shorter than the count is exiled whole and an empty one
/// exiles nothing. The cards are bound to the name as the objects they become in exile (CR 400.7), whether or not
/// anything was exiled.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::ExileTop { who, count, bind } = effect else {
        return Err(wrong_effect("ExileTop"));
    };
    let wanted = usize::try_from(cx.scope.amount(count)?.max(0)).unwrap_or(usize::MAX);
    let state = cx.state();
    let mut events = Vec::new();
    let mut exiled = Vec::new();
    for owner in cx.scope.players(who)? {
        if !state.is_alive(owner) {
            continue;
        }
        for id in top_cards(state, owner, wanted) {
            let Some(card) = state.object(id) else {
                continue;
            };
            events.push(Event::ZoneChanged {
                object: id,
                from: Zone::Library(owner),
                to: Zone::Exile,
                cause: ZoneCause::Exiled,
            });
            exiled.push(Target::Object(ObjectRef {
                id,
                counter: card.reference().counter + 1,
            }));
        }
    }
    Ok(Run::Done {
        events,
        bound: vec![(bind.clone(), exiled)],
        snapshots: Vec::new(),
        next: Next::Sibling,
    })
}

#[cfg(test)]
mod tests {
    //! Goal: exiling the top of a library exiles the top cards of each library the effect names, the top card first and face up, takes what a short library has and nothing from an empty one, binds the objects the cards become in exile (a count of nothing binds none), and names the library the effect names, not the controller's.
    use mtg_core::ids::{ObjectId, PlayerId};
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, state};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);
    const FOE: PlayerId = PlayerId(1);

    fn game() -> GameState {
        state(
            vec![vec![], vec![]],
            vec![vec![creature("Source", 1, 1)], vec![]],
        )
    }

    fn top(game: &GameState, player: PlayerId, n: usize) -> Vec<ObjectId> {
        game.zone(Zone::Library(player))
            .iter()
            .rev()
            .take(n)
            .copied()
            .collect()
    }

    fn exiled(game: &GameState, json: &str) -> (Vec<Event>, Vec<Target>) {
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        let effect: Effect = serde_json::from_str(json).unwrap();
        let Run::Done {
            events,
            mut bound,
            snapshots,
            next,
        } = run(&cx, &effect).unwrap()
        else {
            panic!("exiling asks nothing");
        };
        assert!(snapshots.is_empty() && next == Next::Sibling);
        assert_eq!(bound.len(), 1);
        assert_eq!(bound[0].0, "top");
        (events, bound.remove(0).1)
    }

    fn moves(events: &[Event], from: PlayerId) -> Vec<ObjectId> {
        events
            .iter()
            .map(|event| match event {
                Event::ZoneChanged {
                    object,
                    from: origin,
                    to: Zone::Exile,
                    cause: ZoneCause::Exiled,
                } if *origin == Zone::Library(from) => *object,
                other => panic!("not an exile from the library: {other:?}"),
            })
            .collect()
    }

    #[rstest]
    #[case::one_card(r#"{"ExileTop":{"as":"top"}}"#, 1)]
    #[case::two_cards(r#"{"ExileTop":{"count":2,"as":"top"}}"#, 2)]
    #[case::more_than_the_library_holds(r#"{"ExileTop":{"count":9,"as":"top"}}"#, 5)]
    #[case::a_count_of_nothing(r#"{"ExileTop":{"count":0,"as":"top"}}"#, 0)]
    fn the_top_cards_are_exiled_top_first_and_bound_as_the_objects_they_become(
        #[case] json: &str,
        #[case] n: usize,
    ) {
        let game = game();
        let (events, bound) = exiled(&game, json);
        assert_eq!(moves(&events, ME), top(&game, ME, n));
        let expected: Vec<Target> = top(&game, ME, n)
            .into_iter()
            .map(|id| {
                Target::Object(ObjectRef {
                    id,
                    counter: game.object(id).unwrap().counter() + 1,
                })
            })
            .collect();
        assert_eq!(bound, expected);
    }

    #[test]
    fn an_empty_library_exiles_nothing_and_binds_nothing() {
        let mut game = game();
        let library: Vec<ObjectId> = game.zone(Zone::Library(ME)).to_vec();
        for id in library {
            crate::fold::apply_event(
                &mut game,
                &Event::ZoneChanged {
                    object: id,
                    from: Zone::Library(ME),
                    to: Zone::Graveyard(ME),
                    cause: ZoneCause::Milled,
                },
            )
            .unwrap();
        }
        let (events, bound) = exiled(&game, r#"{"ExileTop":{"count":2,"as":"top"}}"#);
        assert!(events.is_empty() && bound.is_empty());
    }

    #[test]
    fn the_library_is_the_one_the_effect_names() {
        let game = game();
        let (events, bound) = exiled(
            &game,
            r#"{"ExileTop":{"who":"Opponents","count":2,"as":"top"}}"#,
        );
        assert_eq!(moves(&events, FOE), top(&game, FOE, 2));
        assert_eq!(bound.len(), 2);
    }
}
