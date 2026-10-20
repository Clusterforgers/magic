//! `Effect::Move`: objects change zones, to a hand, a graveyard or a library.

use mtg_cards::effect::{Destination, Effect, Order, Place};
use mtg_core::ids::{ObjectId, PlayerId};

use super::{unsupported, wrong_effect};
use crate::choose::{Questions, Stop};
use crate::events::{Event, ZoneCause};
use crate::mulligan::shuffled;
use crate::object::{GameObject, ObjectKind};
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::snapshot::{Snapshot, snapshot};
use crate::state::GameState;
use crate::wording;
use crate::zones::Zone;

/// The objects the reference stands for that are still the ones it named, once each, in the order named. An ability
/// or a spell on the stack does not move by this effect.
fn movers<'a>(
    cx: &'a Cx<'_>,
    what: &mtg_cards::refs::Ref,
) -> Result<Vec<&'a GameObject>, ResolveError> {
    let state = cx.state();
    let mut found: Vec<&GameObject> = Vec::new();
    for named in cx.scope.objects(what)? {
        let object = state.object(named.id).filter(|object| {
            object.counter() == named.counter
                && object.kind() != ObjectKind::Ability
                && object.zone() != Zone::Stack
        });
        if let Some(object) = object
            && !found.iter().any(|seen| seen.id() == object.id())
        {
            found.push(object);
        }
    }
    Ok(found)
}

/// The move of one object into `to`, and what it was if it left the battlefield.
fn moved(state: &GameState, object: ObjectId, from: Zone, to: Zone) -> (Event, Option<Snapshot>) {
    let shot = (from == Zone::Battlefield)
        .then(|| snapshot(state, object))
        .flatten();
    let event = Event::ZoneChanged {
        object,
        from,
        to,
        cause: ZoneCause::Moved,
    };
    (event, shot)
}

/// Each object the reference names goes to its owner's hand or graveyard, or into its owner's library on top, on
/// the bottom or shuffled in. Cards put into one library together are put in the order the controller chooses ("in any
/// order"), asked as an `OrderCards` per library unless there is one card, and the first of them is nearest the top; a
/// shuffle needs no order.
/// An object already where it would go stays. Moves from the battlefield carry a snapshot of the object as it was.
/// A destination of the battlefield and a random order are not played yet; `Caps` keeps those cards out.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Move { what, to } = effect else {
        return Err(wrong_effect("Move"));
    };
    let state = cx.state();
    let objects = movers(cx, what)?;
    let mut events = Vec::new();
    let mut snapshots = Vec::new();
    match to {
        Destination::Hand | Destination::Graveyard => {
            for object in objects {
                let owner = object.owner();
                let there = if matches!(to, Destination::Hand) {
                    Zone::Hand(owner)
                } else {
                    Zone::Graveyard(owner)
                };
                if object.zone() != there {
                    let (event, shot) = moved(state, object.id(), object.zone(), there);
                    events.push(event);
                    snapshots.extend(shot);
                }
            }
        }
        Destination::Library {
            order: Some(Order::Random),
            place: Place::Top | Place::Bottom,
        } => return unsupported("Effect.Move.Random"),
        Destination::Library { place, .. } => {
            let mut owners: Vec<(PlayerId, Vec<&GameObject>)> = Vec::new();
            for object in objects {
                match owners
                    .iter_mut()
                    .find(|(owner, _)| *owner == object.owner())
                {
                    Some((_, group)) => group.push(object),
                    None => owners.push((object.owner(), vec![object])),
                }
            }
            return Stop::settle(into_libraries(cx, *place, &owners));
        }
        Destination::Battlefield { .. } => return unsupported("Effect.Move.Battlefield"),
    }
    Ok(Run::Done {
        events,
        bound: Vec::new(),
        snapshots,
        next: Next::Sibling,
    })
}

