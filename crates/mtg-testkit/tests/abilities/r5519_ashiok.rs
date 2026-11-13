//! Goal: "Spells and abilities your opponents control can't cause their controller to search their library."
//! (`GameRule::OwnSpellsCantSearch`, Ashiok, Dream Render) stops an opponent's own search and nothing else: Ashiok's
//! controller still searches, and an opponent still searches for a spell Ashiok's controller cast (Path to Exile), at
//! two to four seats; the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::{names, pid};
use mtg_engine::characteristics::controller_of;
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// Resolves the stack, taking every card offered and saying yes to every question.
fn r5519_resolve(driver: &mut Driver) {
    for _ in 0..30 {
        if driver.state().stack().is_empty() {
            break;
        }
        let decision = driver.decision().unwrap().clone();
        match decision.kind {
            DecisionKind::ChooseCards(options) => {
                driver
                    .answer(Answer::ChooseCards(CardsChoice(options.from.clone())))
                    .unwrap();
            }
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(true))).unwrap();
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
}

/// How many Forests a seat controls.
fn r5519_forests(driver: &Driver, seat: usize) -> usize {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            driver
                .state()
                .object(**id)
                .is_some_and(|object| object.card().name == "Forest")
                && controller_of(driver.state(), **id) == Some(pid(seat))
        })
        .count()
}

#[rstest]
fn r5519_ashiok_stops_only_an_opponents_own_search(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] ashiok_is_mine: bool,
) {
    let mut mine = vec![ready(forest()), ready(forest())];
    let ashiok = ready(card("Ashiok, Dream Render").unwrap());
    let mut scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .hand(0, [card("Rampant Growth").unwrap()])
        .library(0, [forest(), free_creature("Top", 1, 1)]);
    if ashiok_is_mine {
        mine.push(ashiok);
    } else {
        scenario = scenario.battlefield(seats - 1, [ashiok]);
    }
    let scenario = scenario.battlefield(0, mine);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    driver.cast("Rampant Growth").unwrap();
    r5519_resolve(&mut driver);
    let searched = ashiok_is_mine;
    assert_eq!(r5519_forests(&driver, 0), 2 + usize::from(searched));
    assert_eq!(
        driver.state().zone(Zone::Library(pid(0))).len(),
        2 - usize::from(searched)
    );
    assert!(names(driver.state(), Zone::Graveyard(pid(0))).contains(&"Rampant Growth".to_owned()));
    let replayed = replay(&scenario.setup().unwrap(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

#[rstest]
fn r5519_ashiok_lets_an_opponent_search_for_my_spell(#[values(2, 3, 4)] seats: usize) {
    let foe = seats - 1;
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .hand(0, [card("Path to Exile").unwrap()])
        .battlefield(
            0,
            [
                ready(plains()),
                ready(card("Ashiok, Dream Render").unwrap()),
            ],
        )
        .battlefield(foe, [ready(free_creature("Victim", 2, 2))])
        .library(foe, [forest(), free_creature("Top", 1, 1)]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let victim = driver.target_named("Victim").unwrap();
    driver.cast_at("Path to Exile", &[victim]).unwrap();
    r5519_resolve(&mut driver);
    assert_eq!(r5519_forests(&driver, foe), 1);
}
