//! `Effect::RevealUntil`: cards are revealed from the top of a library until some of them match (CR 701.20).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::choose::Target;
use crate::events::Event;
use crate::library::Revealed;
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::zones::Zone;

/// Each player the effect names who is in the game reveals cards from the top of their library, one at a time, until
/// `count` of them match the filter or the library is empty; the cards revealed are one `Revealed` of that player, seen by
/// every seat, and stay in the library. The matching cards are bound to `as` and every card revealed, the matching ones
/// included, to `revealed_as`, top first. With several players each reveals from their own library, in turn order.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::RevealUntil {
        who,
        filter,
        count,
        bind,
        revealed_as,
    } = effect
    else {
        return Err(wrong_effect("RevealUntil"));
    };
    let wanted = usize::try_from(cx.scope.amount(count)?.max(0)).unwrap_or(usize::MAX);
    let state = cx.state();
    let mut events = Vec::new();
    let mut found = Vec::new();
    let mut revealed = Vec::new();
    for owner in cx.scope.players(who)? {
        if !state.is_alive(owner) {
            continue;
        }
        let mut cards = Vec::new();
        let mut matches = 0;
        for &id in state.zone(Zone::Library(owner)).iter().rev() {
            if matches >= wanted {
                break;
            }
            let Some(card) = state.object(id) else {
                continue;
            };
            let reference = card.reference();
            cards.push(id);
            revealed.push(Target::Object(reference));
            if cx.scope.selects(filter, reference)? {
                found.push(Target::Object(reference));
                matches += 1;
            }
        }
        if !cards.is_empty() {
            events.push(Event::Revealed(Revealed {
                player: owner,
                cards,
            }));
        }
    }
    Ok(Run::Done {
        events,
        bound: vec![(bind.clone(), found), (revealed_as.clone(), revealed)],
        snapshots: Vec::new(),
        next: Next::Sibling,
    })
}

