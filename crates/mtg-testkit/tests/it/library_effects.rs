//! Goal: Opt, Brainstorm and the other library effects play whole at tables of two, three and four seats, as their cards say: a scry shows the top cards and asks which go to the bottom (none, some or all, from a library of any length, and not at all for a count of nothing or an empty library) and the order of the cards on each side; Brainstorm draws three and takes two from the hand back on top in the order its player chooses; a shuffle comes from the game's random stream, the same for the same seed; a reveal, a look and a move to the bottom work in the order of the card, Eternal Witness returns a card from the graveyard only when its controller says yes, the controller of the spell chooses the order of the cards it puts into a library, answers to an order of cards are checked by name, an engine rebuilt from the log asks the same question at each decision, and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::{name_of, names, pid};
use mtg_cards::CardDefinition;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{CardsChoice, Choice, OrderChoice, Target, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::error::IllegalAnswer;
use mtg_engine::events::ResolutionOutcome;
use mtg_engine::library::{LibraryLooked, Revealed, Scried};
use mtg_engine::zones::Zone;
use mtg_engine::{Decision, Engine, EngineError, Event, GameSetup, replay};
use mtg_testkit::cards::{bottomer, forest, island, revealer, scryer, stirrer};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0's library from the top down, none of its names in the hand.
const LIBRARY: [&str; 6] = [
    "Grizzly Bears",
    "Serra Angel",
    "Counterspell",
    "Wrath of God",
    "Sol Ring",
    "Doom Blade",
];
/// What seat 0 holds besides the spell.
const HAND: [&str; 2] = ["Lightning Bolt", "Giant Growth"];
const SCRY: &str = "Put any number of the cards on the bottom of your library.";
const ON_TOP: &str = "Put the cards on top of your library in any order.";
const ON_THE_BOTTOM: &str = "Put the cards on the bottom of your library in any order.";

fn named(list: &[&str]) -> Vec<Arc<CardDefinition>> {
    list.iter().map(|name| card(name).unwrap()).collect()
}

/// Seat 0 holds `spell` and then `hand`, controls an Island and has `library` (top first); the others have libraries of Forests.
fn table(seats: usize, spell: Arc<CardDefinition>, hand: &[&str], library: &[&str]) -> Scenario {
    Scenario::new(seats)
        .hand(0, [vec![spell], named(hand)].concat())
        .library(0, named(library))
        .battlefield(0, [ready(island())])
}

fn passes_until_asked(driver: &mut Driver) -> Result<(), ScriptError> {
    while matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        && !driver.state().stack().is_empty()
    {
        driver.pass()?;
    }
    Ok(())
}

/// Casts the spell and passes until it asks something or is done.
fn resolve(driver: &mut Driver, spell: &str) -> Result<(), ScriptError> {
    driver.cast(spell)?;
    passes_until_asked(driver)
}

fn cards(ids: &[ObjectId]) -> Answer {
    Answer::ChooseCards(CardsChoice(ids.to_vec()))
}

fn order(ids: &[ObjectId]) -> Answer {
    Answer::OrderCards(OrderChoice(ids.to_vec()))
}

/// The ids of the cards called `wanted`, in that order, among `among`.
fn ids_named(driver: &Driver, among: &[ObjectId], wanted: &[&str]) -> Vec<ObjectId> {
    wanted
        .iter()
        .map(|name| {
            *among
                .iter()
                .find(|id| name_of(driver.state(), **id) == *name)
                .unwrap_or_else(|| panic!("{name} is not among the cards offered"))
        })
        .collect()
}

fn from_names(driver: &Driver, ids: &[ObjectId]) -> Vec<String> {
    ids.iter().map(|id| name_of(driver.state(), *id)).collect()
}

/// A library from the top down, as names.
fn library(driver: &Driver, seat: usize) -> Vec<String> {
    let mut library = names(driver.state(), Zone::Library(pid(seat)));
    library.reverse();
    library
}

/// The ids of a library as they are held, bottom first.
fn held(driver: &Driver, seat: usize) -> Vec<ObjectId> {
    driver.state().zone(Zone::Library(pid(seat))).to_vec()
}

/// What happened in a resolution, as short words.
fn story(events: &[Event]) -> Vec<&'static str> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::CardDrawn { .. } => Some("drawn"),
            Event::ChoiceMade {
                choice: Choice::Cards(_),
                ..
            } => Some("chose"),
            Event::ChoiceMade {
                choice: Choice::Order(_),
                ..
            } => Some("ordered"),
            Event::Scried(_) => Some("scried"),
            Event::ZoneChanged {
                to: Zone::Library(_),
                ..
            } => Some("put back"),
            Event::LibraryBottomed { .. } => Some("bottomed"),
            Event::LibraryShuffled { .. } => Some("shuffled"),
            Event::LibraryLooked(_) => Some("looked"),
            Event::Revealed(_) => Some("revealed"),
            Event::ResolutionEnded {
                outcome: ResolutionOutcome::Resolved,
                ..
            } => Some("resolved"),
            _ => None,
        })
        .collect()
}

