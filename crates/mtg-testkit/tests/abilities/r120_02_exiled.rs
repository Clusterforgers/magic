//! Goal: Sanar reveals until it has as many nonland cards as colors among its controller's permanents and lets them
//! exile one card of each such color (a card taken for one color is not offered again), castable this turn; Invasion of
//! Alara exiles until it has two nonland cards of mana value 4 or less, casts one free or none, puts one into the hand
//! and every other exiled card on the bottom; at two to four seats, and each game replays to its live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, forest, instant, island, mountain, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

fn replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

fn names_in(driver: &Driver, zone: Zone) -> Vec<String> {
    let mut names: Vec<String> = driver
        .state()
        .zone(zone)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .map(|object| object.card().name.clone())
        .collect();
    names.sort();
    names
}

fn red(name: &str) -> Arc<CardDefinition> {
    instant(name, 0, &[Color::Red])
}

fn blue(name: &str) -> Arc<CardDefinition> {
    instant(name, 0, &[Color::Blue])
}

/// Plays to the first main phase of seat 0's second turn, answering each choice of a card of a color with the first
/// card offered when `take` holds, with none otherwise; returns how many such choices were asked and what they offered.
fn sanar_turn(driver: &mut Driver, seats: usize, take: bool) -> Vec<Vec<ObjectId>> {
    let turn = 1 + u32::try_from(seats).unwrap();
    let mut asked = Vec::new();
    let mut seen = false;
    for _ in 0..400 {
        let (number, step) = (driver.state().turn().number(), driver.state().turn().step());
        let main = number == turn && step == Step::Main1;
        seen |= main && !driver.state().stack().is_empty();
        if seen && main && driver.state().stack().is_empty() {
            break;
        }
        let decision = driver.decision().unwrap().clone();
        match &decision.kind {
            DecisionKind::ChooseCards(options) if decision.player == PlayerId(0) => {
                asked.push(options.from.clone());
                let picked = if take {
                    vec![options.from[0]]
                } else {
                    Vec::new()
                };
                driver
                    .answer(Answer::ChooseCards(CardsChoice(picked)))
                    .unwrap();
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
        if number > turn {
            break;
        }
    }
    asked
}

#[rstest]
fn sanar_exiles_one_card_of_each_color(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] take: bool,
) {
    let scenario = Scenario::new(seats)
        .battlefield(
            0,
            [
                ready(card("Sanar, Innovative First-Year").unwrap()),
                ready(island()),
                ready(mountain()),
            ],
        )
        .library(
            0,
            [
                plains(),
                plains(),
                red("Red One"),
                blue("Blue One"),
                red("Red Two"),
                blue("Blue Two"),
            ],
        );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let asked = sanar_turn(&mut driver, seats, take);
    assert_eq!(
        asked.len(),
        2,
        "one choice for red, one for blue: {asked:?}"
    );
    assert!(asked.iter().all(|offered| offered.len() == 1), "{asked:?}");
    if take {
        assert_eq!(names_in(&driver, Zone::Exile), ["Blue One", "Red One"]);
        let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
            panic!("priority was expected");
        };
        let red_one = driver
            .state()
            .zone(Zone::Exile)
            .iter()
            .copied()
            .find(|id| {
                driver
                    .state()
                    .object(*id)
                    .is_some_and(|o| o.card().name == "Red One")
            })
            .unwrap();
        assert!(
            options.castable.contains(&red_one),
            "the exiled cards may be cast this turn"
        );
    } else {
        assert_eq!(names_in(&driver, Zone::Exile), Vec::<String>::new());
    }
    replays(&driver);
}

#[rstest]
fn sanar_offers_a_card_taken_for_one_color_to_no_other(#[values(2, 3, 4)] seats: usize) {
    let gold = instant("Gold", 0, &[Color::Blue, Color::Red]);
    let scenario = Scenario::new(seats)
        .battlefield(0, [ready(card("Sanar, Innovative First-Year").unwrap())])
        .library(
            0,
            [
                plains(),
                plains(),
                gold,
                plains(),
                blue("Blue One"),
                red("Red One"),
            ],
        );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let asked = sanar_turn(&mut driver, seats, true);
    // Blue comes first on the color wheel: Gold or Blue One; then red finds nothing left once Gold is taken.
    assert_eq!(asked.len(), 1, "{asked:?}");
    assert_eq!(asked[0].len(), 2);
    assert_eq!(names_in(&driver, Zone::Exile).len(), 1);
    replays(&driver);
}

#[rstest]
fn invasion_of_alara_finds_two_cards(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] cast: bool,
) {
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(plains()),
                ready(island()),
                ready(swamp()),
                ready(mountain()),
                ready(forest()),
            ],
        )
        .hand(
            0,
            [card("Invasion of Alara // Awaken the Maelstrom").unwrap()],
        )
        .library(
            0,
            [
                plains(),
                creature("Big", 5, &[], 5, 5),
                red("Red One"),
                plains(),
                blue("Blue One"),
                red("Red Two"),
            ],
        );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Invasion of Alara").unwrap();
    let mut chosen_to_cast = false;
    let mut kept = false;
    for _ in 0..200 {
        let decision = driver.decision().unwrap().clone();
        match &decision.kind {
            DecisionKind::ChooseCards(options)
                if decision.player == PlayerId(0) && !chosen_to_cast =>
            {
                chosen_to_cast = true;
                assert_eq!(options.from.len(), 2, "the two cards found");
                let picked = if cast {
                    vec![options.from[0]]
                } else {
                    Vec::new()
                };
                driver
                    .answer(Answer::ChooseCards(CardsChoice(picked)))
                    .unwrap();
            }
            DecisionKind::ChooseCards(options) if decision.player == PlayerId(0) => {
                kept = true;
                assert_eq!(
                    options.from.len(),
                    2,
                    "both stay to choose from when none was cast"
                );
                driver
                    .answer(Answer::ChooseCards(CardsChoice(vec![options.from[0]])))
                    .unwrap();
            }
            DecisionKind::YesNo(_) if decision.player == PlayerId(0) => {
                driver.answer(Answer::YesNo(YesNoChoice(true))).unwrap();
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() && chosen_to_cast => {
                break;
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    assert!(chosen_to_cast);
    assert_eq!(
        kept, !cast,
        "one card left goes to the hand without a choice"
    );
    let hand = names_in(&driver, Zone::Hand(PlayerId(0)));
    assert_eq!(hand.len(), 1, "{hand:?}");
    assert_eq!(names_in(&driver, Zone::Exile), Vec::<String>::new());
    let graveyard = names_in(&driver, Zone::Graveyard(PlayerId(0)));
    assert_eq!(graveyard.len(), usize::from(cast), "{graveyard:?}");
    // The plains, the big creature and a found card not cast went to the bottom; Red Two stayed on top.
    let library = driver.state().zone(Zone::Library(PlayerId(0)));
    assert_eq!(library.len(), if cast { 4 } else { 5 });
    let top = library
        .last()
        .and_then(|id| driver.state().object(*id))
        .unwrap();
    assert_eq!(top.card().name, "Red Two");
    replays(&driver);
}