#[cfg(test)]
mod tests {
    //! Goal: revealing until cards match names the cards from the top down to the last match, once for each library, to every seat in one event, binds the matches and everything revealed, stops early on the count, takes a short library whole and leaves the library as it was.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_core::card::CardType;
    use mtg_core::ids::{ObjectId, PlayerId};
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, setup};
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);
    const FOE: PlayerId = PlayerId(1);

    fn land() -> Arc<CardDefinition> {
        Arc::new(CardDefinition::new("Forest").with_type(CardType::Land))
    }

    /// Both libraries, top first: land, land, creature, land, creature, creature.
    fn game(library: usize) -> GameState {
        let mut game = setup(
            vec![vec![], vec![]],
            vec![vec![creature("Source", 1, 1)], vec![]],
        );
        for player in &mut game.players {
            let top_first = [
                land(),
                land(),
                creature("Bear", 2, 2),
                land(),
                creature("Elk", 2, 2),
                creature("Ox", 2, 2),
            ];
            player.library = top_first.into_iter().take(library).collect();
        }
        match GameState::from_setup(&game) {
            Ok(state) => state,
            Err(error) => panic!("the setup is valid: {error}"),
        }
    }

    fn top(game: &GameState, player: PlayerId) -> Vec<ObjectId> {
        game.zone(Zone::Library(player))
            .iter()
            .rev()
            .copied()
            .collect()
    }

    fn revealing(game: &GameState, json: &str) -> (Vec<Event>, Vec<(String, Vec<Target>)>) {
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = crate::resolve::Resolving::begin(source.id());
        let cx = Cx {
            scope: crate::resolve::Scope::announcing(game, ME, source.reference()),
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
        assert!(snapshots.is_empty() && next == Next::Sibling);
        (events, bound)
    }

    fn ids(targets: &[Target]) -> Vec<ObjectId> {
        targets
            .iter()
            .map(|target| target.object().unwrap().id)
            .collect()
    }

    const CREATURE: &str = r#""filter":[{"Type":"Creature"}]"#;

    #[rstest]
    #[case::the_first_creature(format!(r#"{{"RevealUntil":{{{CREATURE},"as":"f","revealed_as":"r"}}}}"#), 3, vec![2])]
    #[case::two_creatures(format!(r#"{{"RevealUntil":{{{CREATURE},"count":2,"as":"f","revealed_as":"r"}}}}"#), 5, vec![2, 4])]
    #[case::all_three(format!(r#"{{"RevealUntil":{{{CREATURE},"count":3,"as":"f","revealed_as":"r"}}}}"#), 6, vec![2, 4, 5])]
    #[case::more_than_the_library_holds(format!(r#"{{"RevealUntil":{{{CREATURE},"count":9,"as":"f","revealed_as":"r"}}}}"#), 6, vec![2, 4, 5])]
    #[case::a_land(r#"{"RevealUntil":{"filter":[{"Type":"Land"}],"as":"f","revealed_as":"r"}}"#.to_owned(), 1, vec![0])]
    #[case::nothing_matches(r#"{"RevealUntil":{"filter":[{"Type":"Artifact"}],"as":"f","revealed_as":"r"}}"#.to_owned(), 6, vec![])]
    #[case::none_wanted(format!(r#"{{"RevealUntil":{{{CREATURE},"count":0,"as":"f","revealed_as":"r"}}}}"#), 0, vec![])]
    fn the_cards_down_to_the_last_match_are_revealed_and_the_library_stays(
        #[case] json: String,
        #[case] revealed: usize,
        #[case] matches: Vec<usize>,
    ) {
        let game = game(6);
        let library = top(&game, ME);
        let (events, bound) = revealing(&game, &json);
        if revealed == 0 {
            assert_eq!(events, []);
        } else {
            assert_eq!(
                events,
                [Event::Revealed(Revealed {
                    player: ME,
                    cards: library[..revealed].to_vec()
                })]
            );
        }
        assert_eq!(
            ids(&bound[0].1),
            matches.iter().map(|n| library[*n]).collect::<Vec<_>>()
        );
        assert_eq!((bound[0].0.as_str(), bound[1].0.as_str()), ("f", "r"));
        assert_eq!(ids(&bound[1].1), library[..revealed].to_vec());
        assert_eq!(top(&game, ME), library, "the cards stay where they were");
    }

    #[test]
    fn each_library_named_is_one_event_in_turn_order() {
        let game = game(6);
        let (events, bound) = revealing(
            &game,
            &format!(
                r#"{{"RevealUntil":{{"who":"EachPlayer",{CREATURE},"as":"f","revealed_as":"r"}}}}"#
            ),
        );
        let mine = top(&game, ME);
        let theirs = top(&game, FOE);
        assert_eq!(
            events,
            [
                Event::Revealed(Revealed {
                    player: ME,
                    cards: mine[..3].to_vec()
                }),
                Event::Revealed(Revealed {
                    player: FOE,
                    cards: theirs[..3].to_vec()
                }),
            ]
        );
        assert_eq!(ids(&bound[0].1), [mine[2], theirs[2]]);
    }

    #[test]
    fn an_empty_library_reveals_nothing() {
        let (events, bound) = revealing(
            &game(0),
            &format!(r#"{{"RevealUntil":{{{CREATURE},"as":"f","revealed_as":"r"}}}}"#),
        );
        assert_eq!(events, []);
        assert_eq!(
            bound,
            [("f".to_owned(), Vec::new()), ("r".to_owned(), Vec::new())]
        );
    }

    #[test]
    fn another_effect_is_a_bug_in_the_dispatch() {
        let game = game(6);
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = crate::resolve::Resolving::begin(source.id());
        let cx = Cx {
            scope: crate::resolve::Scope::announcing(&game, ME, source.reference()),
            resolving: &resolving,
        };
        let effect: Effect = serde_json::from_str(r#"{"Draw":{}}"#).unwrap();
        assert!(matches!(run(&cx, &effect), Err(ResolveError::Invalid(_))));
    }
}