fn scried(events: &[Event]) -> Vec<&Scried> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Scried(scried) => Some(scried),
            _ => None,
        })
        .collect()
}

#[rstest]
fn opt_scries_one_card_and_then_draws(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] bottom: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, card("Opt").unwrap(), &[], &LIBRARY))?;
    let others: Vec<Vec<ObjectId>> = (1..seats).map(|seat| held(&driver, seat)).collect();
    let random = driver.state().rng_index();
    resolve(&mut driver, "Opt")?;

    // The controller sees the top card and decides; nobody else is asked.
    let decision = driver.decision()?.clone();
    let DecisionKind::ChooseCards(options) = &decision.kind else {
        panic!("the scry asks which cards go to the bottom");
    };
    assert_eq!(decision.player, pid(0));
    assert_eq!(from_names(&driver, &options.from), [LIBRARY[0]]);
    assert_eq!((options.min, options.max), (0, 1));
    assert_eq!(options.text, SCRY);
    let top = options.from[0];
    let begun = driver.engine().events().len();
    let down = if bottom { vec![top] } else { Vec::new() };
    driver.answer(cards(&down))?;
    passes_until_asked(&mut driver)?;

    let played = &driver.engine().events()[begun..];
    assert_eq!(story(played), ["chose", "scried", "drawn", "resolved"]);
    let (kept, sent) = if bottom {
        (Vec::new(), vec![top])
    } else {
        (vec![top], Vec::new())
    };
    assert_eq!(
        scried(played),
        [&Scried {
            player: pid(0),
            top: kept,
            bottom: sent
        }]
    );
    let drawn = LIBRARY[usize::from(bottom)];
    assert_eq!(names(driver.state(), Zone::Hand(pid(0))), [drawn]);
    let mut expected: Vec<&str> = LIBRARY[usize::from(bottom) + 1..].to_vec();
    if bottom {
        expected.push(LIBRARY[0]);
    }
    assert_eq!(library(&driver, 0), expected);
    for (seat, before) in (1..seats).zip(others) {
        assert_eq!(held(&driver, seat), before, "seat {seat} was left alone");
    }
    assert_eq!(
        driver.state().rng_index(),
        random,
        "a scry draws no random number"
    );
    Ok(())
}

