//! Goal: the objects of a seat that left the game are gone before anything else happens (CR 800.4a), at tables of two to four seats: no replacement question or choice of cards is asked while one remains, whether the seat conceded or lost to a state-based action, and whether it left while an Aura aimed at it waits for the graveyard or while a "look at the top cards, take one from among them" spell or ability waits for its choice (of the seat that left or of another); the choice then goes on with the cards it named, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{names, pid};
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::{CardsChoice, ReplacementChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_engine::{GameSetup, replay};
use mtg_testkit::cards::{bear, burn, forest, mountain, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, enchanting, ready};
use rstest::rstest;

/// How a seat leaves the game.
#[derive(Debug, Clone, Copy)]
enum Exit {
    /// It concedes at a decision of another seat.
    Concedes,
    /// A burn spell of seat 0 takes its last life points.
    ZeroLife,
}

/// The log of the game replays to the state it was played to.
fn replays(driver: &Driver, setup: &GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

/// Nothing is owned by `seat`: its cards, tokens and abilities have left the game with it.
fn nothing_left_of(driver: &Driver, seat: usize) {
    let left: Vec<_> = driver
        .state()
        .objects()
        .filter(|o| o.owner() == pid(seat))
        .map(|o| o.card().name.clone())
        .collect();
    assert!(left.is_empty(), "seat {seat} still owns {left:?}");
}

/// Answers every question with its first option and passes priority until the stack is empty; returns how many
/// replacement questions were asked.
fn settle(driver: &mut Driver) -> Result<u32, ScriptError> {
    let mut asked = 0;
    for _ in 0..200 {
        let Some(decision) = driver.engine().decision().cloned() else {
            return Ok(asked);
        };
        match decision.kind {
            DecisionKind::ChooseReplacement(_) => {
                asked += 1;
                driver.answer(Answer::ChooseReplacement(ReplacementChoice(0)))?;
            }
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty() && driver.state().resolving().is_none() =>
            {
                return Ok(asked);
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Seat 0 holds a Curse that enchants seat 1, seat 2 two Rest in Peace (so the Curse has two replacements to
/// choose between when it goes to the graveyard) and seat 1 a creature, a land and a card in hand.
fn cursed(seats: usize, exit: Exit) -> Scenario {
    let scenario = Scenario::new(seats)
        .battlefield(
            0,
            [
                ready(mountain()),
                enchanting(ready(card("Curse of the Forsaken").unwrap()), 1),
            ],
        )
        .battlefield(1, [ready(bear()), ready(forest())])
        .hand(1, [bear()])
        .battlefield(
            2,
            [
                ready(card("Rest in Peace").unwrap()),
                ready(card("Rest in Peace").unwrap()),
            ],
        );
    match exit {
        Exit::Concedes => scenario,
        Exit::ZeroLife => scenario.hand(0, [burn("Bolt", 3)]).life(1, 3),
    }
}

#[rstest]
fn a_seat_that_leaves_takes_its_objects_before_the_question_its_loss_raises(
    #[values(3, 4)] seats: usize,
    #[values(Exit::Concedes, Exit::ZeroLife)] exit: Exit,
) -> Result<(), ScriptError> {
    let setup = cursed(seats, exit).setup().unwrap();
    let mut driver = Driver::from_scenario(cursed(seats, exit))?;
    match exit {
        Exit::Concedes => {
            driver.concede(pid(1))?;
        }
        Exit::ZeroLife => {
            driver.cast_at("Bolt", &[driver.target_player(1)])?;
            // Both Rest in Peace apply to the Bolt on its way to the graveyard: seat 1 is alive until it resolves.
            for _ in 0..50 {
                if !driver.state().is_alive(pid(1)) {
                    break;
                }
                if matches!(driver.decision()?.kind, DecisionKind::ChooseReplacement(_)) {
                    driver.answer(Answer::ChooseReplacement(ReplacementChoice(0)))?;
                } else {
                    driver.pass()?;
                }
            }
        }
    }
    // The first question after the loss is the Curse's, with nothing of seat 1 left in the game.
    assert!(!driver.state().is_alive(pid(1)));
    nothing_left_of(&driver, 1);
    let kind = driver.decision()?.kind.clone();
    assert!(
        matches!(kind, DecisionKind::ChooseReplacement(_)),
        "the two Rest in Peace ask for an order for the Curse, not {kind:?}"
    );
    settle(&mut driver)?;
    nothing_left_of(&driver, 1);
    let exiled = names(driver.state(), Zone::Exile);
    assert!(
        exiled.contains(&"Curse of the Forsaken".to_owned()),
        "{exiled:?}"
    );
    replays(&driver, &setup);
    Ok(())
}

/// A card that looks at the top cards of its controller's library and takes one from among them.
#[derive(Debug, Clone, Copy)]
struct Among {
    name: &'static str,
    /// How many cards it looks at.
    looks: usize,
    /// Where the card taken goes.
    takes: Zone,
    /// Whether the rest goes to the graveyard (else to the bottom of the library).
    rest_to_graveyard: bool,
}

const SALVAGE: Among = Among {
    name: "Grisly Salvage",
    looks: 5,
    takes: Zone::Hand(PlayerId(0)),
    rest_to_graveyard: true,
};
const WAYFINDER: Among = Among {
    name: "Satyr Wayfinder",
    looks: 4,
    takes: Zone::Hand(PlayerId(0)),
    rest_to_graveyard: true,
};
const REJUVENATOR: Among = Among {
    name: "Elvish Rejuvenator",
    looks: 5,
    takes: Zone::Battlefield,
    rest_to_graveyard: false,
};

/// Seat 0 holds the card and the lands for it, with lands and creatures on top of its library; seat 1 has a creature
/// and two cards in hand, and the other seats a creature each.
fn looking(seats: usize, among: Among) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .hand_named(0, [among.name])
        .battlefield(0, [forest(), forest(), swamp(), swamp()].map(ready))
        .library(
            0,
            [
                forest(),
                bear(),
                swamp(),
                bear(),
                forest(),
                swamp(),
                bear(),
                forest(),
                swamp(),
            ],
        )
        .battlefield(1, [ready(bear()), ready(forest())])
        .hand(1, [bear(), bear()]);
    for seat in 2..seats {
        scenario = scenario.battlefield(seat, [ready(bear())]);
    }
    scenario
}

/// The top `n` cards of seat 0's library, top first.
fn top(driver: &Driver, n: usize) -> Vec<ObjectId> {
    driver
        .state()
        .zone(Zone::Library(pid(0)))
        .iter()
        .rev()
        .take(n)
        .copied()
        .collect()
}

/// Casts the card and passes until seat 0 is asked which card to take from among the cards looked at; returns the
/// cards on offer.
fn until_asked(driver: &mut Driver, among: Among) -> Result<Vec<ObjectId>, ScriptError> {
    driver.cast(among.name)?;
    for _ in 0..50 {
        match driver.decision()?.kind.clone() {
            DecisionKind::ChooseCards(options) => {
                assert_eq!(driver.decision()?.player, pid(0));
                return Ok(options.from);
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck(format!("the choice of {}", among.name)))
}

#[rstest]
fn a_seat_that_leaves_in_the_choice_from_among_the_top_cards_takes_its_objects_and_the_choice_goes_on(
    #[values(2, 3, 4)] seats: usize,
    #[values(SALVAGE, WAYFINDER, REJUVENATOR)] among: Among,
    #[values(1, 2)] leaver: usize,
) -> Result<(), ScriptError> {
    if leaver >= seats {
        return Ok(());
    }
    let setup = looking(seats, among).setup().unwrap();
    let mut driver = Driver::from_scenario(looking(seats, among))?;
    let looked = top(&driver, among.looks);
    let offered = until_asked(&mut driver, among)?;
    assert!(offered.iter().all(|id| looked.contains(id)));
    driver.concede(pid(leaver))?;
    nothing_left_of(&driver, leaver);
    // The choice is still seat 0's, over the same cards.
    let DecisionKind::ChooseCards(options) = driver.decision()?.kind.clone() else {
        panic!("the choice goes on after seat {leaver} left");
    };
    assert_eq!(driver.decision()?.player, pid(0));
    assert_eq!(options.from, offered);
    let taken = offered[0];
    driver.answer(Answer::ChooseCards(CardsChoice(vec![taken])))?;
    settle(&mut driver)?;
    nothing_left_of(&driver, leaver);
    let state = driver.state();
    assert_eq!(
        state
            .object(taken)
            .map(mtg_engine::object::GameObject::zone),
        Some(among.takes)
    );
    let rest = if among.rest_to_graveyard {
        Zone::Graveyard(pid(0))
    } else {
        Zone::Library(pid(0))
    };
    for id in looked.iter().filter(|id| **id != taken) {
        assert_eq!(
            state.object(*id).map(mtg_engine::object::GameObject::zone),
            Some(rest),
            "{id:?}"
        );
    }
    if !among.rest_to_graveyard {
        let bottom = &state.zone(rest)[..among.looks - 1];
        for id in looked.iter().filter(|id| **id != taken) {
            assert!(bottom.contains(id), "{id:?} is not on the bottom");
        }
    }
    if seats == 2 {
        assert!(state.result().is_some(), "the game ends");
    } else {
        assert!(state.is_alive(pid(0)));
    }
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn the_seat_that_looks_leaving_ends_the_look_and_takes_its_cards_with_it(
    #[values(2, 3, 4)] seats: usize,
    #[values(SALVAGE, WAYFINDER, REJUVENATOR)] among: Among,
) -> Result<(), ScriptError> {
    let setup = looking(seats, among).setup().unwrap();
    let mut driver = Driver::from_scenario(looking(seats, among))?;
    until_asked(&mut driver, among)?;
    driver.concede(pid(0))?;
    nothing_left_of(&driver, 0);
    settle(&mut driver)?;
    nothing_left_of(&driver, 0);
    if seats > 2 {
        assert!(driver.state().is_alive(pid(1)));
        assert!(driver.engine().decision().is_some());
    } else {
        assert!(driver.state().result().is_some(), "the game ends");
    }
    replays(&driver, &setup);
    Ok(())
}
