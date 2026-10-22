//! Goal: Consider, Curate and the other surveil cards play whole at tables of two, three and four seats, as their cards say: a surveil shows the top cards to its player alone and asks which go into the graveyard (none, some or all, from a library of any length, and not at all for a count of nothing or an empty library) and the order of the cards that stay; the cards put there are moved by an effect and not milled; a card that replaces a move to the graveyard sends the surveilled card elsewhere; every player of "each player surveils" decides before a library changes; and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::{name_of, names, pid};
use mtg_cards::CardDefinition;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{CardsChoice, Choice, OrderChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::events::{ResolutionOutcome, ZoneCause};
use mtg_engine::library::Surveilled;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{island, surveiler};
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
const SURVEIL: &str = "Put any number of the cards into your graveyard.";
const ON_TOP: &str = "Put the cards on top of your library in any order.";

fn named(list: &[&str]) -> Vec<Arc<CardDefinition>> {
    list.iter().map(|name| card(name).unwrap()).collect()
}

/// Seat 0 holds `spell` and controls two Islands; its library is `library` (top first).
fn table(seats: usize, spell: Arc<CardDefinition>, library: &[&str]) -> Scenario {
    Scenario::new(seats)
        .hand(0, [spell])
        .library(0, named(library))
        .battlefield(0, [ready(island()), ready(island())])
}

fn passes_until_asked(driver: &mut Driver) -> Result<(), ScriptError> {
    while matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        && !driver.state().stack().is_empty()
    {
        driver.pass()?;
    }
    Ok(())
}

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

fn from_names(driver: &Driver, ids: &[ObjectId]) -> Vec<String> {
    ids.iter().map(|id| name_of(driver.state(), *id)).collect()
}

/// A library from the top down, as names.
fn library(driver: &Driver, seat: usize) -> Vec<String> {
    let mut library = names(driver.state(), Zone::Library(pid(seat)));
    library.reverse();
    library
}

/// The names in seat 0's graveyard, sorted: the order of a graveyard is not a rule.
fn graveyard_of(driver: &Driver) -> Vec<String> {
    let mut graveyard = names(driver.state(), Zone::Graveyard(pid(0)));
    graveyard.sort_unstable();
    graveyard
}

fn held(driver: &Driver, seat: usize) -> Vec<ObjectId> {
    driver.state().zone(Zone::Library(pid(seat))).to_vec()
}

fn surveilled(events: &[Event]) -> Vec<&Surveilled> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Surveilled(surveilled) => Some(surveilled),
            _ => None,
        })
        .collect()
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
            Event::Surveilled(_) => Some("surveilled"),
            Event::ZoneChanged {
                from: Zone::Library(_),
                to: Zone::Graveyard(_),
                cause: ZoneCause::Moved,
                ..
            } => Some("put into the graveyard"),
            Event::ZoneChanged {
                cause: ZoneCause::Milled,
                ..
            } => Some("milled"),
            Event::ResolutionEnded {
                outcome: ResolutionOutcome::Resolved,
                ..
            } => Some("resolved"),
            _ => None,
        })
        .collect()
}

#[rstest]
fn consider_surveils_one_card_and_then_draws(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] graveyard: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, card("Consider").unwrap(), &LIBRARY))?;
    let others: Vec<Vec<ObjectId>> = (1..seats).map(|seat| held(&driver, seat)).collect();
    let random = driver.state().rng_index();
    resolve(&mut driver, "Consider")?;

    // The controller sees the top card and decides; nobody else is asked.
    let decision = driver.decision()?.clone();
    let DecisionKind::ChooseCards(options) = &decision.kind else {
        panic!("the surveil asks which cards go into the graveyard");
    };
    assert_eq!(decision.player, pid(0));
    assert_eq!(from_names(&driver, &options.from), [LIBRARY[0]]);
    assert_eq!((options.min, options.max), (0, 1));
    assert_eq!(options.text, SURVEIL);
    let top = options.from[0];
    let begun = driver.engine().events().len();
    let gone = if graveyard { vec![top] } else { Vec::new() };
    driver.answer(cards(&gone))?;
    passes_until_asked(&mut driver)?;

    let played = &driver.engine().events()[begun..];
    let mut told = vec!["chose", "surveilled"];
    if graveyard {
        told.push("put into the graveyard");
    }
    told.extend(["drawn", "resolved"]);
    assert_eq!(story(played), told);
    let (kept, sent) = if graveyard {
        (Vec::new(), vec![top])
    } else {
        (vec![top], Vec::new())
    };
    assert_eq!(
        surveilled(played),
        [&Surveilled {
            player: pid(0),
            top: kept,
            graveyard: sent
        }]
    );
    assert_eq!(
        names(driver.state(), Zone::Hand(pid(0))),
        [LIBRARY[usize::from(graveyard)]]
    );
    assert_eq!(
        graveyard_of(&driver),
        if graveyard {
            vec!["Consider", LIBRARY[0]]
        } else {
            vec!["Consider"]
        }
    );
    assert_eq!(library(&driver, 0), LIBRARY[usize::from(graveyard) + 1..]);
    for (seat, before) in (1..seats).zip(others) {
        assert_eq!(held(&driver, seat), before, "seat {seat} was left alone");
    }
    assert_eq!(
        driver.state().rng_index(),
        random,
        "a surveil draws no random number"
    );
    Ok(())
}

