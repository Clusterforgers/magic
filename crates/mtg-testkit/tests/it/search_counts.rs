//! Goal: a search whose count a where clause names (Traverse the Outlands: the greatest power among your creatures; Harvest Season: your tapped creatures) asks for up to that many basic land cards at tables of two, three and four seats, with none, one and many cards: nothing to search for with a count of zero, the count as the most the searcher may choose, every land chosen on the battlefield tapped and out of the library, the rest of the library kept, and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, tapped};
use rstest::rstest;

/// Seat 0 holds `spell` over five Forests, controls creatures of these (power, tapped) and has six Forests and four
/// Islands in its library; the other seats have a library.
fn table(seats: usize, spell: &str, creatures: &[(i32, bool)]) -> Scenario {
    let mut mine: Vec<_> = (0..5).map(|_| ready(forest())).collect();
    mine.extend(
        creatures
            .iter()
            .enumerate()
            .map(|(at, (power, is_tapped))| {
                let creature = free_creature(&format!("Fodder {at}"), *power, 1);
                if *is_tapped {
                    tapped(creature)
                } else {
                    ready(creature)
                }
            }),
    );
    let library: Vec<_> = (0..6)
        .map(|_| forest())
        .chain((0..4).map(|_| island()))
        .collect();
    let mut scenario = Scenario::new(seats)
        .hand(0, [card(spell).unwrap()])
        .battlefield(0, mine)
        .library(0, library);
    for seat in 1..seats {
        scenario = scenario.library(seat, (0..8).map(|_| island()));
    }
    scenario
}

/// The basic lands seat 0 controls and how many of the Islands among them are tapped (only found lands are Islands).
fn lands_in_play(driver: &Driver) -> (usize, usize) {
    let state = driver.state();
    let lands: Vec<_> = state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|object| matches!(object.card().name.as_str(), "Forest" | "Island"))
        .collect();
    let islands = lands.iter().filter(|object| object.card().name == "Island");
    let tapped = islands
        .filter(|object| {
            object
                .permanent()
                .is_some_and(mtg_engine::object::PermanentState::tapped)
        })
        .count();
    (lands.len(), tapped)
}

#[rstest]
#[case::no_creature("Traverse the Outlands", &[], 0)]
#[case::one_creature("Traverse the Outlands", &[(3, false)], 3)]
#[case::the_greatest_of_many("Traverse the Outlands", &[(2, false), (5, true), (1, false)], 5)]
#[case::no_tapped_creature("Harvest Season", &[(4, false)], 0)]
#[case::one_tapped_creature("Harvest Season", &[(4, true), (2, false)], 1)]
#[case::many_tapped_creatures("Harvest Season", &[(1, true), (1, true), (1, true), (9, false)], 3)]
fn the_count_a_where_clause_names_is_the_most_that_may_be_found(
    #[case] spell: &str,
    #[case] creatures: &[(i32, bool)],
    #[case] count: u32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let setup = table(seats, spell, creatures).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, spell, creatures))?;
    let before = driver.state().zone(Zone::Library(pid(0))).len();
    driver.begin_cast(spell)?;
    let cost: Vec<&str> = vec!["Forest"; if spell == "Harvest Season" { 3 } else { 5 }];
    driver.pay_tapping(&cost, 0)?;
    let mut asked = false;
    for _ in 0..40 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        if let DecisionKind::ChooseCards(options) = &driver.decision()?.kind {
            asked = true;
            assert_eq!(options.max, count, "the most the searcher may find");
            assert_eq!(options.min, 0);
            // Only basic land cards are offered, and the Islands are among them for no one.
            assert!(options.from.len() >= count as usize);
            let all: Vec<_> = options.from.iter().copied().take(count as usize).collect();
            driver.answer(Answer::ChooseCards(CardsChoice(all)))?;
        } else {
            driver.answer_passively()?;
        }
    }
    assert_eq!(asked, count > 0, "nothing is asked for a count of zero");
    let (lands, tapped_islands) = lands_in_play(&driver);
    assert_eq!(lands, 5 + count as usize, "every land found is in play");
    let islands = lands
        - driver
            .state()
            .zone(Zone::Battlefield)
            .iter()
            .filter(|id| {
                driver
                    .state()
                    .object(**id)
                    .is_some_and(|object| object.card().name == "Forest")
            })
            .count();
    assert_eq!(tapped_islands, islands, "the lands found came in tapped");
    let after = driver.state().zone(Zone::Library(pid(0))).len();
    assert_eq!(after, before - count as usize, "the lands left the library");
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}
