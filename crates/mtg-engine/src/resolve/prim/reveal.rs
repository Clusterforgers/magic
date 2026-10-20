//! `Effect::Reveal`: objects are revealed to every player (CR 701.20).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::events::Event;
use crate::library::Revealed;
use crate::object::ObjectKind;
use crate::resolve::{Cx, ResolveError, Run};

/// The controller reveals the cards the reference names that are still the objects it named, all in one `Revealed`
/// that every seat sees. Nothing to reveal is no event.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Reveal { what } = effect else {
        return Err(wrong_effect("Reveal"));
    };
    let state = cx.state();
    let mut cards = Vec::new();
    for named in cx.scope.objects(what)? {
        let here = state.object(named.id).is_some_and(|object| {
            object.counter() == named.counter && object.kind() != ObjectKind::Ability
        });
        if here && !cards.contains(&named.id) {
            cards.push(named.id);
        }
    }
    Ok(Run::events(if cards.is_empty() {
        Vec::new()
    } else {
        vec![Event::Revealed(Revealed {
            player: cx.controller(),
            cards,
        })]
    }))
}

#[cfg(test)]
mod tests {
    //! Goal: a reveal names the cards the reference stands for, once each, in one event by the controller that every seat may see, leaves out an object that has since become another one, and is no event when there is nothing to reveal.
    use mtg_core::ids::PlayerId;

    use super::*;
    use crate::fixtures::{creature, state};
    use crate::resolve::{Next, Resolving, Scope};
    use crate::state::GameState;
    use crate::zones::Zone;

    const ME: PlayerId = PlayerId(0);

    /// Seat 0 holds A and B; the Source is on the battlefield.
    fn game() -> GameState {
        state(
            vec![vec![creature("A", 1, 1), creature("B", 1, 1)], vec![]],
            vec![vec![creature("Source", 1, 1)], vec![]],
        )
    }

    fn revealed(game: &GameState, json: &str) -> Vec<Event> {
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        let effect: Effect = serde_json::from_str(json).unwrap();
        let Run::Done {
            events,
            bound,
            snapshots,
            next,
        } = run(&cx, &effect).unwrap()
        else {
            panic!("revealing asks nothing");
        };
        assert!(bound.is_empty() && snapshots.is_empty() && next == Next::Sibling);
        events
    }

    #[test]
    fn the_cards_named_are_revealed_once_each_by_the_controller() {
        let game = game();
        let hand = game.zone(Zone::Hand(ME)).to_vec();
        let events = revealed(
            &game,
            r#"{"Reveal":{"what":{"Each":[{"In":"Hand"},{"OwnedBy":"You"}]}}}"#,
        );
        assert_eq!(
            events,
            [Event::Revealed(Revealed {
                player: ME,
                cards: hand
            })]
        );
    }

    #[test]
    fn the_source_itself_can_be_revealed() {
        let game = game();
        let source = game.zone(Zone::Battlefield)[0];
        let events = revealed(&game, r#"{"Reveal":{"what":"This"}}"#);
        assert_eq!(
            events,
            [Event::Revealed(Revealed {
                player: ME,
                cards: vec![source]
            })]
        );
    }

    #[test]
    fn nothing_to_reveal_is_no_event() {
        let events = revealed(
            &game(),
            r#"{"Reveal":{"what":{"Each":[{"In":"Hand"},{"OwnedBy":"Opponents"}]}}}"#,
        );
        assert_eq!(events, []);
    }

    #[test]
    fn an_object_that_changed_zones_since_it_was_named_is_not_revealed() {
        let mut game = game();
        let card = game.zone(Zone::Hand(ME))[0];
        let named = game.object(card).unwrap().reference();
        crate::fold::apply_event(
            &mut game,
            &Event::CardDiscarded {
                player: ME,
                object: card,
            },
        )
        .unwrap();
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let mut resolving = Resolving::begin(source.id());
        resolving
            .bindings
            .insert("seen".into(), vec![crate::choose::Target::Object(named)]);
        let cx = Cx {
            scope: Scope {
                bindings: &resolving.bindings,
                ..Scope::announcing(&game, ME, source.reference())
            },
            resolving: &resolving,
        };
        let effect: Effect =
            serde_json::from_str(r#"{"Reveal":{"what":{"Bound":"seen"}}}"#).unwrap();
        let Run::Done { events, .. } = run(&cx, &effect).unwrap() else {
            panic!("revealing asks nothing");
        };
        assert_eq!(events, []);
    }

    #[test]
    fn another_effect_is_a_bug_in_the_dispatch() {
        let game = game();
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(&game, ME, source.reference()),
            resolving: &resolving,
        };
        let effect: Effect = serde_json::from_str(r#"{"Draw":{}}"#).unwrap();
        assert!(matches!(run(&cx, &effect), Err(ResolveError::Invalid(_))));
    }
}