/// The events that put each owner's cards into their library at `place`.
fn into_libraries(
    cx: &Cx<'_>,
    place: Place,
    owners: &[(PlayerId, Vec<&GameObject>)],
) -> Result<Run, Stop> {
    let state = cx.state();
    let mut questions = Questions::new(cx);
    let mut events = Vec::new();
    let mut snapshots = Vec::new();
    let mut shuffles = 0;
    for (owner, group) in owners {
        let library = Zone::Library(*owner);
        let mut ids: Vec<ObjectId> = group.iter().map(|object| object.id()).collect();
        if place != Place::Shuffled && ids.len() > 1 {
            let who = cx.controller();
            ids = questions.order(who, ids, wording::library_order(place == Place::Bottom))?;
        }
        let zone_of = |id: ObjectId| {
            group
                .iter()
                .find(|object| object.id() == id)
                .map(|object| object.zone())
        };
        // The first card ends up nearest the top, so a move to the top puts it last; a card already in the library
        // stays for the bottom and for a shuffle, and goes to the top again for a move to the top.
        let putting: Vec<ObjectId> = if place == Place::Top {
            ids.iter().rev().copied().collect()
        } else {
            ids.clone()
        };
        for id in putting {
            if let Some(zone) = zone_of(id).filter(|zone| *zone != library || place == Place::Top) {
                let (event, shot) = moved(state, id, zone, library);
                events.push(event);
                snapshots.extend(shot);
            }
        }
        match place {
            Place::Top => {}
            Place::Bottom => events.push(Event::LibraryBottomed {
                player: *owner,
                cards: ids,
            }),
            Place::Shuffled => {
                let mut cards = state.zone(library).to_vec();
                cards.extend(ids.iter().filter(|id| zone_of(**id) != Some(library)));
                events.push(Event::LibraryShuffled {
                    player: *owner,
                    order: shuffled(state, shuffles, cards),
                });
                shuffles += 1;
            }
        }
    }
    questions.end()?;
    Ok(Run::Done {
        events,
        bound: Vec::new(),
        snapshots,
        next: Next::Sibling,
    })
}

#[cfg(test)]
mod tests {
    //! Goal: a move sends each object named to its owner's hand or graveyard or into its owner's library on top, on the bottom or shuffled in; cards put into one library together are put in the order their owner chooses, asked only when there are two or more, and the log leaves the library in exactly that order; an object already in place stays, a stale one or a spell on the stack is passed over, a move from the battlefield carries what it was, and a destination or order the engine does not play is refused by name.
    use rstest::rstest;

    use super::*;
    use crate::choose::{Choice, Target};
    use crate::decision::{Decision, DecisionKind};
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;
    use crate::resolve::{Resolving, Scope};

    const ME: PlayerId = PlayerId(0);
    const FOE: PlayerId = PlayerId(1);

    /// Seat 0 holds A, B and C, seat 1 holds D and E; the Source and a Wolf are on the battlefield of seat 0. Each library has five cards.
    fn game() -> GameState {
        let hand = |names: &[&str]| names.iter().map(|n| creature(n, 1, 1)).collect();
        state(
            vec![hand(&["A", "B", "C"]), hand(&["D", "E"])],
            vec![
                vec![creature("Source", 1, 1), creature("Wolf", 2, 2)],
                vec![],
            ],
        )
    }

    fn hand(game: &GameState, player: PlayerId) -> Vec<ObjectId> {
        game.zone(Zone::Hand(player)).to_vec()
    }

    fn library(game: &GameState, player: PlayerId) -> Vec<ObjectId> {
        game.zone(Zone::Library(player)).to_vec()
    }

