//! Goal: "if you do" after a step that is not a "may" asks what the step did: a land that sacrifices itself and then searches (Obscura Storefront, Maestros Theater), a payment of a land that buys a search (Springbloom Druid), a sacrifice of the card followed by a search of creatures only while an opponent controls three or more (Defense of the Heart) and a removal of every counter whose absence puts one back (Bounty of the Luxa) play as printed at tables of two, three and four seats, and a table that leaves the condition false changes nothing.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::{names, on_battlefield, pid};
use mtg_cards::value::CounterKind;
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{forest, free_creature, island, mountain, plains, swamp};
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn assert_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

fn count(driver: &Driver, zone: Zone, name: &str) -> usize {
    names(driver.state(), zone)
        .iter()
        .filter(|found| found.as_str() == name)
        .count()
}

/// Passes priority until the stack is empty, answering a yes-or-no question with `yes` and a choice of cards with as
/// many of the first cards on offer as it allows.
fn settle(driver: &mut Driver, yes: bool) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        match driver.decision()?.kind.clone() {
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(yes)))?;
            }
            DecisionKind::ChooseCards(options) => {
                let most = usize::try_from(options.max).unwrap();
                let first = options.from.iter().copied().take(most).collect();
                driver.answer(Answer::ChooseCards(CardsChoice(first)))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn library_of(kinds: &[&str]) -> Vec<std::sync::Arc<mtg_cards::CardDefinition>> {
    kinds
        .iter()
        .map(|kind| match *kind {
            "Forest" => forest(),
            "Island" => island(),
            "Mountain" => mountain(),
            "Plains" => plains(),
            _ => swamp(),
        })
        .collect()
}

#[rstest]
#[case::storefront_fetches_a_swamp("Obscura Storefront", &["Forest", "Swamp"], Some("Swamp"))]
#[case::storefront_has_no_mountain_in_its_list("Obscura Storefront", &["Forest", "Mountain"], None)]
#[case::theater_fetches_a_mountain("Maestros Theater", &["Plains", "Mountain"], Some("Mountain"))]
#[case::theater_has_no_plains_in_its_list("Maestros Theater", &["Plains", "Forest"], None)]
fn a_land_sacrifices_itself_and_then_searches_and_gains_life(
    #[case] land: &str,
    #[case] library: &[&str],
    #[case] fetched: Option<&str>,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand_named(0, [land])
        .library(0, library_of(library));
    let mut driver = Driver::from_scenario(scenario)?;
    driver.play_land(land)?;
    settle(&mut driver, true)?;
    assert_eq!(count(&driver, Zone::Graveyard(pid(0)), land), 1);
    assert!(on_battlefield(driver.state(), land).is_none());
    // The life comes with the search, whether it found a land or not.
    assert_eq!(life(&driver, 0), 21);
    for seat in 1..seats {
        assert_eq!(life(&driver, seat), 20);
    }
    match fetched {
        Some(name) => assert_eq!(count(&driver, Zone::Battlefield, name), 1),
        None => assert_eq!(names(driver.state(), Zone::Battlefield).len(), 0),
    }
    assert_replays(&driver);
    Ok(())
}

#[rstest]
#[case::paid(true, 4, 1)]
#[case::declined(false, 3, 3)]
fn a_land_sacrificed_for_a_search_is_asked_and_only_a_paid_search_runs(
    #[case] yes: bool,
    #[case] forests_after: usize,
    #[case] library_after: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand_named(0, ["Springbloom Druid"])
        .battlefield(0, [ready(forest()), ready(forest()), ready(forest())])
        .library(0, library_of(&["Forest", "Forest", "Forest"]));
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Springbloom Druid")?;
    settle(&mut driver, yes)?;
    assert_eq!(count(&driver, Zone::Battlefield, "Forest"), forests_after);
    assert_eq!(
        names(driver.state(), Zone::Library(pid(0))).len(),
        library_after
    );
    assert_replays(&driver);
    Ok(())
}

#[rstest]
#[case::two_creatures_are_not_three(&[2, 0, 0])]
#[case::three_creatures_are(&[3, 0, 0])]
#[case::three_in_the_second_opponents_hand(&[1, 3, 0])]
#[case::two_each_are_not_three_together(&[2, 2, 2])]
fn a_search_runs_only_while_an_opponent_controls_three_creatures(
    #[case] opponents: &[usize],
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Untap)
        .battlefield_named(0, ["Defense of the Heart"])
        .library(
            0,
            [
                free_creature("Boar", 3, 3),
                free_creature("Wolf", 2, 2),
                free_creature("Newt", 1, 1),
            ],
        );
    for seat in 1..seats {
        let creatures = (0..opponents[seat - 1])
            .map(|n| ready(free_creature(&format!("Guard {seat}{n}"), 1, 1)))
            .collect::<Vec<_>>();
        scenario = scenario.battlefield(seat, creatures);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.idle_until(2, Step::Upkeep)?;
    settle(&mut driver, true)?;
    let in_play = count(&driver, Zone::Battlefield, "Defense of the Heart");
    let found = ["Boar", "Wolf", "Newt"]
        .iter()
        .map(|name| count(&driver, Zone::Battlefield, name))
        .sum::<usize>();
    // Only the seats at the table count: a list longer than the opponents leaves its last entries out.
    let three_held = (1..seats).any(|seat| opponents[seat - 1] >= 3);
    assert_eq!(in_play, usize::from(!three_held));
    assert_eq!(found, if three_held { 2 } else { 0 });
    assert_replays(&driver);
    Ok(())
}

fn flood(driver: &Driver) -> u32 {
    let id = on_battlefield(driver.state(), "Bounty of the Luxa").unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |held| {
            held.counters_of(&CounterKind::Other("flood".into()))
        })
}

#[rstest]
#[case::empty_adds_a_counter_and_draws(0, 1, 1, 0)]
#[case::one_counter_is_taken_and_mana_comes(1, 0, 0, 3)]
#[case::three_counters_are_all_taken(3, 0, 0, 3)]
fn bounty_of_the_luxa_alternates_between_a_counter_with_a_card_and_mana(
    #[case] before: u32,
    #[case] after: u32,
    #[case] drawn: usize,
    #[case] mana: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats).at(2, 0, Step::Upkeep).battlefield(
        0,
        [with_counters(
            ready(mtg_testkit::corpus::card("Bounty of the Luxa").unwrap()),
            CounterKind::Other("flood".into()),
            before,
        )],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    let hand = names(driver.state(), Zone::Hand(pid(0))).len();
    driver.idle_until(2, Step::Main1)?;
    let hand_in_main = names(driver.state(), Zone::Hand(pid(0))).len();
    settle(&mut driver, true)?;
    assert_eq!(flood(&driver), after);
    assert_eq!(
        names(driver.state(), Zone::Hand(pid(0))).len() - hand_in_main,
        drawn
    );
    let _ = hand;
    let added = driver
        .engine()
        .events()
        .iter()
        .filter(|event| matches!(event, Event::ManaAdded { .. }))
        .count();
    assert_eq!(added, mana);
    assert_replays(&driver);
    Ok(())
}

#[rstest]
fn bounty_of_the_luxa_waits_for_its_controllers_first_main_phase(
    #[values(3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(2, 1, Step::Upkeep)
        .battlefield_named(0, ["Bounty of the Luxa"]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.idle_until(2, Step::Main1)?;
    settle(&mut driver, true)?;
    assert_eq!(flood(&driver), 0);
    assert_replays(&driver);
    Ok(())
}