/// The scry of `looked` cards of a library of `length`: the indices of those that go to the bottom, and whether the
/// orders asked are answered backwards.
#[rstest]
#[case::none_go_down_in_the_order_given(3, 6, &[], false)]
#[case::none_go_down_and_the_rest_is_reversed(3, 6, &[], true)]
#[case::the_middle_card_goes_down(3, 6, &[1], false)]
#[case::the_middle_card_goes_down_the_rest_reversed(3, 6, &[1], true)]
#[case::two_go_down_in_the_order_given(3, 6, &[0, 2], false)]
#[case::two_go_down_reversed(3, 6, &[0, 2], true)]
#[case::all_go_down(3, 6, &[0, 1, 2], false)]
#[case::all_go_down_reversed(3, 6, &[0, 1, 2], true)]
#[case::a_scry_of_one(1, 6, &[0], false)]
#[case::a_short_library_is_looked_at_whole(4, 2, &[0], false)]
#[case::a_short_library_kept_and_reversed(4, 2, &[], true)]
#[case::a_library_of_one(2, 1, &[], false)]
#[case::a_library_of_one_sent_down(2, 1, &[0], false)]
fn a_scry_puts_any_number_on_the_bottom_and_orders_what_is_left_on_each_side(
    #[case] count: i32,
    #[case] length: usize,
    #[case] down: &[usize],
    #[case] backwards: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let deck = &LIBRARY[..length];
    let looked = deck.len().min(usize::try_from(count).unwrap());
    let mut driver = Driver::from_scenario(table(seats, scryer("Scry", 0, count), &[], deck))?;
    let others: Vec<Vec<ObjectId>> = (1..seats).map(|seat| held(&driver, seat)).collect();
    resolve(&mut driver, "Scry")?;

    let (mut asked, mut texts) = (0, Vec::new());
    while let DecisionKind::ChooseCards(_) | DecisionKind::OrderCards(_) = &driver.decision()?.kind
    {
        let decision = driver.decision()?.clone();
        assert_eq!(decision.player, pid(0), "the scrying player alone is asked");
        asked += 1;
        let answer = match decision.kind {
            DecisionKind::ChooseCards(options) => {
                assert_eq!(
                    from_names(&driver, &options.from),
                    deck[..looked],
                    "the cards looked at, top first"
                );
                assert_eq!(
                    (options.min, options.max),
                    (0, u32::try_from(looked).unwrap())
                );
                texts.push(options.text.clone());
                cards(&down.iter().map(|at| options.from[*at]).collect::<Vec<_>>())
            }
            DecisionKind::OrderCards(options) => {
                texts.push(options.text.clone());
                let mut offered = options.cards;
                if backwards {
                    offered.reverse();
                }
                order(&offered)
            }
            other => panic!("unexpected {}", other.name()),
        };
        driver.answer(answer)?;
        passes_until_asked(&mut driver)?;
        assert!(asked < 5, "a scry asks at most three questions");
    }

    let kept: Vec<usize> = (0..looked).filter(|at| !down.contains(at)).collect();
    let mut stay: Vec<&str> = kept.iter().map(|at| deck[*at]).collect();
    let mut sent: Vec<&str> = down.iter().map(|at| deck[*at]).collect();
    let mut expected_texts = vec![SCRY];
    if stay.len() > 1 {
        expected_texts.push(ON_TOP);
    }
    if sent.len() > 1 {
        expected_texts.push(ON_THE_BOTTOM);
    }
    if backwards {
        stay.reverse();
        sent.reverse();
    }
    assert_eq!(texts, expected_texts);
    // The library from the top down: the cards that stayed, what was not looked at, the cards sent down.
    let expected: Vec<&str> = [stay.clone(), deck[looked..].to_vec(), sent.clone()].concat();
    assert_eq!(library(&driver, 0), expected);
    let events = driver.engine().events();
    let done = scried(events);
    assert_eq!(done.len(), 1);
    assert_eq!(from_names(&driver, &done[0].top), stay);
    assert_eq!(from_names(&driver, &done[0].bottom), sent);
    for (seat, before) in (1..seats).zip(others) {
        assert_eq!(held(&driver, seat), before, "seat {seat} was left alone");
    }
    Ok(())
}

#[rstest]
#[case::a_count_of_nothing(0, &LIBRARY)]
#[case::an_empty_library(3, &[])]
fn a_scry_of_nothing_or_of_an_empty_library_asks_nothing_and_does_not_happen(
    #[case] count: i32,
    #[case] deck: &[&str],
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, scryer("Scry", 0, count), &[], deck))?;
    let before = held(&driver, 0);
    resolve(&mut driver, "Scry")?;
    assert!(matches!(driver.decision()?.kind, DecisionKind::Priority(_)));
    assert_eq!(driver.state().stack(), []);
    assert_eq!(story(driver.engine().events()), ["resolved"]);
    assert_eq!(held(&driver, 0), before);
    Ok(())
}

