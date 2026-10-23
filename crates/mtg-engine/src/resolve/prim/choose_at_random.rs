//! `Effect::ChooseAtRandom`: cards of a public zone are picked at random and the pick is bound.

use mtg_cards::effect::Effect;
use mtg_core::ids::ObjectId;

use super::wrong_effect;
use crate::choose::Target;
use crate::events::Event;
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::rng::GameRng;
use crate::zones::Zone;

/// Picks as many of the objects `from` names as `count` says, fewer when there are not so many, with the next
/// substream of the game's random source, and binds the pick as `as`. The pick is logged whole as
/// [`Event::ChosenAtRandom`], so a replay from the log gives the same game and the substream is spent. Nothing to pick
/// from is no event. The cards must lie in a graveyard, on the battlefield or in exile: every seat sees them, so every
/// seat may see which was picked; a pick among cards of a hand or a library would tell the seats that cannot see them
/// which one it was, and is refused.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::ChooseAtRandom { from, count, bind } = effect else {
        return Err(wrong_effect("ChooseAtRandom"));
    };
    let candidates = cx.scope.refs(from)?;
    let mut cards: Vec<ObjectId> = Vec::with_capacity(candidates.len());
    for target in &candidates {
        let Some(object) = target.object() else {
            return Err(ResolveError::Invalid(
                "a pick at random is among cards, not players".into(),
            ));
        };
        let public = cx
            .state()
            .object(object.id)
            .is_some_and(|found| !matches!(found.zone(), Zone::Hand(_) | Zone::Library(_)));
        if !public {
            return Err(ResolveError::Invalid(
                "a pick at random among hidden cards is not played".into(),
            ));
        }
        cards.push(object.id);
    }
    let wanted = usize::try_from(cx.scope.amount(count)?.max(0)).unwrap_or(usize::MAX);
    let picked = wanted.min(cards.len());
    if picked == 0 {
        return Ok(Run::Done {
            events: Vec::new(),
            bound: vec![(bind.clone(), Vec::new())],
            snapshots: Vec::new(),
            next: Next::Sibling,
        });
    }
    let state = cx.state();
    GameRng::substream(state.seed, state.rng_index).shuffle(&mut cards);
    cards.truncate(picked);
    let chosen: Vec<Target> = cards
        .iter()
        .filter_map(|id| {
            candidates
                .iter()
                .find(|target| target.object().is_some_and(|object| object.id == *id))
        })
        .copied()
        .collect();
    Ok(Run::Done {
        events: vec![Event::ChosenAtRandom { cards }],
        bound: vec![(bind.clone(), chosen)],
        snapshots: Vec::new(),
        next: Next::Sibling,
    })
}

