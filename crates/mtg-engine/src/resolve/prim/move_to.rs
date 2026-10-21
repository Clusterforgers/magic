//! `Effect::Move`: objects change zones, to a hand, a graveyard, a library or the battlefield.

use mtg_cards::effect::{Destination, Effect, Order, Place};
use mtg_cards::refs::Ref;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};

use super::{unsupported, wrong_effect};
use crate::attach;
use crate::choose::{Questions, Stop, Target};
use crate::events::{Event, ZoneCause};
use crate::loyalty;
use crate::mulligan::shuffled;
use crate::object::{GameObject, ObjectKind, ObjectRef};
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

/// The player an object enters the battlefield under when the effect names one; `None` leaves it to its owner.
fn entering_under(cx: &Cx<'_>, under: Option<&Ref>) -> Result<Option<PlayerId>, ResolveError> {
    let Some(who) = under else {
        return Ok(None);
    };
    match cx.scope.players(who)?.as_slice() {
        [] => Ok(None),
        [player] => Ok(Some(*player)),
        _ => Err(ResolveError::Invalid(
            "a permanent enters under the control of one player".into(),
        )),
    }
}

/// Each object the reference names goes to its owner's hand or graveyard, or into its owner's library on top, on
/// the bottom or shuffled in, or onto the battlefield. Cards put into one library together are put in the order the
/// controller chooses ("in any order"), asked as an `OrderCards` per library unless there is one card, and the first
/// of them is nearest the top; a shuffle needs no order.
/// An object already where it would go stays. Moves from the battlefield carry a snapshot of the object as it was.
/// A card put onto the battlefield is one `ZoneChanged`, so what triggers on entering sees it, and an `Entered` after
/// it when the card comes in tapped or under a player other than its owner; an instant or a sorcery card stays where it
/// is (CR 304.4, 307.4). A random order is not played yet; `Caps` keeps those cards out.
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
        Destination::Battlefield { tapped, under } => {
            let under = entering_under(cx, under.as_ref())?;
            return Stop::settle(onto_battlefield(cx, what, &objects, *tapped, under));
        }
    }
    Ok(Run::Done {
        events,
        bound: Vec::new(),
        snapshots,
        next: Next::Sibling,
    })
}

/// The cards put onto the battlefield: one `ZoneChanged` each, so what triggers on entering sees it, and an `Entered`
/// after it for a card that comes in tapped or under a player other than its owner. An Aura that is not cast enters
/// attached to a permanent its controller chooses among those it may enchant (asked unless there is one), and one with
/// nothing to enchant stays where it is (CR 303.4g).
fn onto_battlefield(
    cx: &Cx<'_>,
    what: &Ref,
    objects: &[&GameObject],
    tapped: bool,
    under: Option<PlayerId>,
) -> Result<Run, Stop> {
    let state = cx.state();
    let mut questions = Questions::new(cx);
    let mut events = Vec::new();
    let mut snapshots = Vec::new();
    let mut entered = Vec::new();
    for object in objects {
        let spell = object.base_types().contains(CardType::Instant)
            || object.base_types().contains(CardType::Sorcery);
        if object.zone() == Zone::Battlefield || spell {
            continue;
        }
        let controller = under.unwrap_or_else(|| object.owner());
        let host = match attach::hosts_when_entering(state, object, controller) {
            None => None,
            Some(hosts) => match hosts.as_slice() {
                [] => continue,
                [only] => Some(*only),
                _ => questions
                    .cards(controller, hosts, (1, 1), wording::ENCHANT.into())?
                    .first()
                    .copied(),
            },
        };
        let (event, shot) = moved(state, object.id(), object.zone(), Zone::Battlefield);
        events.push(event);
        snapshots.extend(shot);
        entered.push(object.id());
        if tapped || controller != object.owner() {
            events.push(Event::Entered {
                object: object.id(),
                tapped,
                controller,
                chosen: None,
            });
        }
        // A planeswalker comes in with its loyalty, proposed with the move so a rule that changes what enters with
        // counters can reach it (CR 306.5b).
        events.extend(loyalty::entering(state, object.id()));
        events.extend(host.map(|to| Event::Attached {
            object: object.id(),
            to,
        }));
    }
    questions.end()?;
    Ok(Run::Done {
        events,
        bound: arrived_as(cx, what, &entered),
        snapshots,
        next: Next::Sibling,
    })
}