/// Brainstorm from a hand of Lightning Bolt and Giant Growth: the three cards drawn are Grizzly Bears, Serra Angel and
/// Counterspell. Cases give the two cards put back and the order they go in, top card first.
#[rstest]
#[case::two_from_the_hand(&["Lightning Bolt", "Giant Growth"], &["Giant Growth", "Lightning Bolt"])]
#[case::two_from_the_hand_in_the_order_given(&["Lightning Bolt", "Giant Growth"], &["Lightning Bolt", "Giant Growth"])]
#[case::two_just_drawn(&["Grizzly Bears", "Counterspell"], &["Counterspell", "Grizzly Bears"])]
#[case::one_of_each(&["Lightning Bolt", "Serra Angel"], &["Serra Angel", "Lightning Bolt"])]
#[case::the_card_just_drawn_first(&["Giant Growth", "Counterspell"], &["Counterspell", "Giant Growth"])]
fn brainstorm_draws_three_and_puts_two_back_on_top_in_the_order_chosen(
    #[case] back: &[&str],
    #[case] first_to_last: &[&str],
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(table(seats, card("Brainstorm").unwrap(), &HAND, &LIBRARY))?;
    let others: Vec<Vec<ObjectId>> = (1..seats).map(|seat| held(&driver, seat)).collect();
    resolve(&mut driver, "Brainstorm")?;

    // The three cards are drawn and the whole hand is offered, to the controller alone.
    let decision = driver.decision()?.clone();
    let DecisionKind::ChooseCards(options) = &decision.kind else {
        panic!("two cards are chosen from the hand");
    };
    assert_eq!(decision.player, pid(0));
    let mut offered = from_names(&driver, &options.from);
    offered.sort();
    assert_eq!(
        offered,
        [
            "Counterspell",
            "Giant Growth",
            "Grizzly Bears",
            "Lightning Bolt",
            "Serra Angel"
        ]
    );
    assert_eq!(
        options.from.len(),
        driver.state().zone(Zone::Hand(pid(0))).len()
    );
    assert_eq!((options.min, options.max), (2, 2));
    assert_eq!(options.text, "Choose 2 cards in your hand.");
    let begun = driver.engine().events().len() - 3 - 1;
    let from = options.from.clone();
    driver.answer(cards(&ids_named(&driver, &from, back)))?;

    // Then the player puts the two in an order; the cards are offered as they are held.
    let decision = driver.decision()?.clone();
    let DecisionKind::OrderCards(options) = &decision.kind else {
        panic!("the two cards are put in an order");
    };
    assert_eq!(decision.player, pid(0));
    assert_eq!(options.text, ON_TOP);
    let mut offered = from_names(&driver, &options.cards);
    offered.sort();
    let mut wanted: Vec<String> = back.iter().map(|name| (*name).to_owned()).collect();
    wanted.sort();
    assert_eq!(offered, wanted);
    let cards_offered = options.cards.clone();
    driver.answer(order(&ids_named(&driver, &cards_offered, first_to_last)))?;
    passes_until_asked(&mut driver)?;

    let played = &driver.engine().events()[begun..];
    assert_eq!(
        story(played),
        [
            "drawn", "drawn", "drawn", "chose", "ordered", "put back", "put back", "resolved"
        ]
    );
    let in_hand: Vec<&str> = [
        "Lightning Bolt",
        "Giant Growth",
        "Grizzly Bears",
        "Serra Angel",
        "Counterspell",
    ]
    .into_iter()
    .filter(|name| !back.contains(name))
    .collect();
    assert_eq!(names(driver.state(), Zone::Hand(pid(0))), in_hand);
    let expected: Vec<&str> = [first_to_last.to_vec(), LIBRARY[3..].to_vec()].concat();
    assert_eq!(library(&driver, 0), expected);
    for (seat, before) in (1..seats).zip(others) {
        assert_eq!(held(&driver, seat), before, "seat {seat} was left alone");
    }
    Ok(())
}

