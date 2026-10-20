//! `Effect::LookAtTop`: the controller looks at the top cards of a library and binds them.

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::choose::Target;
use crate::events::Event;
use crate::library::{LibraryLooked, top_cards};
use crate::resolve::{Cx, Next, ResolveError, Run};

/// The controller looks at the top cards of the library of each player the effect names, in turn order, and the
/// cards are bound to the name. Each library looked at is one `LibraryLooked` (only the controller learns the cards;
/// the others see how many). A library shorter than the count is looked at whole and an empty one is no event;
/// the name is bound either way, to the cards that were seen.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::LookAtTop { who, count, bind } = effect else {
        return Err(wrong_effect("LookAtTop"));
    };
    let wanted = usize::try_from(cx.scope.amount(count)?.max(0)).unwrap_or(usize::MAX);
    let state = cx.state();
    let mut events = Vec::new();
    let mut seen = Vec::new();
    for owner in cx.scope.players(who)? {
        let cards = top_cards(state, owner, wanted);
        if cards.is_empty() {
            continue;
        }
        seen.extend(
            cards
                .iter()
                .filter_map(|id| state.object(*id))
                .map(|card| Target::Object(card.reference())),
        );
        events.push(Event::LibraryLooked(LibraryLooked {
            player: cx.controller(),
            cards,
        }));
    }
    Ok(Run::Done {
        events,
        bound: vec![(bind.clone(), seen)],
        snapshots: Vec::new(),
        next: Next::Sibling,
    })
}

#[cfg(test)]
mod tests {
    //! Goal: looking at the top of a library names the top cards, top first, to the controller alone as one event per library, binds exactly those cards to the name, takes what a short library has and nothing from an empty one, and follows the player the effect names rather than the one who looks.
    use mtg_core::ids::{ObjectId, PlayerId};
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, setup, state};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;
    use crate::zones::Zone;

    const ME: PlayerId = PlayerId(0);
    const FOE: PlayerId = PlayerId(1);

    /// Two seats with five cards in each library; the Source is on the battlefield.
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

    /// What a finished look proposes and binds, with the name it binds.
    fn looked(game: &GameState, json: &str) -> (Vec<Event>, Vec<(String, Vec<Target>)>) {
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
            panic!("looking asks nothing");
        };
        assert!(snapshots.is_empty() && next == Next::Sibling);
        (events, bound)
    }

    fn ids(targets: &[Target]) -> Vec<ObjectId> {
        targets
            .iter()
            .map(|target| target.object().unwrap().id)
            .collect()
    }

    #[rstest]
    #[case::one_card(r#"{"LookAtTop":{"as":"top"}}"#, 1)]
    #[case::three_cards(r#"{"LookAtTop":{"count":3,"as":"top"}}"#, 3)]
    #[case::more_than_the_library_holds(r#"{"LookAtTop":{"count":9,"as":"top"}}"#, 5)]
    fn the_controller_looks_at_the_top_cards_top_first_and_the_name_holds_them(
        #[case] json: &str,
        #[case] n: usize,
    ) {
        let game = game();
        let (events, bound) = looked(&game, json);
        assert_eq!(
            events,
            [Event::LibraryLooked(LibraryLooked {
                player: ME,
                cards: top(&game, ME, n)
            })]
        );
        assert_eq!(bound.len(), 1);
        assert_eq!(bound[0].0, "top");
        assert_eq!(ids(&bound[0].1), top(&game, ME, n));
    }

    #[test]
    fn the_library_is_the_one_the_effect_names_and_the_looker_is_the_controller() {
        let game = game();
        let (events, bound) = looked(
            &game,
            r#"{"LookAtTop":{"who":"Opponents","count":2,"as":"top"}}"#,
        );
        assert_eq!(
            events,
            [Event::LibraryLooked(LibraryLooked {
                player: ME,
                cards: top(&game, FOE, 2)
            })]
        );
        assert_eq!(ids(&bound[0].1), top(&game, FOE, 2));
    }

    #[test]
    fn each_library_named_is_one_event_in_turn_order() {
        let game = game();
        let (events, bound) = looked(&game, r#"{"LookAtTop":{"who":"EachPlayer","as":"top"}}"#);
        assert_eq!(
            events,
            [ME, FOE].map(|owner| Event::LibraryLooked(LibraryLooked {
                player: ME,
                cards: top(&game, owner, 1)
            }))
        );
        assert_eq!(
            ids(&bound[0].1),
            [top(&game, ME, 1), top(&game, FOE, 1)].concat()
        );
    }

    #[rstest]
    #[case::none(r#"{"LookAtTop":{"count":0,"as":"top"}}"#)]
    #[case::a_negative_count(r#"{"LookAtTop":{"count":-1,"as":"top"}}"#)]
    fn looking_at_nothing_is_no_event_and_binds_an_empty_set(#[case] json: &str) {
        let (events, bound) = looked(&game(), json);
        assert_eq!(events, []);
        assert_eq!(bound, [("top".to_owned(), Vec::new())]);
    }

    #[test]
    fn an_empty_library_shows_nothing() {
        let mut empty = setup(
            vec![vec![], vec![]],
            vec![vec![creature("Source", 1, 1)], vec![]],
        );
        empty.players[0].library.clear();
        let game = GameState::from_setup(&empty).unwrap();
        let (events, bound) = looked(&game, r#"{"LookAtTop":{"count":2,"as":"top"}}"#);
        assert_eq!(events, []);
        assert_eq!(bound, [("top".to_owned(), Vec::new())]);
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
