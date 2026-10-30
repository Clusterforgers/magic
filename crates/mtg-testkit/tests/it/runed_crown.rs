//! Goal: Runed Crown ("When this Equipment enters, you may search your library, hand, and/or graveyard for a Rune card and put it onto the battlefield attached to this Equipment. If you search your library this way, shuffle.") searches the zones the player chooses and no others, whether or not each holds a Rune card: the Rune found enters attached to the Crown, a search that finds nothing or a player who declines puts nothing onto the battlefield, the library is shuffled exactly when it was searched, each zone searched is one `CardsFound` and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::ObjectId;
use mtg_engine::choose::{CardsChoice, ChoiceOption, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameSetup, replay};
use mtg_testkit::cards::{free_creature, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::{run, say, settle};
use crate::common::pid;

const RUNE: &str = "Rune of Sustenance";

/// Seat 0 holds Runed Crown and three Plains; a Rune card lies in each zone `holds` names (library, hand, graveyard).
fn game(seats: usize, holds: [bool; 3]) -> Scenario {
    let rune = || card(RUNE).unwrap();
    let filler = || (0..10).map(|_| free_creature("Filler", 1, 1));
    let library: Vec<_> = holds[0].then(rune).into_iter().chain(filler()).collect();
    let mut hand = vec![card("Runed Crown").unwrap()];
    hand.extend(holds[1].then(rune));
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .hand(0, hand)
        .library(0, library)
        .battlefield(0, (0..3).map(|_| ready(plains())));
    if holds[2] {
        scenario = scenario.graveyard(0, [rune()]);
    }
    scenario
}

/// Casts the Crown and resolves it up to the first question of its trigger.
fn enter(driver: &mut Driver, seats: usize) -> Result<(), ScriptError> {
    driver.cast("Runed Crown")?;
    run(driver, seats)
}

/// The label of the option the pending question offers, by its words.
fn choose_zones(driver: &mut Driver, label: &str) -> Result<(), ScriptError> {
    let DecisionKind::ChooseOption(options) = &driver.decision()?.kind else {
        return Err(ScriptError::Stuck("the zones are chosen".into()));
    };
    let at = options
        .options
        .iter()
        .position(|option| matches!(option, ChoiceOption::Label(text) if text == label))
        .ok_or_else(|| ScriptError::Stuck(format!("{label} is not offered")))?;
    driver.answer(Answer::ChooseOption(OptionChoice(at)))?;
    Ok(())
}

/// The cards the pending question offers, by the zone each lies in.
fn offered(driver: &Driver) -> Vec<(ObjectId, Zone)> {
    let DecisionKind::ChooseCards(options) = &driver.decision().unwrap().kind else {
        panic!("cards are chosen");
    };
    options
        .from
        .iter()
        .map(|id| (*id, driver.state().object(*id).unwrap().zone()))
        .collect()
}

fn found(driver: &Driver) -> Vec<Zone> {
    driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::CardsFound(found) => Some(found.zone),
            _ => None,
        })
        .collect()
}

fn shuffles(driver: &Driver) -> usize {
    driver
        .engine()
        .events()
        .iter()
        .filter(|event| matches!(event, Event::LibraryShuffled { .. }))
        .count()
}

/// The Runes on the battlefield and what they are attached to.
fn runes_attached(driver: &Driver) -> Vec<Option<ObjectId>> {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter(|object| object.card().name == RUNE)
        .map(|object| {
            object
                .permanent()
                .and_then(|permanent| permanent.attached_to())
                .map(|host| host.id)
        })
        .collect()
}

