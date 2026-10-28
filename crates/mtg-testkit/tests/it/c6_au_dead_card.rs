//! Goal: a trigger on another creature dying finds that card in its new zone (CR 400.7) at tables of two, three and four seats: Patron of the Vein and Curse of Clinging Webs exile the opponent's creature card from the graveyard and do their second thing, Changing Loyalty and Fool's Demise return it to the battlefield under the Aura's controller (Fool's Demise also returns to its owner's hand), and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::copying::lands;
use common::{names, pid};
use mtg_cards::value::CounterKind;
use mtg_engine::choose::Target;
use mtg_engine::decision::DecisionKind;
use mtg_engine::replay;
use mtg_engine::zones::Zone;
use mtg_testkit::corpus::card;
use mtg_testkit::scenario::{attached_to, enchanting};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The card of seat 0 that acts on the creature of seat 1 dying.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Watcher {
    Patron,
    Curse,
    ChangingLoyalty,
    FoolsDemise,
}

impl Watcher {
    const fn name(self) -> &'static str {
        match self {
            Self::Patron => "Patron of the Vein",
            Self::Curse => "Curse of Clinging Webs",
            Self::ChangingLoyalty => "Changing Loyalty",
            Self::FoolsDemise => "Fool's Demise",
        }
    }

    /// Whether the card returns the creature rather than exiling it.
    const fn returns(self) -> bool {
        matches!(self, Self::ChangingLoyalty | Self::FoolsDemise)
    }
}

/// Seat 0 holds Lightning Bolt over a Mountain and the watcher; seat 1 controls Grizzly Bears (an Aura watcher is on
/// them, a Curse on seat 1).
fn game(seats: usize, watcher: Watcher) -> Scenario {
    let held = ready(card(watcher.name()).unwrap());
    let held = match watcher {
        Watcher::Patron => held,
        Watcher::Curse => enchanting(held, 1),
        Watcher::ChangingLoyalty | Watcher::FoolsDemise => attached_to(held, 1, 0),
    };
    Scenario::new(seats)
        .hand_named(0, ["Lightning Bolt"])
        .battlefield(0, lands(0, 1).into_iter().chain([held]))
        .battlefield(1, [ready(card("Grizzly Bears").unwrap())])
}

/// Passes and answers every question (the order of triggers among them) until a player has priority over an empty
/// stack.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        let decision = driver.engine().decision().map(|d| d.kind.clone());
        let idle = driver.state().stack().is_empty() && driver.state().resolving().is_none();
        match decision {
            Some(DecisionKind::Priority(_)) if idle => return Ok(()),
            Some(DecisionKind::Priority(_)) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// The permanents named `name` that `seat` controls.
fn controlled(driver: &Driver, seat: usize, name: &str) -> usize {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            let object = driver.state().object(**id).unwrap();
            object.controller() == pid(seat) && object.card().name == name
        })
        .count()
}

#[rstest]
fn the_card_of_the_creature_that_died_is_found_in_its_new_zone(
    #[values(2, 3, 4)] seats: usize,
    #[values(
        Watcher::Patron,
        Watcher::Curse,
        Watcher::ChangingLoyalty,
        Watcher::FoolsDemise
    )]
    watcher: Watcher,
) -> Result<(), ScriptError> {
    let setup = game(seats, watcher).setup().unwrap();
    let mut driver = Driver::from_scenario(game(seats, watcher))?;
    let bears = driver.id_of("Grizzly Bears").unwrap();
    let target = Target::Object(driver.state().object(bears).unwrap().reference());
    driver.cast_at("Lightning Bolt", &[target])?;
    settle(&mut driver)?;
    let graveyard = names(driver.state(), Zone::Graveyard(pid(1)));
    assert!(
        graveyard.is_empty(),
        "the card left the graveyard: {graveyard:?}"
    );
    if watcher.returns() {
        assert_eq!(
            controlled(&driver, 0, "Grizzly Bears"),
            1,
            "it returned under seat 0"
        );
        assert_eq!(names(driver.state(), Zone::Exile), Vec::<String>::new());
    } else {
        assert_eq!(names(driver.state(), Zone::Exile), ["Grizzly Bears"]);
        assert_eq!(controlled(&driver, 1, "Grizzly Bears"), 0);
    }
    match watcher {
        Watcher::Patron => {
            let patron = driver.id_of("Patron of the Vein").unwrap();
            let counters = driver
                .state()
                .object(patron)
                .and_then(|object| object.permanent())
                .map_or(0, |p| p.counters_of(&CounterKind::PlusOnePlusOne));
            assert_eq!(counters, 1, "the Vampire got its counter");
        }
        Watcher::Curse => assert_eq!(controlled(&driver, 0, "Spider"), 1, "seat 0 made a Spider"),
        Watcher::ChangingLoyalty => assert_eq!(
            names(driver.state(), Zone::Graveyard(pid(0))),
            ["Lightning Bolt", "Changing Loyalty"]
        ),
        Watcher::FoolsDemise => assert!(
            names(driver.state(), Zone::Hand(pid(0))).contains(&"Fool's Demise".to_owned()),
            "the Aura returned to its owner's hand"
        ),
    }
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