/// The surveil of `count` cards of a library of `length`: the indices of those that go into the graveyard, and
/// whether the order asked is answered backwards.
#[rstest]
#[case::none_go_in_the_order_given(3, 6, &[], false)]
#[case::none_go_and_the_rest_is_reversed(3, 6, &[], true)]
#[case::the_middle_card_goes(3, 6, &[1], false)]
#[case::the_middle_card_goes_the_rest_reversed(3, 6, &[1], true)]
#[case::the_top_and_the_third_go(3, 6, &[0, 2], false)]
#[case::all_go(3, 6, &[0, 1, 2], false)]
#[case::a_surveil_of_one(1, 6, &[0], false)]
#[case::a_short_library_is_looked_at_whole(4, 2, &[0], false)]
#[case::a_short_library_kept_and_reversed(4, 2, &[], true)]
#[case::a_library_of_one(2, 1, &[], false)]
#[case::a_library_of_one_sent(2, 1, &[0], false)]
fn a_surveil_puts_any_number_into_the_graveyard_and_orders_what_stays_on_top(
    #[case] count: i32,
    #[case] length: usize,
    #[case] gone: &[usize],
    #[case] backwards: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let deck = &LIBRARY[..length];
    let looked = deck.len().min(usize::try_from(count).unwrap());
    let mut driver = Driver::from_scenario(table(seats, surveiler("Surveil", 0, count), deck))?;
    let others: Vec<Vec<ObjectId>> = (1..seats).map(|seat| held(&driver, seat)).collect();
    resolve(&mut driver, "Surveil")?;

    let mut texts = Vec::new();
    while let DecisionKind::ChooseCards(_) | DecisionKind::OrderCards(_) = &driver.decision()?.kind
    {
        let decision = driver.decision()?.clone();
        assert_eq!(
            decision.player,
            pid(0),
            "the surveilling player alone is asked"
        );
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
                cards(&gone.iter().map(|at| options.from[*at]).collect::<Vec<_>>())
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
        assert!(texts.len() < 3, "a surveil asks at most two questions");
    }

    let kept: Vec<usize> = (0..looked).filter(|at| !gone.contains(at)).collect();
    let mut stay: Vec<&str> = kept.iter().map(|at| deck[*at]).collect();
    let sent: Vec<&str> = gone.iter().map(|at| deck[*at]).collect();
    let mut expected_texts = vec![SURVEIL];
    if stay.len() > 1 {
        expected_texts.push(ON_TOP);
    }
    if backwards {
        stay.reverse();
    }
    assert_eq!(texts, expected_texts);
    // The library from the top down: the cards that stayed, then what was not looked at.
    assert_eq!(
        library(&driver, 0),
        [stay.clone(), deck[looked..].to_vec()].concat()
    );
    let mut expected_graveyard = vec!["Surveil"];
    expected_graveyard.extend(&sent);
    expected_graveyard.sort_unstable();
    assert_eq!(graveyard_of(&driver), expected_graveyard);
    let events = driver.engine().events();
    let done = surveilled(events);
    assert_eq!(done.len(), 1);
    assert_eq!(from_names(&driver, &done[0].top), stay);
    assert_eq!(from_names(&driver, &done[0].graveyard), sent);
    assert!(
        !story(events).contains(&"milled"),
        "putting a card into the graveyard is not milling it"
    );
    for (seat, before) in (1..seats).zip(others) {
        assert_eq!(held(&driver, seat), before, "seat {seat} was left alone");
    }
    Ok(())
}

#[rstest]
#[case::a_count_of_nothing(0, &LIBRARY)]
#[case::an_empty_library(3, &[])]
fn a_surveil_of_nothing_or_of_an_empty_library_asks_nothing_and_does_not_happen(
    #[case] count: i32,
    #[case] deck: &[&str],
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, surveiler("Surveil", 0, count), deck))?;
    let before = held(&driver, 0);
    resolve(&mut driver, "Surveil")?;
    assert!(matches!(driver.decision()?.kind, DecisionKind::Priority(_)));
    assert_eq!(driver.state().stack(), []);
    assert_eq!(story(driver.engine().events()), ["resolved"]);
    assert_eq!(held(&driver, 0), before);
    Ok(())
}

#[rstest]
fn the_log_of_a_surveil_replays_to_the_state_it_was_played_to(
    #[values(2, 3)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats, card("Curate").unwrap(), &LIBRARY);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    resolve(&mut driver, "Curate")?;
    let DecisionKind::ChooseCards(options) = driver.decision()?.kind.clone() else {
        panic!("the surveil asks which cards go");
    };
    driver.answer(cards(&options.from[..1]))?;
    passes_until_asked(&mut driver)?;
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    assert_eq!(graveyard_of(&driver), ["Curate", LIBRARY[0]]);
    assert_eq!(names(driver.state(), Zone::Hand(pid(0))), [LIBRARY[1]]);
    Ok(())
}