fn replays(driver: &Driver, setup: &GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

/// The zones of the labels, in the order the question lists them.
const SEARCHES: [(&str, [bool; 3]); 7] = [
    ("Library, hand and graveyard", [true, true, true]),
    ("Library and hand", [true, true, false]),
    ("Library and graveyard", [true, false, true]),
    ("Hand and graveyard", [false, true, true]),
    ("Library", [true, false, false]),
    ("Hand", [false, true, false]),
    ("Graveyard", [false, false, true]),
];

#[rstest]
fn the_zones_chosen_are_searched_and_the_library_is_shuffled_only_when_it_was_one_of_them(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2, 3, 4, 5, 6)] search: usize,
    #[values(
        [true, true, true],
        [true, false, false],
        [false, true, false],
        [false, false, true],
        [false, false, false]
    )]
    holds: [bool; 3],
    #[values(true, false)] take: bool,
) -> Result<(), ScriptError> {
    let (label, zones) = SEARCHES[search];
    let setup = game(seats, holds).setup().unwrap();
    let mut driver = Driver::from_scenario(game(seats, holds))?;
    enter(&mut driver, seats)?;
    say(&mut driver, true)?;
    // Three zones are named, so the player is asked which of them to search.
    choose_zones(&mut driver, label)?;
    let matching: Vec<bool> = (0..3).map(|at| zones[at] && holds[at]).collect();
    let wanted = matching.iter().any(|m| *m);
    // A graveyard alone is a public zone: it gives up the card it holds without a question (CR 701.23b).
    let forced = wanted && !zones[0] && !zones[1];
    if wanted && !forced {
        let cards = offered(&driver);
        let places: Vec<Zone> = cards.iter().map(|(_, zone)| *zone).collect();
        assert_eq!(places.len(), matching.iter().filter(|m| **m).count());
        if take {
            driver.answer(Answer::ChooseCards(CardsChoice(vec![cards[0].0])))?;
        } else {
            // Finding nothing is allowed: a library or a hand does not have to give up a card.
            driver.answer(Answer::ChooseCards(CardsChoice(Vec::new())))?;
        }
    }
    settle(&mut driver)?;
    let searched: Vec<Zone> = found(&driver);
    let expected: Vec<Zone> = (0..3)
        .filter(|at| zones[*at])
        .map(|at| {
            [
                Zone::Library(pid(0)),
                Zone::Hand(pid(0)),
                Zone::Graveyard(pid(0)),
            ][at]
        })
        .collect();
    assert_eq!(searched, expected, "one CardsFound for each zone searched");
    assert_eq!(
        shuffles(&driver),
        usize::from(zones[0]),
        "shuffled when the library was searched, found or not"
    );
    let crown = driver.id_of("Runed Crown")?;
    let attached = runes_attached(&driver);
    if wanted && (take || forced) {
        assert_eq!(
            attached,
            [Some(crown)],
            "the Rune enters attached to the Crown"
        );
    } else {
        assert_eq!(attached, Vec::<Option<ObjectId>>::new());
    }
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn declining_the_search_looks_in_no_zone_and_shuffles_nothing(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let holds = [true, true, true];
    let setup = game(seats, holds).setup().unwrap();
    let mut driver = Driver::from_scenario(game(seats, holds))?;
    enter(&mut driver, seats)?;
    say(&mut driver, false)?;
    settle(&mut driver)?;
    assert_eq!(found(&driver), []);
    assert_eq!(shuffles(&driver), 0);
    assert_eq!(runes_attached(&driver), []);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn a_search_offers_the_cards_of_every_zone_it_looks_in_to_the_searcher_alone(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let holds = [true, true, true];
    let mut driver = Driver::from_scenario(game(seats, holds))?;
    enter(&mut driver, seats)?;
    say(&mut driver, true)?;
    choose_zones(&mut driver, "Library, hand and graveyard")?;
    let decision = driver.decision()?.clone();
    assert_eq!(decision.player, pid(0), "the searcher alone is asked");
    let mut places: Vec<Zone> = offered(&driver).into_iter().map(|(_, zone)| zone).collect();
    places.sort_by_key(|zone| format!("{zone:?}"));
    assert_eq!(
        places,
        [
            Zone::Graveyard(pid(0)),
            Zone::Hand(pid(0)),
            Zone::Library(pid(0))
        ]
    );
    Ok(())
}