#[test]
fn brainstorm_from_a_library_of_exactly_three_cards_empties_it_and_fills_it_again()
-> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(table(2, card("Brainstorm").unwrap(), &[], &LIBRARY[..3]))?;
    resolve(&mut driver, "Brainstorm")?;
    let DecisionKind::ChooseCards(options) = driver.decision()?.kind.clone() else {
        panic!("two of the three cards are chosen");
    };
    let mut offered = from_names(&driver, &options.from);
    offered.sort();
    assert_eq!(offered, ["Counterspell", "Grizzly Bears", "Serra Angel"]);
    assert_eq!(driver.state().zone(Zone::Library(pid(0))), []);
    let back = ids_named(&driver, &options.from, &["Grizzly Bears", "Counterspell"]);
    driver.answer(cards(&back))?;
    let DecisionKind::OrderCards(options) = driver.decision()?.kind.clone() else {
        panic!("the two cards are put in an order");
    };
    assert_eq!(options.cards.len(), 2);
    driver.answer(order(&ids_named(
        &driver,
        &options.cards,
        &["Counterspell", "Grizzly Bears"],
    )))?;
    passes_until_asked(&mut driver)?;
    assert_eq!(library(&driver, 0), ["Counterspell", "Grizzly Bears"]);
    assert_eq!(names(driver.state(), Zone::Hand(pid(0))), ["Serra Angel"]);
    assert!(
        driver.state().result().is_none(),
        "the library was never drawn from while empty"
    );
    Ok(())
}

#[rstest]
#[case::the_same_seed(11, 11, true)]
#[case::another_seed(11, 12, false)]
fn a_shuffle_comes_from_the_games_random_stream_and_a_seed_gives_the_same_order(
    #[case] first: u64,
    #[case] second: u64,
    #[case] same: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    /// Every library before and after the spell, the random numbers it used and the log.
    struct Shuffles {
        before: Vec<Vec<ObjectId>>,
        after: Vec<Vec<ObjectId>>,
        used: u64,
        events: Vec<Event>,
    }
    let play = |seed: u64| -> Result<Shuffles, ScriptError> {
        let mut scenario = table(seats, stirrer("Stir"), &[], &LIBRARY).seed(seed);
        for seat in 1..seats {
            scenario = scenario.library_named(seat, LIBRARY);
        }
        let mut driver = Driver::from_scenario(scenario)?;
        let before: Vec<Vec<ObjectId>> = (0..seats).map(|seat| held(&driver, seat)).collect();
        let start = driver.state().rng_index();
        resolve(&mut driver, "Stir")?;
        Ok(Shuffles {
            before,
            after: (0..seats).map(|seat| held(&driver, seat)).collect(),
            used: driver.state().rng_index() - start,
            events: driver.engine().events().to_vec(),
        })
    };
    let Shuffles {
        before,
        after,
        used,
        events,
    } = play(first)?;
    let Shuffles {
        after: again,
        events: replayed,
        ..
    } = play(second)?;
    // Every library is shuffled, in turn order from the active player, and keeps exactly its cards.
    let shuffled: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            Event::LibraryShuffled { player, order } => Some((*player, order.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(
        shuffled
            .iter()
            .map(|(player, _)| *player)
            .collect::<Vec<_>>(),
        (0..seats).map(pid).collect::<Vec<_>>()
    );
    assert_eq!(
        used, seats as u64,
        "each shuffle moves the random stream on"
    );
    for seat in 0..seats {
        let (mut was, mut is) = (before[seat].clone(), after[seat].clone());
        assert_eq!(is, shuffled[seat].1, "the library holds the order logged");
        was.sort_unstable();
        is.sort_unstable();
        assert_eq!(was, is, "a shuffle keeps the cards");
    }
    assert_ne!(before, after, "the libraries changed order");
    if same {
        assert_eq!(after, again);
        assert_eq!(events, replayed);
    } else {
        assert_ne!(after, again);
    }
    Ok(())
}

#[rstest]
fn a_reveal_names_the_hand_and_a_look_and_a_move_put_the_top_card_on_the_bottom(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, revealer("Show"), &HAND, &LIBRARY))?;
    let others: Vec<Vec<ObjectId>> = (1..seats).map(|seat| held(&driver, seat)).collect();
    let (hand_after_cast, top) = (
        driver.state().zone(Zone::Hand(pid(0)))[1..].to_vec(),
        *driver.state().zone(Zone::Library(pid(0))).last().unwrap(),
    );
    let begun = driver.engine().events().len();
    resolve(&mut driver, "Show")?;
    let played = &driver.engine().events()[begun..];
    let wanted = [
        Event::Revealed(Revealed {
            player: pid(0),
            cards: hand_after_cast,
        }),
        Event::LibraryLooked(LibraryLooked {
            player: pid(0),
            cards: vec![top],
        }),
        Event::LibraryBottomed {
            player: pid(0),
            cards: vec![top],
        },
    ];
    let seen: Vec<&Event> = played
        .iter()
        .filter(|event| {
            matches!(
                event,
                Event::Revealed(_) | Event::LibraryLooked(_) | Event::LibraryBottomed { .. }
            )
        })
        .collect();
    assert_eq!(
        seen,
        wanted.iter().collect::<Vec<_>>(),
        "the reveal, the look and the move, once each in the order of the card"
    );
    let mut expected: Vec<&str> = LIBRARY[1..].to_vec();
    expected.push(LIBRARY[0]);
    assert_eq!(library(&driver, 0), expected);
    assert_eq!(held(&driver, 0)[0], top, "it is the bottom card");
    for (seat, before) in (1..seats).zip(others) {
        assert_eq!(held(&driver, seat), before);
    }
    Ok(())
}