    /// The effect that moves the cards bound to `cards` to `to`.
    fn moving(to: &str) -> Effect {
        let json = format!(r#"{{"Move":{{"what":{{"Bound":"cards"}},"to":{to}}}}}"#);
        serde_json::from_str(&json).unwrap()
    }

    const TOP: &str = r#"{"Library":{"place":"Top"}}"#;
    const BOTTOM: &str = r#"{"Library":{"place":"Bottom"}}"#;
    const SHUFFLED: &str = r#"{"Library":{"place":"Shuffled"}}"#;

    fn named(game: &GameState, cards: &[ObjectId]) -> Vec<Target> {
        cards
            .iter()
            .map(|id| Target::Object(game.object(*id).unwrap().reference()))
            .collect()
    }

    /// Runs the effect with `targets` bound to `cards` and `journal` as the answers given so far.
    fn run_bound(
        game: &GameState,
        targets: Vec<Target>,
        journal: Vec<(PlayerId, Choice)>,
        effect: &Effect,
    ) -> Result<Run, ResolveError> {
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let mut resolving = Resolving {
            journal,
            ..Resolving::begin(source.id())
        };
        resolving.bindings.insert("cards".into(), targets);
        let cx = Cx {
            scope: Scope {
                bindings: &resolving.bindings,
                ..Scope::announcing(game, ME, source.reference())
            },
            resolving: &resolving,
        };
        run(&cx, effect)
    }

    fn run_with(
        game: &GameState,
        cards: &[ObjectId],
        journal: Vec<(PlayerId, Choice)>,
        effect: &Effect,
    ) -> Result<Run, ResolveError> {
        run_bound(game, named(game, cards), journal, effect)
    }

    fn done(run: Run) -> (Vec<Event>, Vec<Snapshot>) {
        let Run::Done {
            events,
            bound,
            snapshots,
            next,
        } = run
        else {
            panic!("the move is done");
        };
        assert!(bound.is_empty() && next == Next::Sibling);
        (events, snapshots)
    }

    fn asked(run: Run) -> Decision {
        let Run::Ask(decision) = run else {
            panic!("the effect asks");
        };
        decision
    }

    fn to(object: ObjectId, from: Zone, there: Zone) -> Event {
        Event::ZoneChanged {
            object,
            from,
            to: there,
            cause: ZoneCause::Moved,
        }
    }

    /// The state after the events fold onto it.
    fn after(game: &GameState, events: &[Event]) -> GameState {
        let mut game = game.clone();
        for event in events {
            apply_event(&mut game, event).unwrap();
        }
        game
    }

    #[rstest]
    #[case::to_the_graveyard(r#""Graveyard""#)]
    #[case::the_hand_is_where_they_are(r#""Hand""#)]
    fn cards_go_to_the_hand_or_graveyard_of_their_owner(#[case] zone: &str) {
        let game = game();
        let cards = [hand(&game, ME)[0], hand(&game, FOE)[0]];
        let (events, snapshots) = done(run_with(&game, &cards, Vec::new(), &moving(zone)).unwrap());
        let expected: Vec<Event> = if zone == r#""Hand""# {
            Vec::new()
        } else {
            vec![
                to(cards[0], Zone::Hand(ME), Zone::Graveyard(ME)),
                to(cards[1], Zone::Hand(FOE), Zone::Graveyard(FOE)),
            ]
        };
        assert_eq!(events, expected);
        assert_eq!(snapshots, []);
    }