#[cfg(test)]
mod tests {
    //! Goal: a pick at random takes the count asked of the cards named from the game's seeded random source, the same cards for the same seed and every card for some seed, fewer when there are not so many and nothing for an empty set (which is no event and spends no substream), binds the pick in the order it was drawn, and refuses cards that a seat cannot see.
    use std::collections::BTreeSet;

    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, setup};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);

    /// Seat 0 has the graveyard A, B, C and D; the other seats have one card each; the Source is on the battlefield.
    fn game(seed: u64, seats: usize) -> GameState {
        let mut hands = vec![vec![creature("Hand card", 1, 1)]];
        hands.extend((1..seats).map(|_| Vec::new()));
        let mut battlefield = vec![vec![creature("Source", 1, 1)]];
        battlefield.extend((1..seats).map(|_| Vec::new()));
        let mut table = setup(hands, battlefield);
        table.seed = seed;
        table.players[0].graveyard = ["A", "B", "C", "D"]
            .into_iter()
            .map(|name| creature(name, 1, 1))
            .collect();
        for (seat, player) in table.players.iter_mut().enumerate().skip(1) {
            player.graveyard = vec![creature(&format!("Foe {seat}"), 1, 1)];
        }
        GameState::from_setup(&table).unwrap()
    }

    fn effect(json: &str) -> Effect {
        serde_json::from_str(json).unwrap()
    }

    fn picks(game: &GameState, json: &str) -> Result<Run, ResolveError> {
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        run(&cx, &effect(json))
    }

    /// The names of the cards picked, which the event logs and the binding holds in the same order.
    fn names(game: &GameState, json: &str) -> Vec<String> {
        let Run::Done { events, bound, .. } = picks(game, json).unwrap() else {
            panic!("a pick at random asks nothing");
        };
        let [Event::ChosenAtRandom { cards }] = events.as_slice() else {
            panic!("one event for the pick: {events:?}");
        };
        let [(_, targets)] = bound.as_slice() else {
            panic!("one binding");
        };
        let bound_ids: Vec<ObjectId> = targets
            .iter()
            .filter_map(|target| target.object().map(|object| object.id))
            .collect();
        assert_eq!(
            &bound_ids, cards,
            "the binding is the log, in the order drawn"
        );
        cards
            .iter()
            .map(|id| game.object(*id).unwrap().card().name.clone())
            .collect()
    }

    const OWN_GRAVEYARD: &str = r#"{"Each":[{"In":"Graveyard"},{"OwnedBy":"You"}]}"#;

    fn pick_json(from: &str, count: &str) -> String {
        format!(r#"{{"ChooseAtRandom":{{"from":{from},"count":{count},"as":"card"}}}}"#)
    }

    #[test]
    fn the_same_seed_gives_the_same_card_and_every_card_comes_for_some_seed() {
        let json = pick_json(OWN_GRAVEYARD, "1");
        let mut seen = BTreeSet::new();
        for seed in 0..64 {
            let first = names(&game(seed, 2), &json);
            assert_eq!(first, names(&game(seed, 2), &json), "seed {seed}");
            assert_eq!(first.len(), 1);
            seen.insert(first[0].clone());
        }
        assert_eq!(
            seen,
            ["A", "B", "C", "D"].into_iter().map(String::from).collect()
        );
    }

    #[rstest]
    #[case::seed_0(0)]
    #[case::seed_1(1)]
    #[case::seed_2(2)]
    fn a_seed_picks_the_same_card_at_every_table_size(#[case] seed: u64) {
        let json = pick_json(OWN_GRAVEYARD, "1");
        let two = names(&game(seed, 2), &json);
        assert_eq!(two, names(&game(seed, 3), &json));
        assert_eq!(two, names(&game(seed, 4), &json));
    }

    #[rstest]
    #[case::one("1", 1)]
    #[case::two("2", 2)]
    #[case::all_four("4", 4)]
    #[case::more_than_there_are("9", 4)]
    fn the_count_asked_is_picked_without_repeats(#[case] count: &str, #[case] expected: usize) {
        let picked = names(&game(3, 2), &pick_json(OWN_GRAVEYARD, count));
        assert_eq!(picked.len(), expected);
        assert_eq!(picked.iter().collect::<BTreeSet<_>>().len(), expected);
    }

    #[rstest]
    #[case::a_count_of_none(OWN_GRAVEYARD, "0")]
    #[case::no_card_matches(
        r#"{"Each":[{"In":"Graveyard"},{"OwnedBy":"You"},{"Type":"Land"}]}"#,
        "1"
    )]
    fn nothing_to_pick_is_no_event(#[case] from: &str, #[case] count: &str) {
        let Run::Done { events, bound, .. } = picks(&game(5, 2), &pick_json(from, count)).unwrap()
        else {
            panic!("a pick at random asks nothing");
        };
        assert!(events.is_empty());
        assert!(bound.iter().all(|(_, targets)| targets.is_empty()));
    }

    #[test]
    fn the_event_spends_one_substream_and_the_next_pick_uses_the_next() {
        let json = pick_json(OWN_GRAVEYARD, "1");
        let game = game(5, 2);
        let Run::Done { events, .. } = picks(&game, &json).unwrap() else {
            panic!("a pick at random asks nothing");
        };
        let mut folded = game.clone();
        for event in &events {
            crate::fold::apply_event(&mut folded, event).unwrap();
        }
        assert_eq!(folded.rng_index(), game.rng_index() + 1);
        let differ = (0..32).any(|seed| {
            let game = self::game(seed, 2);
            let mut after = game.clone();
            after.rng_index += 1;
            names(&game, &json) != names(&after, &json)
        });
        assert!(differ, "another substream draws another card for some seed");
    }

    #[test]
    fn cards_on_the_battlefield_may_be_picked_and_cards_of_a_hand_or_library_may_not() {
        let game = game(1, 2);
        let battlefield = r#"{"Each":[{"In":"Battlefield"}]}"#;
        assert_eq!(names(&game, &pick_json(battlefield, "1")), ["Source"]);
        for hidden in [
            r#"{"Each":[{"In":"Hand"}]}"#,
            r#"{"Each":[{"In":"Library"}]}"#,
        ] {
            assert!(
                matches!(
                    picks(&game, &pick_json(hidden, "1")),
                    Err(ResolveError::Invalid(_))
                ),
                "{hidden}"
            );
        }
    }

    #[test]
    fn players_are_not_picked_at_random() {
        assert!(matches!(
            picks(&game(1, 2), &pick_json(r#""Opponents""#, "1")),
            Err(ResolveError::Invalid(_))
        ));
    }
}