#[rstest]
fn the_controller_orders_the_cards_a_spell_puts_on_the_bottom_of_each_library_it_looked_at(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = table(seats, bottomer("Sink"), &[], &LIBRARY);
    for seat in 1..seats {
        scenario = scenario.library_named(seat, LIBRARY);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let mine = held(&driver, 0);
    resolve(&mut driver, "Sink")?;
    // The controller, who looked at them, orders the two cards of each opponent's library in turn order.
    for seat in 1..seats {
        let decision = driver.decision()?.clone();
        let DecisionKind::OrderCards(options) = &decision.kind else {
            panic!("the controller orders the cards");
        };
        assert_eq!(decision.player, pid(0));
        assert_eq!(options.text, ON_THE_BOTTOM);
        assert_eq!(from_names(&driver, &options.cards), LIBRARY[..2]);
        assert_eq!(
            options.cards,
            held(&driver, seat)[4..]
                .iter()
                .rev()
                .copied()
                .collect::<Vec<_>>()
        );
        let mut reversed = options.cards.clone();
        reversed.reverse();
        driver.answer(order(&reversed))?;
        passes_until_asked(&mut driver)?;
    }
    assert!(matches!(driver.decision()?.kind, DecisionKind::Priority(_)));
    assert_eq!(
        held(&driver, 0),
        mine,
        "the controller's own library is untouched"
    );
    // The first of the order lies higher than the last, which is the bottom card.
    let expected: Vec<&str> = [&LIBRARY[2..], &["Serra Angel", "Grizzly Bears"]].concat();
    for seat in 1..seats {
        assert_eq!(library(&driver, seat), expected, "seat {seat}");
    }
    assert_eq!(
        story(driver.engine().events())
            .iter()
            .filter(|s| **s == "bottomed")
            .count(),
        seats - 1
    );
    Ok(())
}

fn refused(driver: &mut Driver, answer: Answer, expected: IllegalAnswer) {
    let (hash, pending) = (
        driver.engine().state_hash(),
        driver.decision().unwrap().clone(),
    );
    let log = driver.engine().events().len();
    let error = driver.answer(answer).unwrap_err();
    assert_eq!(error, ScriptError::Engine(EngineError::Illegal(expected)));
    assert_eq!(
        driver.engine().state_hash(),
        hash,
        "a refusal changes nothing"
    );
    assert_eq!(driver.engine().events().len(), log);
    assert_eq!(
        driver.decision().unwrap(),
        &pending,
        "the same question stands"
    );
}

#[test]
fn an_answer_to_an_order_of_cards_that_is_not_every_card_offered_once_is_refused_by_name()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(3, scryer("Scry", 0, 3), &[], &LIBRARY))?;
    resolve(&mut driver, "Scry")?;
    driver.answer(cards(&[]))?;
    let DecisionKind::OrderCards(options) = driver.decision()?.kind.clone() else {
        panic!("the three cards that stay are ordered");
    };
    let [a, b, c] = options.cards[..] else {
        panic!("three cards are offered");
    };
    let foreign = driver.state().zone(Zone::Library(pid(0)))[0];
    refused(
        &mut driver,
        order(&[a, b, foreign]),
        IllegalAnswer::NotOffered("card"),
    );
    refused(
        &mut driver,
        order(&[a, a, b]),
        IllegalAnswer::Duplicate("card"),
    );
    refused(
        &mut driver,
        order(&[a, b]),
        IllegalAnswer::WrongCount {
            expected: 3,
            got: 2,
        },
    );
    refused(
        &mut driver,
        order(&[]),
        IllegalAnswer::WrongCount {
            expected: 3,
            got: 0,
        },
    );
    refused(
        &mut driver,
        cards(&[a, b, c]),
        IllegalAnswer::WrongKind {
            expected: "order-cards",
            got: "choose-cards",
        },
    );
    driver.answer(order(&[c, a, b]))?;
    passes_until_asked(&mut driver)?;
    let expected: Vec<&str> = [LIBRARY[2], LIBRARY[0], LIBRARY[1]]
        .into_iter()
        .chain(LIBRARY[3..].iter().copied())
        .collect();
    assert_eq!(library(&driver, 0), expected);
    Ok(())
}

