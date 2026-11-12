//! Goal: crimes and the clash wordings play whole at tables of two, three and four seats: Bandit's Haul gets a loot counter for its controller's first crime of a turn only, Patrolling Peacemaker proliferates when an opponent targets what its controller controls and not when they target their own, Whirlpool Whelm returns its target or, on a won clash, may put it on top of its owner's library, Pollen Lullaby holds the clashed player's creatures through their next untap step only on a win, and each log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_engine::choose::{CardsChoice, OptionChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, cost, free_creature, island, mountain, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, tapped};
use rstest::rstest;

use crate::common::{name_of, pid};

/// A vanilla creature card of mana value `value`.
fn w11c_costing(value: u32) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(format!("Cost {value}"))
            .with_type(CardType::Creature)
            .with_mana_cost(cost(value, &[]))
            .with_power_toughness(1, 1),
    )
}

/// Seat 0's library is eight cards of mana value `mine`, every other library eight of mana value `theirs`.
fn w11c_libraries(mut scenario: Scenario, seats: usize, mine: u32, theirs: u32) -> Scenario {
    for seat in 0..seats {
        let value = if seat == 0 { mine } else { theirs };
        scenario = scenario.library(seat, (0..8).map(|_| w11c_costing(value)));
    }
    scenario
}

/// Passes and pays until the stack is empty: picks the last option, every card offered, and answers the yes/no
/// questions with `yes` in order (no once they run out). The number of yes/no questions asked.
fn w11c_resolve(driver: &mut Driver, yes: &[bool]) -> Result<usize, ScriptError> {
    let mut asked = 0;
    for _ in 0..80 {
        match &driver.decision()?.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(asked),
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::ChooseOption(options) => {
                let last = options.options.len() - 1;
                driver.answer(Answer::ChooseOption(OptionChoice(last)))?;
            }
            DecisionKind::ChooseCards(options) => {
                let all = options.from.clone();
                driver.answer(Answer::ChooseCards(CardsChoice(all)))?;
            }
            DecisionKind::YesNo(_) => {
                let answer = yes.get(asked).copied().unwrap_or(false);
                asked += 1;
                driver.answer(Answer::YesNo(YesNoChoice(answer)))?;
            }
            other => panic!("unexpected {}", other.name()),
        }
    }
    panic!("the stack did not empty");
}

/// The counters on the permanent named `name`.
fn w11c_counters(driver: &Driver, name: &str) -> u32 {
    let state = driver.state();
    let id = state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| name_of(state, *id) == name)
        .unwrap();
    let permanent = state.object(id).unwrap().permanent().unwrap();
    permanent.counters().map(|(_, n)| n).sum()
}