    #[test]
    fn a_permanent_leaves_the_battlefield_with_a_snapshot_of_what_it_was() {
        let game = game();
        let wolf = game.zone(Zone::Battlefield)[1];
        let (events, snapshots) =
            done(run_with(&game, &[wolf], Vec::new(), &moving(r#""Hand""#)).unwrap());
        assert_eq!(events, [to(wolf, Zone::Battlefield, Zone::Hand(ME))]);
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].id, wolf);
        assert_eq!(snapshots[0].zone, Zone::Battlefield);
    }

    #[test]
    fn one_card_goes_on_top_without_a_question() {
        let game = game();
        let card = hand(&game, ME)[1];
        let (events, _) = done(run_with(&game, &[card], Vec::new(), &moving(TOP)).unwrap());
        assert_eq!(events, [to(card, Zone::Hand(ME), Zone::Library(ME))]);
        let folded = after(&game, &events);
        assert_eq!(*folded.zone(Zone::Library(ME)).last().unwrap(), card);
    }

    #[test]
    fn several_cards_are_ordered_by_their_owner_and_the_first_ends_up_on_top() {
        let game = game();
        let cards = hand(&game, ME);
        let decision = asked(run_with(&game, &cards, Vec::new(), &moving(TOP)).unwrap());
        let DecisionKind::OrderCards(options) = decision.kind else {
            panic!("the cards are ordered");
        };
        assert_eq!(decision.player, ME);
        assert_eq!(options.cards, cards);
        assert_eq!(
            options.text,
            "Put the cards on top of your library in any order."
        );
        let chosen = vec![cards[2], cards[0], cards[1]];
        let journal = vec![(ME, Choice::Order(chosen.clone()))];
        let (events, _) = done(run_with(&game, &cards, journal, &moving(TOP)).unwrap());
        assert_eq!(events.len(), 3);
        let folded = after(&game, &events);
        let top: Vec<ObjectId> = folded
            .zone(Zone::Library(ME))
            .iter()
            .rev()
            .take(3)
            .copied()
            .collect();
        assert_eq!(top, chosen, "the first of the order is the top card");
        assert_eq!(folded.zone(Zone::Library(ME)).len(), 8);
    }

    #[test]
    fn several_cards_go_to_the_bottom_with_the_last_of_the_order_the_bottom_card() {
        let game = game();
        let cards = hand(&game, ME)[..2].to_vec();
        let decision = asked(run_with(&game, &cards, Vec::new(), &moving(BOTTOM)).unwrap());
        let DecisionKind::OrderCards(options) = decision.kind else {
            panic!("the cards are ordered");
        };
        assert_eq!(
            options.text,
            "Put the cards on the bottom of your library in any order."
        );
        let chosen = vec![cards[1], cards[0]];
        let journal = vec![(ME, Choice::Order(chosen.clone()))];
        let (events, _) = done(run_with(&game, &cards, journal, &moving(BOTTOM)).unwrap());
        assert_eq!(
            events.last(),
            Some(&Event::LibraryBottomed {
                player: ME,
                cards: chosen.clone()
            })
        );
        let low = library(&after(&game, &events), ME);
        assert_eq!(
            low[..2],
            [chosen[1], chosen[0]],
            "the last of the order is the bottom card"
        );
        assert_eq!(low.len(), 7);
        assert_eq!(low[2..], library(&game, ME)[..]);
    }

    #[test]
    fn a_card_of_the_library_goes_to_the_bottom_or_the_top_of_it_without_leaving() {
        let game = game();
        let top_card = *game.zone(Zone::Library(ME)).last().unwrap();
        let (events, _) = done(run_with(&game, &[top_card], Vec::new(), &moving(BOTTOM)).unwrap());
        assert_eq!(
            events,
            [Event::LibraryBottomed {
                player: ME,
                cards: vec![top_card]
            }]
        );
        assert_eq!(library(&after(&game, &events), ME)[0], top_card);
        let bottom_card = library(&game, ME)[0];
        let (events, _) = done(run_with(&game, &[bottom_card], Vec::new(), &moving(TOP)).unwrap());
        assert_eq!(
            events,
            [to(bottom_card, Zone::Library(ME), Zone::Library(ME))]
        );
        let folded = after(&game, &events);
        assert_eq!(*folded.zone(Zone::Library(ME)).last().unwrap(), bottom_card);
    }

    #[test]
    fn cards_shuffled_in_join_the_library_which_is_then_shuffled_without_an_order_asked() {
        let game = game();
        let cards = hand(&game, ME)[..2].to_vec();
        let (events, _) = done(run_with(&game, &cards, Vec::new(), &moving(SHUFFLED)).unwrap());
        assert_eq!(events.len(), 3);
        let Event::LibraryShuffled { player, order } = &events[2] else {
            panic!("the library is shuffled last");
        };
        assert_eq!(*player, ME);
        let mut want = [library(&game, ME), cards.clone()].concat();
        let mut got = order.clone();
        want.sort_unstable();
        got.sort_unstable();
        assert_eq!(got, want);
        let folded = after(&game, &events);
        assert_eq!(library(&folded, ME), *order);
        assert_eq!(folded.rng_index(), game.rng_index() + 1);
    }

    #[test]
    fn the_controller_is_asked_for_the_order_of_the_cards_of_each_library_in_turn() {
        let game = game();
        let cards = [hand(&game, ME), hand(&game, FOE)].concat();
        let first = asked(run_with(&game, &cards, Vec::new(), &moving(TOP)).unwrap());
        let DecisionKind::OrderCards(options) = &first.kind else {
            panic!("the cards are ordered");
        };
        assert_eq!((first.player, options.cards.as_slice()), (ME, &cards[..3]));
        let mine = vec![cards[1], cards[0], cards[2]];
        let journal = vec![(ME, Choice::Order(mine))];
        let second = asked(run_with(&game, &cards, journal.clone(), &moving(TOP)).unwrap());
        let DecisionKind::OrderCards(options) = &second.kind else {
            panic!("the cards are ordered");
        };
        assert_eq!((second.player, options.cards.as_slice()), (ME, &cards[3..]));
        let theirs = vec![cards[4], cards[3]];
        let journal = [journal, vec![(ME, Choice::Order(theirs))]].concat();
        let (events, _) = done(run_with(&game, &cards, journal, &moving(TOP)).unwrap());
        assert_eq!(events.len(), 5);
        let folded = after(&game, &events);
        assert_eq!(folded.zone(Zone::Library(FOE)).len(), 7);
        assert_eq!(*folded.zone(Zone::Library(FOE)).last().unwrap(), cards[4]);
    }

    #[test]
    fn what_is_stale_or_named_twice_is_passed_over_and_a_spell_on_the_stack_does_not_move() {
        let mut game = game();
        let card = hand(&game, ME)[0];
        let stale = named(&game, &[card]);
        apply_event(
            &mut game,
            &Event::CardDiscarded {
                player: ME,
                object: card,
            },
        )
        .unwrap();
        let other = hand(&game, ME)[0];
        let targets = [stale, named(&game, &[other, other])].concat();
        let (events, _) =
            done(run_bound(&game, targets, Vec::new(), &moving(r#""Graveyard""#)).unwrap());
        assert_eq!(events, [to(other, Zone::Hand(ME), Zone::Graveyard(ME))]);
        // A card on the stack is a spell, which this effect leaves alone.
        let mut cast = game.clone();
        apply_event(
            &mut cast,
            &Event::SpellCast {
                player: ME,
                object: other,
                choices: crate::choose::Choices::default(),
            },
        )
        .unwrap();
        let (events, _) = done(
            run_bound(
                &cast,
                named(&cast, &[other]),
                Vec::new(),
                &moving(r#""Hand""#),
            )
            .unwrap(),
        );
        assert_eq!(events, []);
    }

    #[rstest]
    #[case::the_battlefield(r#"{"Battlefield":{}}"#, "Effect.Move.Battlefield")]
    #[case::a_random_order_on_top(
        r#"{"Library":{"place":"Top","order":"Random"}}"#,
        "Effect.Move.Random"
    )]
    #[case::a_random_order_on_the_bottom(
        r#"{"Library":{"place":"Bottom","order":"Random"}}"#,
        "Effect.Move.Random"
    )]
    fn what_the_engine_does_not_play_is_refused_by_name(
        #[case] to: &str,
        #[case] name: &'static str,
    ) {
        let game = game();
        let card = hand(&game, ME)[0];
        let refused = run_with(&game, &[card], Vec::new(), &moving(to));
        assert_eq!(refused.unwrap_err(), ResolveError::Unsupported(name));
    }

    #[test]
    fn a_random_order_is_no_matter_when_the_library_is_shuffled_after() {
        let game = game();
        let card = hand(&game, ME)[0];
        let to = r#"{"Library":{"place":"Shuffled","order":"Random"}}"#;
        assert!(run_with(&game, &[card], Vec::new(), &moving(to)).is_ok());
    }

    #[test]
    fn an_answer_the_step_cannot_use_is_a_bug() {
        let game = game();
        let cards = hand(&game, ME);
        for journal in [
            vec![(ME, Choice::Order(cards[..2].to_vec()))],
            vec![(FOE, Choice::Order(cards.clone()))],
            vec![(ME, Choice::Cards(cards.clone()))],
            vec![
                (ME, Choice::Order(cards.clone())),
                (ME, Choice::Order(cards.clone())),
            ],
        ] {
            let refused = run_with(&game, &cards, journal.clone(), &moving(TOP));
            assert!(
                matches!(refused, Err(ResolveError::Invalid(_))),
                "{journal:?}: {refused:?}"
            );
        }
    }

    #[test]
    fn another_effect_is_a_bug_in_the_dispatch() {
        let game = game();
        let draw = serde_json::from_str(r#"{"Draw":{}}"#).unwrap();
        let refused = run_with(&game, &[], Vec::new(), &draw);
        assert!(matches!(refused, Err(ResolveError::Invalid(_))));
    }
}