#[test]
fn an_answer_to_the_choice_of_cards_for_the_bottom_is_checked_like_any_choice_of_cards()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(2, scryer("Scry", 0, 2), &[], &LIBRARY))?;
    resolve(&mut driver, "Scry")?;
    let DecisionKind::ChooseCards(options) = driver.decision()?.kind.clone() else {
        panic!("the cards for the bottom are chosen");
    };
    let third = driver.state().zone(Zone::Library(pid(0)))[3];
    refused(
        &mut driver,
        cards(&[third]),
        IllegalAnswer::NotOffered("card"),
    );
    refused(
        &mut driver,
        cards(&[options.from[0], options.from[0]]),
        IllegalAnswer::Duplicate("card"),
    );
    refused(
        &mut driver,
        cards(&[options.from[0], options.from[1], third]),
        IllegalAnswer::NotOffered("card"),
    );
    Ok(())
}

#[rstest]
fn eternal_witness_returns_a_card_from_the_graveyard_to_the_hand_only_when_its_controller_says_yes(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] yes: bool,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .hand_named(0, ["Eternal Witness"])
        .graveyard_named(0, ["Lightning Bolt"])
        .battlefield(0, [forest(), forest(), forest()].map(ready));
    let mut driver = Driver::from_scenario(scenario)?;
    let bolt = driver.state().zone(Zone::Graveyard(pid(0)))[0];
    resolve(&mut driver, "Eternal Witness")?;
    // The trigger asks for the card, then whether to return it.
    let aimed = Target::Object(driver.state().object(bolt).unwrap().reference());
    driver.choose_targets(&[aimed])?;
    passes_until_asked(&mut driver)?;
    let decision = driver.decision()?.clone();
    assert_eq!(decision.player, pid(0));
    assert!(matches!(decision.kind, DecisionKind::YesNo(_)));
    driver.answer(Answer::YesNo(YesNoChoice(yes)))?;
    passes_until_asked(&mut driver)?;
    let (hand, graveyard) = if yes {
        (vec!["Lightning Bolt"], Vec::new())
    } else {
        (Vec::new(), vec!["Lightning Bolt"])
    };
    assert_eq!(names(driver.state(), Zone::Hand(pid(0))), hand);
    assert_eq!(names(driver.state(), Zone::Graveyard(pid(0))), graveyard);
    Ok(())
}