/// The name that named the cards, bound again to the permanents that `entered`: a card that changes zones is a new
/// object (CR 400.7), and "that creature" in the steps after the move is the one that arrived. A name that is not the
/// reference stays as it is, and so does whatever the name held that did not enter.
fn arrived_as(cx: &Cx<'_>, what: &Ref, entered: &[ObjectId]) -> Vec<(String, Vec<Target>)> {
    let Ref::Bound(name) = what else {
        return Vec::new();
    };
    let Ok(named) = cx.scope.refs(what) else {
        return Vec::new();
    };
    let state = cx.state();
    let renewed: Vec<Target> = named
        .iter()
        .map(|target| match *target {
            Target::Object(object)
                if entered.contains(&object.id)
                    && state
                        .object(object.id)
                        .is_some_and(|live| live.counter() == object.counter) =>
            {
                Target::Object(ObjectRef {
                    id: object.id,
                    counter: object.counter + 1,
                })
            }
            other => other,
        })
        .collect();
    if renewed == named {
        return Vec::new();
    }
    vec![(name.clone(), renewed)]
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
    //! Goal: a move sends each object named to its owner's hand or graveyard or into its owner's library on top, on the bottom or shuffled in; cards put into one library together are put in the order their owner chooses, asked only when there are two or more, and the log leaves the library in exactly that order; an object already in place stays, a stale one or a spell on the stack is passed over, a move from the battlefield carries what it was, a card put onto the battlefield enters untapped or tapped under its owner or the player named while an instant or sorcery card and a permanent already there stay, and an order the engine does not play is refused by name.
    use rstest::rstest;

    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_core::card::Supertype;

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
        let onto_the_battlefield = events.iter().any(|event| {
            matches!(
                event,
                Event::ZoneChanged {
                    to: Zone::Battlefield,
                    ..
                }
            )
        });
        assert!(
            (bound.is_empty() || onto_the_battlefield) && next == Next::Sibling,
            "only cards put onto the battlefield bind their name again"
        );
        (events, snapshots)
    }

    /// What a finished move binds.
    fn bound_by(run: Run) -> Vec<(String, Vec<Target>)> {
        let Run::Done { bound, .. } = run else {
            panic!("the move is done");
        };
        bound
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

    /// Seat 0 holds a basic land, seat 1 a sorcery; the Source is on the battlefield of seat 0.
    fn lands_and_a_spell() -> GameState {
        let land = CardDefinition::new("Forest")
            .with_type(CardType::Land)
            .with_supertype(Supertype::Basic);
        let sorcery = CardDefinition::new("Sorcery").with_type(CardType::Sorcery);
        state(
            vec![vec![Arc::new(land)], vec![Arc::new(sorcery)]],
            vec![vec![creature("Source", 1, 1)], vec![]],
        )
    }

    fn to_the_battlefield(object: ObjectId, from: Zone) -> Event {
        to(object, from, Zone::Battlefield)
    }

    /// The events of moving one card of seat 0's hand to the battlefield as `destination` says, with the name `nobody`
    /// bound to no one.
    fn entering(game: &GameState, card: ObjectId, destination: &str) -> Vec<Event> {
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let mut resolving = Resolving::begin(source.id());
        resolving
            .bindings
            .insert("cards".into(), named(game, &[card]));
        resolving.bindings.insert("nobody".into(), Vec::new());
        let cx = Cx {
            scope: Scope {
                bindings: &resolving.bindings,
                ..Scope::announcing(game, ME, source.reference())
            },
            resolving: &resolving,
        };
        let (events, snapshots) = done(run(&cx, &moving(destination)).unwrap());
        assert_eq!(snapshots, []);
        events
    }

    #[rstest]
    #[case::untapped_under_its_owner(r#"{"Battlefield":{}}"#, false, ME)]
    #[case::tapped_under_its_owner(r#"{"Battlefield":{"tapped":true}}"#, true, ME)]
    #[case::under_an_opponent(r#"{"Battlefield":{"under":"Opponents"}}"#, false, FOE)]
    #[case::tapped_under_an_opponent(
        r#"{"Battlefield":{"tapped":true,"under":"Opponents"}}"#,
        true,
        FOE
    )]
    #[case::under_the_controller(r#"{"Battlefield":{"under":"You"}}"#, false, ME)]
    #[case::under_no_one_it_is_the_owner(
        r#"{"Battlefield":{"under":{"Bound":"nobody"}}}"#,
        false,
        ME
    )]
    fn a_card_enters_the_battlefield_as_the_effect_says(
        #[case] destination: &str,
        #[case] tapped: bool,
        #[case] controller: PlayerId,
    ) {
        let game = game();
        let card = hand(&game, ME)[0];
        let events = entering(&game, card, destination);
        let moved = to_the_battlefield(card, Zone::Hand(ME));
        let entered = Event::Entered {
            object: card,
            tapped,
            controller,
            chosen: None,
        };
        let expected = if tapped || controller != ME {
            vec![moved, entered]
        } else {
            vec![moved]
        };
        assert_eq!(events, expected);
        let folded = after(&game, &events);
        let object = folded.object(card).unwrap();
        assert_eq!(
            (object.zone(), object.controller(), object.owner()),
            (Zone::Battlefield, controller, ME)
        );
        let permanent = object.permanent().unwrap();
        assert_eq!(permanent.tapped(), tapped);
        assert!(permanent.summoning_sick());
    }

    #[test]
    fn a_card_comes_to_the_battlefield_from_a_library_or_a_graveyard_as_from_a_hand() {
        let mut game = game();
        let from_hand = hand(&game, ME)[0];
        let from_library = *library(&game, FOE).last().unwrap();
        let discarded = hand(&game, ME)[1];
        apply_event(
            &mut game,
            &Event::CardDiscarded {
                player: ME,
                object: discarded,
            },
        )
        .unwrap();
        let cards = [from_hand, from_library, discarded];
        let (events, _) =
            done(run_with(&game, &cards, Vec::new(), &moving(r#"{"Battlefield":{}}"#)).unwrap());
        assert_eq!(
            events,
            [
                to_the_battlefield(from_hand, Zone::Hand(ME)),
                to_the_battlefield(from_library, Zone::Library(FOE)),
                to_the_battlefield(discarded, Zone::Graveyard(ME)),
            ]
        );
        let folded = after(&game, &events);
        assert_eq!(folded.object(from_library).unwrap().controller(), FOE);
    }

    #[test]
    fn an_instant_or_sorcery_card_and_a_permanent_already_there_stay_where_they_are() {
        let game = lands_and_a_spell();
        let (sorcery, on_the_field, land) = (
            hand(&game, FOE)[0],
            game.zone(Zone::Battlefield)[0],
            hand(&game, ME)[0],
        );
        let tapped = moving(r#"{"Battlefield":{"tapped":true}}"#);
        let (events, _) =
            done(run_with(&game, &[sorcery, on_the_field, land], Vec::new(), &tapped).unwrap());
        assert_eq!(
            events,
            [
                to_the_battlefield(land, Zone::Hand(ME)),
                Event::Entered {
                    object: land,
                    tapped: true,
                    controller: ME,
                    chosen: None
                }
            ]
        );
    }

    #[test]
    fn the_name_of_cards_put_onto_the_battlefield_names_the_permanents_that_arrived() {
        let game = game();
        let (first, second) = (hand(&game, ME)[0], hand(&game, ME)[1]);
        let on_the_field = game.zone(Zone::Battlefield)[0];
        let mut cards = named(&game, &[first, second, on_the_field]);
        let stale = ObjectRef {
            id: first,
            counter: 99,
        };
        cards.push(Target::Object(stale));
        let run = run_bound(&game, cards, Vec::new(), &moving(r#"{"Battlefield":{}}"#)).unwrap();
        let renewed = |id: ObjectId| {
            let live = game.object(id).unwrap();
            Target::Object(ObjectRef {
                id,
                counter: live.counter() + 1,
            })
        };
        let unmoved = Target::Object(game.object(on_the_field).unwrap().reference());
        assert_eq!(
            bound_by(run),
            [(
                "cards".to_owned(),
                vec![
                    renewed(first),
                    renewed(second),
                    unmoved,
                    Target::Object(stale)
                ]
            )],
            "what arrived is named again, what stayed or was stale is not"
        );
        let stays = named(&game, &[on_the_field]);
        let run = run_bound(&game, stays, Vec::new(), &moving(r#"{"Battlefield":{}}"#)).unwrap();
        assert_eq!(
            bound_by(run),
            [],
            "nothing arrived, so the name is left alone"
        );
    }

    #[test]
    fn cards_moved_anywhere_else_keep_the_name_they_had() {
        let game = game();
        let card = hand(&game, ME)[0];
        for to in [r#""Graveyard""#, TOP, BOTTOM, SHUFFLED] {
            let run = run_with(&game, &[card], Vec::new(), &moving(to)).unwrap();
            assert_eq!(bound_by(run), [], "{to}");
        }
    }

    #[test]
    fn entering_under_several_players_is_refused() {
        let game = game();
        let card = hand(&game, ME)[0];
        let to = r#"{"Battlefield":{"under":"EachPlayer"}}"#;
        let refused = run_with(&game, &[card], Vec::new(), &moving(to));
        assert!(
            matches!(refused, Err(ResolveError::Invalid(_))),
            "{refused:?}"
        );
    }

    #[rstest]
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