fn w11c_replays(driver: &Driver, scenario: Scenario) {
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

#[rstest]
#[case::theirs_then_theirs(&["Grizzly Bears", "Ogre"], 1)]
#[case::mine_then_theirs(&["Wolf", "Ogre"], 1)]
#[case::mine_twice(&["Wolf", "Mine Too"], 0)]
fn bandits_haul_counts_the_first_crime_of_a_turn(
    #[values(2, 3, 4)] seats: usize,
    #[case] victims: &[&str],
    #[case] loot: u32,
) -> Result<(), ScriptError> {
    let scenario = || {
        let mut lands: Vec<_> = (0..6).map(|_| ready(mountain())).collect();
        lands.push(ready(card("Bandit's Haul").unwrap()));
        lands.push(ready(free_creature("Wolf", 5, 5)));
        lands.push(ready(free_creature("Mine Too", 5, 5)));
        let scenario = Scenario::new(seats)
            .hand(0, [card("Lash Out").unwrap(), card("Lash Out").unwrap()])
            .battlefield(0, lands)
            .battlefield(1, [ready(bear()), ready(free_creature("Ogre", 5, 5))]);
        w11c_libraries(scenario, seats, 1, 1)
    };
    let mut driver = Driver::from_scenario(scenario())?;
    for victim in victims {
        let target = driver.target_named(victim)?;
        driver.cast_at("Lash Out", &[target])?;
        w11c_resolve(&mut driver, &[])?;
    }
    assert_eq!(w11c_counters(&driver, "Bandit's Haul"), loot);
    w11c_replays(&driver, scenario());
    Ok(())
}

#[rstest]
#[case::an_opponents_crime("Wolf", 3)]
#[case::their_own_creature("Ogre", 2)]
fn patrolling_peacemaker_proliferates_on_an_opponents_crime(
    #[values(2, 3, 4)] seats: usize,
    #[case] victim: &str,
    #[case] counters: u32,
) -> Result<(), ScriptError> {
    let scenario = || {
        let mut mine: Vec<_> = (0..3).map(|_| ready(plains())).collect();
        mine.push(ready(free_creature("Wolf", 5, 5)));
        let mut theirs: Vec<_> = (0..6).map(|_| ready(mountain())).collect();
        theirs.push(ready(free_creature("Ogre", 5, 5)));
        let scenario = Scenario::new(seats)
            .hand(0, [card("Patrolling Peacemaker").unwrap()])
            .hand(1, [card("Lash Out").unwrap()])
            .battlefield(0, mine)
            .battlefield(1, theirs);
        w11c_libraries(scenario, seats, 1, 1)
    };
    let mut driver = Driver::from_scenario(scenario())?;
    driver.cast("Patrolling Peacemaker")?;
    w11c_resolve(&mut driver, &[])?;
    assert_eq!(w11c_counters(&driver, "Patrolling Peacemaker"), 2);
    driver.pass()?;
    assert_eq!(driver.decision()?.player, pid(1));
    let target = driver.target_named(victim)?;
    driver.cast_at("Lash Out", &[target])?;
    w11c_resolve(&mut driver, &[])?;
    assert_eq!(w11c_counters(&driver, "Patrolling Peacemaker"), counters);
    w11c_replays(&driver, scenario());
    Ok(())
}

#[rstest]
#[case::won_and_on_top(5, true, true)]
#[case::won_and_returned(5, false, false)]
#[case::lost(0, true, false)]
fn whirlpool_whelm_may_put_the_creature_on_top_on_a_win(
    #[values(2, 3, 4)] seats: usize,
    #[case] mine: u32,
    #[case] may: bool,
    #[case] on_top: bool,
) -> Result<(), ScriptError> {
    let scenario = || {
        let lands: Vec<_> = (0..4).map(|_| ready(island())).collect();
        let scenario = Scenario::new(seats)
            .hand(0, [card("Whirlpool Whelm").unwrap()])
            .battlefield(0, lands)
            .battlefield(1, [ready(bear())]);
        w11c_libraries(scenario, seats, mine, 1)
    };
    let mut driver = Driver::from_scenario(scenario())?;
    let target = driver.target_named("Grizzly Bears")?;
    driver.cast_at("Whirlpool Whelm", &[target])?;
    // The two clash questions keep both cards on top; a won clash then asks the may.
    let asked = w11c_resolve(&mut driver, &[false, false, may])?;
    assert_eq!(asked, if mine > 1 { 3 } else { 2 });
    let state = driver.state();
    let top = state.zone(Zone::Library(pid(1))).last().copied().unwrap();
    assert_eq!(name_of(state, top) == "Grizzly Bears", on_top);
    let in_hand = state
        .zone(Zone::Hand(pid(1)))
        .iter()
        .any(|id| name_of(state, *id) == "Grizzly Bears");
    assert_eq!(in_hand, !on_top);
    w11c_replays(&driver, scenario());
    Ok(())
}

#[rstest]
#[case::won(5, true)]
#[case::lost(0, false)]
fn pollen_lullaby_holds_the_clashed_players_creatures_on_a_win(
    #[values(2, 3, 4)] seats: usize,
    #[case] mine: u32,
    #[case] held: bool,
) -> Result<(), ScriptError> {
    let scenario = || {
        let lands: Vec<_> = (0..4).map(|_| ready(plains())).collect();
        let mut scenario = Scenario::new(seats)
            .hand(0, [card("Pollen Lullaby").unwrap()])
            .battlefield(0, lands);
        for seat in 1..seats {
            scenario = scenario.battlefield(seat, [tapped(bear())]);
        }
        w11c_libraries(scenario, seats, mine, 1)
    };
    let mut driver = Driver::from_scenario(scenario())?;
    driver.cast("Pollen Lullaby")?;
    w11c_resolve(&mut driver, &[])?;
    driver.idle_until(2, Step::Upkeep)?;
    let state = driver.state();
    assert_eq!(state.turn().active(), pid(1));
    let bear = state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| state.object(*id).unwrap().controller() == pid(1))
        .unwrap();
    let still_tapped = state.object(bear).unwrap().permanent().unwrap().tapped();
    // The opponent clashed with is the last one offered: seat 1 only at two seats.
    assert_eq!(still_tapped, held && seats == 2);
    w11c_replays(&driver, scenario());
    Ok(())
}