/// What a player of these spells answers: cast the spell, pay, put cards back as offered and keep a scry.
fn answer_for(decision: &Decision, spell: &str, backwards: bool) -> Answer {
    match &decision.kind {
        DecisionKind::Priority(options) => Answer::Priority(match options.castable.first() {
            Some(card) if decision.player == pid(0) => PriorityAction::CastSpell(*card),
            _ => PriorityAction::Pass,
        }),
        DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
        DecisionKind::ChooseCards(options) => {
            // A scry keeps everything; a choice of two (Brainstorm) takes the last two.
            let take = if options.min == 0 {
                Vec::new()
            } else {
                options.from[options.from.len() - options.min as usize..].to_vec()
            };
            cards(&take)
        }
        DecisionKind::OrderCards(options) => {
            let mut offered = options.cards.clone();
            if backwards {
                offered.reverse();
            }
            order(&offered)
        }
        other => panic!("the script for {spell} does not expect {}", other.name()),
    }
}

/// The spells played whole: each is cast by seat 0 of three, with its hand and library.
fn plays() -> Vec<(&'static str, Scenario)> {
    vec![
        ("Opt", table(3, card("Opt").unwrap(), &HAND, &LIBRARY)),
        ("Scry", table(3, scryer("Scry", 0, 3), &HAND, &LIBRARY)),
        (
            "Brainstorm",
            table(3, card("Brainstorm").unwrap(), &HAND, &LIBRARY),
        ),
        ("Sink", table(3, bottomer("Sink"), &HAND, &LIBRARY)),
        ("Stir", table(3, stirrer("Stir"), &HAND, &LIBRARY)),
    ]
}

fn finished(driver: &Driver, spell: &str) -> bool {
    driver.state().stack().is_empty()
        && driver
            .state()
            .zone(Zone::Graveyard(pid(0)))
            .iter()
            .any(|id| name_of(driver.state(), *id) == spell)
}

#[test]
fn an_engine_rebuilt_from_the_log_asks_the_same_question_at_every_decision()
-> Result<(), ScriptError> {
    for (spell, scenario) in plays() {
        let setup: GameSetup = scenario.clone().setup()?;
        let mut driver = Driver::from_scenario(scenario)?;
        let mut library_questions = 0;
        for _ in 0..200 {
            let decision = driver.decision()?.clone();
            let mut resumed = Engine::resume(setup.clone(), driver.engine().events().to_vec())?;
            assert_eq!(
                resumed.state_hash(),
                driver.engine().state_hash(),
                "{spell}"
            );
            let mid_cast = matches!(&decision.kind, DecisionKind::PayMana(_));
            if !mid_cast {
                // A half-finished cast lives outside the state; everything else is asked again as it was.
                assert_eq!(resumed.decision(), Some(&decision), "{spell}");
            }
            library_questions += usize::from(matches!(
                decision.kind,
                DecisionKind::ChooseCards(_) | DecisionKind::OrderCards(_)
            ));
            // Both engines take the same answer to the same events.
            let answer = answer_for(&decision, spell, true);
            let live = driver.answer(answer.clone())?;
            if !mid_cast {
                assert_eq!(resumed.apply(answer)?, live, "{spell}");
                assert_eq!(
                    resumed.state_hash(),
                    driver.engine().state_hash(),
                    "{spell}"
                );
            }
            if finished(&driver, spell) {
                break;
            }
        }
        assert!(finished(&driver, spell), "{spell} was played whole");
        assert!(
            library_questions > 0 || spell == "Stir",
            "{spell} asked a question about cards"
        );
    }
    Ok(())
}

#[test]
fn the_log_replays_to_the_state_it_was_played_to_at_every_decision() -> Result<(), ScriptError> {
    for (spell, scenario) in plays() {
        let setup = scenario.clone().setup()?;
        let mut driver = Driver::from_scenario(scenario)?;
        let mut points = Vec::new();
        for _ in 0..200 {
            points.push((driver.engine().events().len(), driver.engine().state_hash()));
            let answer = answer_for(driver.decision()?, spell, false);
            driver.answer(answer)?;
            if finished(&driver, spell) {
                break;
            }
        }
        points.push((driver.engine().events().len(), driver.engine().state_hash()));
        let log = driver.engine().events().to_vec();
        assert!(points.len() > 4, "{spell}");
        for (length, hash) in points {
            assert_eq!(
                replay(&setup, &log[..length])?.state_hash(),
                hash,
                "{spell} after {length} events"
            );
        }
    }
    Ok(())
}
