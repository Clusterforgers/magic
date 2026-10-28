//! `Effect::Discard`: players discard cards of their choice (CR 701.9).

use mtg_cards::effect::Effect;

use mtg_core::ids::{ObjectId, PlayerId};

use super::wrong_effect;
use crate::choose::{Chooser, Picked, Target, pick_cards};
use crate::events::Event;
use crate::object::ObjectRef;
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::rng::GameRng;
use crate::wording;
use crate::zones::Zone;

/// Each player the effect names who is still in the game discards the number of cards from their hand, in turn
/// order. The player chooses which and is asked only when the hand holds more than they discard; a smaller hand is
/// discarded whole, and an empty one is no event. A random discard asks no one (`c6_eg_at_random`).
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Discard {
        who,
        count,
        random,
        bind,
    } = effect
    else {
        return Err(wrong_effect("Discard"));
    };
    let take = usize::try_from(cx.scope.amount(count)?.max(0)).unwrap_or(usize::MAX);
    let players: Vec<PlayerId> = cx
        .scope
        .players(who)?
        .into_iter()
        .filter(|player| cx.state().is_alive(*player))
        .collect();
    let (picks, chosen) = if *random {
        c6_eg_at_random(cx, &players, take)
    } else {
        let choosers: Vec<Chooser> = players
            .into_iter()
            .map(|player| Chooser {
                player,
                from: cx.state().zone(Zone::Hand(player)).to_vec(),
                take,
                text: wording::discard(take),
            })
            .collect();
        match pick_cards(cx, &choosers)? {
            Picked::Ask(decision) => return Ok(Run::Ask(decision)),
            Picked::Done(chosen) => (Vec::new(), chosen),
        }
    };
    // The cards as they become in their graveyards (CR 400.7), for a later step that counts them.
    let discarded: Vec<Target> = chosen
        .iter()
        .flat_map(|(_, cards)| cards)
        .filter_map(|id| cx.state().object(*id))
        .map(|object| {
            Target::Object(ObjectRef {
                id: object.id(),
                counter: object.counter() + 1,
            })
        })
        .collect();
    let events = picks
        .into_iter()
        .chain(chosen.into_iter().flat_map(|(player, cards)| {
            cards
                .into_iter()
                .map(move |object| Event::CardDiscarded { player, object })
        }))
        .collect();
    Ok(match bind {
        None => Run::events(events),
        Some(name) => Run::Done {
            events,
            bound: vec![(name.clone(), discarded)],
            snapshots: Vec::new(),
            next: Next::Sibling,
        },
    })
}

/// The cards each player discards at random (CR 701.9b): the hand is shuffled with the next unspent substream of the
/// game's random source, one substream per player with cards in hand, and the first cards are taken. Each pick is
/// logged as [`Event::ChosenAtRandom`] before the discards, so a replay spends the same substreams; it names only the
/// cards the discard then reveals, never the rest of the hand.
fn c6_eg_at_random(
    cx: &Cx<'_>,
    players: &[PlayerId],
    take: usize,
) -> (Vec<Event>, Vec<(PlayerId, Vec<ObjectId>)>) {
    let state = cx.state();
    let mut picks = Vec::new();
    let mut chosen = Vec::new();
    for player in players {
        let mut cards = state.zone(Zone::Hand(*player)).to_vec();
        if cards.is_empty() || take == 0 {
            continue;
        }
        let offset = u64::try_from(picks.len()).unwrap_or(u64::MAX);
        GameRng::substream(state.seed, state.rng_index.saturating_add(offset)).shuffle(&mut cards);
        cards.truncate(take);
        picks.push(Event::ChosenAtRandom {
            cards: cards.clone(),
        });
        chosen.push((*player, cards));
    }
    (picks, chosen)
}

#[cfg(test)]
mod tests {
    //! Goal: a discard takes the number of cards from each player it names in turn order, the player chooses and is asked only when the hand holds more than that, a smaller or empty hand is discarded without a question, the question carries the words of the card, and what cannot be played yet is refused by name.
    use mtg_core::ids::{ObjectId, PlayerId};
    use rstest::rstest;

