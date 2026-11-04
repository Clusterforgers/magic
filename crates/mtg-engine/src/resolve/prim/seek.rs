//! `Effect::Seek`: a player seeks random cards of their library that match, into their hand (Alchemy).

use mtg_cards::effect::Effect;
use mtg_core::ids::ObjectId;

use super::wrong_effect;
use crate::events::{Event, ZoneCause};
use crate::resolve::{Cx, ResolveError, Run};
use crate::rng::GameRng;
use crate::zones::Zone;

/// Each player named who is still in the game picks `count` of the cards of their library that match the filter, fewer
/// when fewer match, with the next substream of the game's random source (one for each player who finds a card), and
/// puts them into their hand. The pick is `Sought` followed by the moves, so a replay from the log folds to the same
/// game and spends the substream. Nothing matching is no event and spends no substream.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Seek { who, filter, count } = effect else {
        return Err(wrong_effect("Seek"));
    };
    let state = cx.state();
    let wanted = usize::try_from(cx.scope.amount(count)?.max(0)).unwrap_or(usize::MAX);
    let mut events = Vec::new();
    let mut spent = 0;
    for player in cx.scope.players(who)? {
        if !state.is_alive(player) || wanted == 0 {
            continue;
        }
        let library = Zone::Library(player);
        let mut matches: Vec<ObjectId> = Vec::new();
        for id in state.zone(library) {
            if let Some(card) = state.object(*id)
                && cx.scope.selects(filter, card.reference())?
            {
                matches.push(*id);
            }
        }
        if matches.is_empty() {
            continue;
        }
        // By id, which says nothing of where the cards lie in the library.
        matches.sort_unstable();
        GameRng::substream(state.seed, state.rng_index + spent).shuffle(&mut matches);
        spent += 1;
        matches.truncate(wanted);
        events.push(Event::Sought {
            player,
            count: u32::try_from(matches.len()).unwrap_or(u32::MAX),
        });
        events.extend(matches.into_iter().map(|object| Event::ZoneChanged {
            object,
            from: library,
            to: Zone::Hand(player),
            cause: ZoneCause::Moved,
        }));
    }
    Ok(Run::events(events))
}

#[cfg(test)]
mod tests {
    //! Goal: a seek moves the count asked of the matching cards of the library into the hand, picked with the game's seeded random source (the same cards for the same seed, every card for some seed), fewer when fewer match, nothing and no substream spent when none does or the library is empty; the log folds to the same state in a replay and spends the substream once for each player who finds.
    use std::collections::BTreeSet;
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_core::card::CardType;
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, setup};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);

    fn land(name: &str) -> Arc<CardDefinition> {
        Arc::new(CardDefinition::new(name).with_type(CardType::Land))
    }

    /// Both libraries hold lands L0 to L3 and creatures C0 and C1 in this order.
    fn game(seed: u64, lands: usize) -> GameState {
        let mut table = setup(
            vec![Vec::new(), Vec::new()],
            vec![vec![creature("Source", 1, 1)], Vec::new()],
        );
        table.seed = seed;
        for player in &mut table.players {
            player.library = (0..lands)
                .map(|n| land(&format!("L{n}")))
                .chain((0..2).map(|n| creature(&format!("C{n}"), 1, 1)))
                .collect();
        }
        GameState::from_setup(&table).unwrap()
    }

    fn events(game: &GameState, json: &str) -> Vec<Event> {
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        let effect: Effect = serde_json::from_str(json).unwrap();
        match run(&cx, &effect).unwrap() {
            Run::Done { events, .. } => events,
            _ => panic!("a seek asks nothing"),
        }
    }

    fn seek(filter: &str, extra: &str) -> String {
        format!(r#"{{"Seek":{{"filter":[{filter}]{extra}}}}}"#)
    }

    const LAND: &str = r#"{"Type":"Land"}"#;

    fn sought(game: &GameState, json: &str) -> Vec<String> {
        events(game, json)
            .iter()
            .filter_map(|event| match event {
                Event::ZoneChanged { object, to, .. } if *to == Zone::Hand(ME) => {
                    Some(game.object(*object).unwrap().card().name.clone())
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn the_same_seed_seeks_the_same_card_and_every_match_comes_for_some_seed() {
        let json = seek(LAND, "");
        let mut seen = BTreeSet::new();
        for seed in 0..64 {
            let first = sought(&game(seed, 4), &json);
            assert_eq!(first, sought(&game(seed, 4), &json), "seed {seed}");
            assert_eq!(first.len(), 1);
            seen.insert(first[0].clone());
        }
        assert_eq!(
            seen,
            ["L0", "L1", "L2", "L3"]
                .into_iter()
                .map(String::from)
                .collect()
        );
    }

    #[rstest]
    #[case::one(1, 1)]
    #[case::two(2, 2)]
    #[case::all(4, 4)]
    #[case::more_than_match(9, 4)]
    #[case::none(0, 0)]
    fn the_count_asked_is_taken_without_repeats_and_only_of_matches(
        #[case] count: u32,
        #[case] expected: usize,
    ) {
        let found = sought(&game(3, 4), &seek(LAND, &format!(r#","count":{count}"#)));
        assert_eq!(found.len(), expected);
        assert_eq!(found.iter().collect::<BTreeSet<_>>().len(), expected);
        assert!(found.iter().all(|name| name.starts_with('L')));
    }

    #[rstest]
    #[case::no_match(game(5, 0), LAND)]
    #[case::empty_library(
        {
            let mut table = setup(vec![Vec::new(), Vec::new()], vec![vec![creature("Source", 1, 1)], Vec::new()]);
            table.players[0].library.clear();
            GameState::from_setup(&table).unwrap()
        },
        r#"{"Type":"Creature"}"#
    )]
    fn nothing_to_seek_is_no_event(#[case] game: GameState, #[case] filter: &str) {
        assert!(events(&game, &seek(filter, "")).is_empty());
    }

    #[test]
    fn a_replay_of_the_log_folds_to_the_same_state_and_spends_one_substream() {
        let start = game(11, 4);
        let made = events(&start, &seek(LAND, r#","count":2"#));
        assert!(matches!(made[0], Event::Sought { count: 2, .. }));
        let mut one = start.clone();
        let mut two = start.clone();
        for event in &made {
            crate::fold::apply_event(&mut one, event).unwrap();
            crate::fold::apply_event(&mut two, event).unwrap();
        }
        assert_eq!(one.state_hash(), two.state_hash());
        assert_ne!(one.state_hash(), start.state_hash());
        assert_eq!(one.rng_index(), start.rng_index() + 1);
        assert_eq!(one.zone(Zone::Hand(ME)).len(), 2);
        assert_eq!(one.zone(Zone::Library(ME)).len(), 4);
    }

    #[test]
    fn every_player_who_finds_spends_a_substream_of_their_own() {
        let start = game(2, 4);
        let made = events(
            &start,
            r#"{"Seek":{"who":"EachPlayer","filter":[{"Type":"Land"}]}}"#,
        );
        let found = made
            .iter()
            .filter(|event| matches!(event, Event::Sought { .. }))
            .count();
        assert_eq!(found, 2);
        let mut after = start.clone();
        for event in &made {
            crate::fold::apply_event(&mut after, event).unwrap();
        }
        assert_eq!(after.rng_index(), start.rng_index() + 2);
    }
}