    use super::*;
    use crate::choose::{CardsOptions, Choice};
    use crate::decision::{Decision, DecisionKind};
    use crate::fixtures::{creature, state};
    use crate::resolve::{Next, Resolving, Scope};
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);
    const FOE: PlayerId = PlayerId(1);

    /// Seat 0 holds A, B and C, seat 1 holds D and E; the Source is on the battlefield.
    fn game() -> GameState {
        state(
            vec![
                vec![
                    creature("A", 1, 1),
                    creature("B", 1, 1),
                    creature("C", 1, 1),
                ],
                vec![creature("D", 1, 1), creature("E", 1, 1)],
            ],
            vec![vec![creature("Source", 1, 1)], vec![]],
        )
    }

    fn effect(json: &str) -> Effect {
        serde_json::from_str(json).unwrap()
    }

    fn hand(game: &GameState, player: PlayerId) -> Vec<ObjectId> {
        game.zone(Zone::Hand(player)).to_vec()
    }

    fn run_with(
        game: &GameState,
        journal: Vec<(PlayerId, Choice)>,
        effect: &Effect,
    ) -> Result<Run, ResolveError> {
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving {
            journal,
            ..Resolving::begin(source.id())
        };
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        run(&cx, effect)
    }

    /// The discards a finished run proposes, as (who, which).
    fn discards(run: Run) -> Vec<(PlayerId, ObjectId)> {
        let Run::Done {
            events,
            bound,
            snapshots,
            next,
        } = run
        else {
            panic!("the effect is done");
        };
        assert!(bound.is_empty() && snapshots.is_empty() && next == Next::Sibling);
        events
            .into_iter()
            .map(|event| match event {
                Event::CardDiscarded { player, object } => (player, object),
                other => panic!("only discards were expected, got {other:?}"),
            })
            .collect()
    }

    fn asked(run: Run) -> Decision {
        let Run::Ask(decision) = run else {
            panic!("the effect asks");
        };
        decision
    }

    #[test]
    fn the_player_is_asked_for_the_cards_when_the_hand_holds_more_than_is_discarded() {
        let game = game();
        let decision = asked(run_with(&game, Vec::new(), &effect(r#"{"Discard":{}}"#)).unwrap());
        assert_eq!(
            decision,
            Decision {
                player: ME,
                kind: DecisionKind::ChooseCards(CardsOptions {
                    source: game.zone(Zone::Battlefield)[0],
                    from: hand(&game, ME),
                    min: 1,
                    max: 1,
                    text: "Discard a card.".into(),
                    can_cancel: false,
                }),
            }
        );
    }

    #[test]
    fn the_cards_discarded_are_bound_as_the_objects_they_become_in_the_graveyards() {
        let game = game();
        let run = run_with(
            &game,
            Vec::new(),
            &effect(r#"{"Discard":{"who":"EachPlayer","count":9,"as":"gone"}}"#),
        )
        .unwrap();
        let Run::Done { events, bound, .. } = run else {
            panic!("the effect is done");
        };
        let [(name, gone)] = bound.as_slice() else {
            panic!("one name is bound");
        };
        assert_eq!(name, "gone");
        let all: Vec<ObjectId> = [ME, FOE]
            .into_iter()
            .flat_map(|player| hand(&game, player))
            .collect();
        assert_eq!(events.len(), all.len());
        assert_eq!(gone.len(), all.len());
        for (target, id) in gone.iter().zip(&all) {
            let was = game.object(*id).unwrap().counter();
            assert_eq!(
                *target,
                Target::Object(ObjectRef {
                    id: *id,
                    counter: was + 1
                })
            );
        }
    }

    #[test]
    fn the_answer_decides_which_cards_go() {
        let game = game();
        let picked = hand(&game, ME)[1];
        let run = run_with(
            &game,
            vec![(ME, Choice::Cards(vec![picked]))],
            &effect(r#"{"Discard":{}}"#),
        )
        .unwrap();
        assert_eq!(discards(run), [(ME, picked)]);
    }

    #[rstest]
    #[case::the_target_of_the_ability(r#"{"Discard":{"who":"Opponents"}}"#, FOE, "Discard a card.")]
    #[case::two_cards(r#"{"Discard":{"count":2}}"#, ME, "Discard 2 cards.")]
    fn any_seat_may_be_the_one_asked_and_the_count_is_in_the_words(
        #[case] json: &str,
        #[case] chooser: PlayerId,
        #[case] text: &str,
    ) {
        let game = game();
        let decision = asked(run_with(&game, Vec::new(), &effect(json)).unwrap());
        assert_eq!(decision.player, chooser);
        let DecisionKind::ChooseCards(options) = decision.kind else {
            panic!("cards are chosen");
        };
        assert_eq!(options.from, hand(&game, chooser));
        assert_eq!(options.text, text);
    }

    #[rstest]
    #[case::the_whole_hand_when_it_is_no_larger(
        r#"{"Discard":{"who":"Opponents","count":2}}"#,
        FOE
    )]
    #[case::the_whole_hand_when_it_is_smaller(r#"{"Discard":{"who":"Opponents","count":5}}"#, FOE)]
    fn a_hand_no_larger_than_the_count_is_discarded_without_a_question(
        #[case] json: &str,
        #[case] player: PlayerId,
    ) {
        let game = game();
        let run = run_with(&game, Vec::new(), &effect(json)).unwrap();
        let all = hand(&game, player);
        assert_eq!(
            discards(run),
            all.into_iter().map(|c| (player, c)).collect::<Vec<_>>()
        );
    }

    #[rstest]
    #[case::none_to_discard(r#"{"Discard":{"count":0}}"#)]
    #[case::a_negative_count(r#"{"Discard":{"count":-2}}"#)]
    fn discarding_nothing_asks_nothing_and_happens_nothing(#[case] json: &str) {
        let game = game();
        let run = run_with(&game, Vec::new(), &effect(json)).unwrap();
        assert_eq!(discards(run), []);
    }

    #[test]
    fn an_empty_hand_has_nothing_to_discard_and_nothing_is_asked() {
        let game = state(
            vec![vec![], vec![creature("D", 1, 1)]],
            vec![vec![creature("Source", 1, 1)], vec![]],
        );
        let run = run_with(&game, Vec::new(), &effect(r#"{"Discard":{}}"#)).unwrap();
        assert_eq!(discards(run), []);
    }

    #[test]
    fn each_player_is_asked_in_turn_order_and_the_discards_follow_together() {
        let game = game();
        let each = effect(r#"{"Discard":{"who":"EachPlayer"}}"#);
        // Seat 0 is asked first; seat 1 holds two cards, so seat 1 is asked next.
        let first = asked(run_with(&game, Vec::new(), &each).unwrap());
        assert_eq!(first.player, ME);
        let mine = hand(&game, ME)[2];
        let second = asked(run_with(&game, vec![(ME, Choice::Cards(vec![mine]))], &each).unwrap());
        assert_eq!(second.player, FOE);
        let theirs = hand(&game, FOE)[0];
        let run = run_with(
            &game,
            vec![
                (ME, Choice::Cards(vec![mine])),
                (FOE, Choice::Cards(vec![theirs])),
            ],
            &each,
        )
        .unwrap();
        assert_eq!(discards(run), [(ME, mine), (FOE, theirs)]);
    }

    #[test]
    fn a_player_who_has_left_the_game_discards_nothing() {
        let mut game = game();
        crate::fold::apply_event(
            &mut game,
            &Event::PlayerLost {
                player: FOE,
                reason: crate::player::LossReason::Conceded,
            },
        )
        .unwrap();
        let run = run_with(
            &game,
            Vec::new(),
            &effect(r#"{"Discard":{"who":"Opponents","count":2}}"#),
        )
        .unwrap();
        assert_eq!(discards(run), []);
    }

    #[test]
    fn an_answer_that_names_a_card_outside_the_hand_is_a_bug() {
        let game = game();
        let foreign = hand(&game, FOE)[0];
        let refused = run_with(
            &game,
            vec![(ME, Choice::Cards(vec![foreign]))],
            &effect(r#"{"Discard":{}}"#),
        );
        assert!(
            matches!(refused, Err(ResolveError::Invalid(_))),
            "{refused:?}"
        );
    }

    #[test]
    fn another_effect_is_a_bug_in_the_dispatch() {
        let game = game();
        let refused = run_with(&game, Vec::new(), &effect(r#"{"Draw":{}}"#));
        assert!(matches!(refused, Err(ResolveError::Invalid(_))));
    }
}

#[cfg(test)]
mod c6_eg_random_tests {
    //! Goal: a discard at random asks no one, takes the count from each player it names with one substream per player with cards in hand, discards a smaller hand whole, logs each pick before the discards and names only the cards discarded, and replays to the same state hash at 2 to 4 seats.
    use mtg_core::ids::{ObjectId, PlayerId};
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, setup};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;

    /// Every seat holds `held` cards; seat 0 has the Source on the battlefield.
    fn c6_eg_game(seed: u64, seats: usize, held: usize) -> GameState {
        let hands = (0..seats)
            .map(|seat| {
                (0..held)
                    .map(|card| creature(&format!("C{seat}.{card}"), 1, 1))
                    .collect()
            })
            .collect();
        let mut battlefield = vec![vec![creature("Source", 1, 1)]];
        battlefield.extend((1..seats).map(|_| Vec::new()));
        let mut table = setup(hands, battlefield);
        table.seed = seed;
        GameState::from_setup(&table).unwrap()
    }

    fn c6_eg_run(game: &GameState, json: &str) -> Vec<Event> {
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(game, PlayerId(0), source.reference()),
            resolving: &resolving,
        };
        let effect: Effect = serde_json::from_str(json).unwrap();
        let Run::Done { events, .. } = run(&cx, &effect).unwrap() else {
            panic!("a discard at random asks no one");
        };
        events
    }

    fn c6_eg_fold(game: &GameState, events: &[Event]) -> GameState {
        let mut after = game.clone();
        for event in events {
            crate::fold::apply_event(&mut after, event).unwrap();
        }
        after
    }

    const C6_EG_EACH: &str = r#"{"Discard":{"who":"EachPlayer","count":2,"random":true}}"#;

    #[rstest]
    #[case::two_seats(2)]
    #[case::three_seats(3)]
    #[case::four_seats(4)]
    fn c6_eg_each_player_discards_at_random_and_the_replay_is_the_same(#[case] seats: usize) {
        let game = c6_eg_game(7, seats, 4);
        let events = c6_eg_run(&game, C6_EG_EACH);
        assert_eq!(events.len(), seats * 3, "a pick and two discards per seat");
        let (picks, discards) = events.split_at(seats);
        for (seat, chunk) in discards.chunks(2).enumerate() {
            let player = PlayerId::from_index(seat);
            let Event::ChosenAtRandom { cards } = &picks[seat] else {
                panic!("the picks come first: {picks:?}");
            };
            let discarded: Vec<ObjectId> = chunk
                .iter()
                .map(|event| match event {
                    Event::CardDiscarded {
                        player: who,
                        object,
                    } if *who == player => *object,
                    other => panic!("a discard of seat {seat}: {other:?}"),
                })
                .collect();
            assert_eq!(cards, &discarded, "the pick names only the cards discarded");
            assert!(
                discarded
                    .iter()
                    .all(|id| game.zone(Zone::Hand(player)).contains(id))
            );
        }
        let after = c6_eg_fold(&game, &events);
        assert_eq!(after.rng_index(), game.rng_index() + seats as u64);
        let replayed = c6_eg_fold(&game, &c6_eg_run(&game, C6_EG_EACH));
        assert_eq!(after.state_hash(), replayed.state_hash());
        for seat in 0..seats {
            assert_eq!(after.zone(Zone::Hand(PlayerId::from_index(seat))).len(), 2);
        }
    }

    #[test]
    fn c6_eg_the_seed_decides_which_cards_go() {
        let json = r#"{"Discard":{"random":true}}"#;
        let first = |seed| c6_eg_run(&c6_eg_game(seed, 2, 4), json);
        let differ = (1..32).any(|seed| first(seed)[1] != first(0)[1]);
        assert!(differ, "another seed discards another card for some seed");
    }

    #[rstest]
    #[case::smaller_hand(2, 3, 2)]
    #[case::empty_hand(0, 1, 0)]
    #[case::nothing_asked(3, 0, 0)]
    fn c6_eg_a_short_hand_goes_whole_and_an_empty_one_spends_nothing(
        #[case] held: usize,
        #[case] count: usize,
        #[case] gone: usize,
    ) {
        let json = format!(r#"{{"Discard":{{"count":{count},"random":true}}}}"#);
        let game = c6_eg_game(3, 2, held);
        let events = c6_eg_run(&game, &json);
        let discards = events
            .iter()
            .filter(|event| matches!(event, Event::CardDiscarded { .. }))
            .count();
        assert_eq!(discards, gone);
        let after = c6_eg_fold(&game, &events);
        assert_eq!(
            after.rng_index(),
            game.rng_index() + u64::from(discards > 0)
        );
    }
}
