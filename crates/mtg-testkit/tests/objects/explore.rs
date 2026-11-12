//! Goal: Seekers' Squire, Siren Lookout and the other explore cards play whole at tables of two, three and four seats, as their cards say: the revealed land goes to the hand, any other card gives the creature a +1/+1 counter (more under a replacement that adds counters) and may go to the graveyard, an empty library still gives the counter, a creature that is gone when the trigger resolves gets nothing, and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::{name_of, pid};
use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{forest, island, land};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const LIBRARY: [&str; 3] = ["Grizzly Bears", "Serra Angel", "Counterspell"];

fn named(list: &[&str]) -> Vec<Arc<CardDefinition>> {
    list.iter().map(|name| card(name).unwrap()).collect()
}

/// Seat 0 holds `creature` and controls four lands of `color`, `extra` besides; every seat has `top` above the library.
fn table(
    seats: usize,
    creature: &str,
    color: Color,
    top: Vec<Arc<CardDefinition>>,
    extra: &[&str],
) -> Scenario {
    let mut lands: Vec<_> = (0..4).map(|_| ready(land(color))).collect();
    lands.extend(extra.iter().map(|name| ready(card(name).unwrap())));
    let mut library = top;
    library.extend(named(&LIBRARY));
    let mut scenario = Scenario::new(seats)
        .hand(0, [card(creature).unwrap()])
        .battlefield(0, lands);
    for seat in 0..seats {
        scenario = scenario.library(seat, library.clone());
    }
    scenario
}

fn passes_until_asked(driver: &mut Driver) -> Result<(), ScriptError> {
    while matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        && !driver.state().stack().is_empty()
    {
        driver.pass()?;
    }
    Ok(())
}

fn plus_counters(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::PlusOnePlusOne)
}

fn zone_names(driver: &Driver, zone: Zone) -> Vec<String> {
    driver
        .state()
        .zone(zone)
        .iter()
        .map(|id| name_of(driver.state(), *id))
        .collect()
}

fn explored(events: &[Event]) -> usize {
    events
        .iter()
        .filter(|event| matches!(event, Event::Explored { .. }))
        .count()
}

#[rstest]
fn a_land_that_is_revealed_goes_to_the_hand_and_nothing_is_asked(
    #[values(2, 3, 4)] seats: usize,
    #[values(
        ("Seekers' Squire", Color::Black),
        ("Siren Lookout", Color::Blue),
        ("Merfolk Branchwalker", Color::Green),
        ("Dire Fleet Interloper", Color::Black)
    )]
    creature: (&str, Color),
) -> Result<(), ScriptError> {
    let (creature, color) = creature;
    let scenario = table(seats, creature, color, vec![forest()], &[]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast(creature)?;
    passes_until_asked(&mut driver)?;
    assert!(matches!(driver.decision()?.kind, DecisionKind::Priority(_)));
    assert_eq!(plus_counters(&driver, creature), 0);
    assert_eq!(zone_names(&driver, Zone::Hand(pid(0))), ["Forest"]);
    assert_eq!(
        zone_names(&driver, Zone::Library(pid(0))).len(),
        LIBRARY.len()
    );
    assert_eq!(explored(driver.engine().events()), 1);
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}

#[rstest]
fn a_card_that_is_not_a_land_gives_a_counter_and_goes_to_the_graveyard_when_the_player_says_so(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] graveyard: bool,
    #[values(
        ("Seekers' Squire", Color::Black),
        ("Siren Lookout", Color::Blue),
        ("Merfolk Branchwalker", Color::Green),
        ("Dire Fleet Interloper", Color::Black)
    )]
    creature: (&str, Color),
) -> Result<(), ScriptError> {
    let (creature, color) = creature;
    let scenario = table(seats, creature, color, Vec::new(), &[]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast(creature)?;
    passes_until_asked(&mut driver)?;
    let decision = driver.decision()?.clone();
    let DecisionKind::ChooseCards(options) = &decision.kind else {
        panic!("the explore asks about the card");
    };
    assert_eq!(decision.player, pid(0));
    assert_eq!(
        options
            .from
            .iter()
            .map(|id| name_of(driver.state(), *id))
            .collect::<Vec<_>>(),
        ["Grizzly Bears"]
    );
    assert_eq!((options.min, options.max), (0, 1));
    let chosen: Vec<ObjectId> = if graveyard {
        options.from.clone()
    } else {
        Vec::new()
    };
    driver.answer(Answer::ChooseCards(CardsChoice(chosen)))?;
    passes_until_asked(&mut driver)?;
    assert_eq!(plus_counters(&driver, creature), 1);
    let library = zone_names(&driver, Zone::Library(pid(0)));
    assert_eq!(
        zone_names(&driver, Zone::Graveyard(pid(0))),
        if graveyard {
            vec!["Grizzly Bears"]
        } else {
            vec![]
        }
    );
    assert_eq!(library.len(), LIBRARY.len() - usize::from(graveyard));
    assert_eq!(
        zone_names(&driver, Zone::Hand(pid(0))),
        Vec::<String>::new()
    );
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}

#[rstest]
fn an_empty_library_still_gives_the_counter(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .hand(0, [card("Siren Lookout").unwrap()])
        .battlefield(0, (0..4).map(|_| ready(island())).collect::<Vec<_>>());
    for seat in 0..seats {
        scenario = scenario.library(seat, Vec::new());
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Siren Lookout")?;
    passes_until_asked(&mut driver)?;
    assert_eq!(plus_counters(&driver, "Siren Lookout"), 1);
    assert_eq!(explored(driver.engine().events()), 1);
    Ok(())
}

#[rstest]
fn a_replacement_that_adds_counters_adds_to_the_counter_an_explore_gives(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(
        seats,
        "Siren Lookout",
        Color::Blue,
        Vec::new(),
        &["Hardened Scales"],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Siren Lookout")?;
    passes_until_asked(&mut driver)?;
    driver.answer(Answer::ChooseCards(CardsChoice(Vec::new())))?;
    passes_until_asked(&mut driver)?;
    assert_eq!(plus_counters(&driver, "Siren Lookout"), 2);
    Ok(())
}

#[rstest]
fn a_creature_that_is_destroyed_with_its_trigger_on_the_stack_gets_no_counter(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = table(seats, "Siren Lookout", Color::Blue, Vec::new(), &[])
        .hand(1, [card("Lightning Bolt").unwrap()])
        .battlefield(1, [ready(land(Color::Red))]);
    for seat in 1..seats {
        scenario = scenario.library(seat, named(&LIBRARY));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Siren Lookout")?;
    // The creature resolves; its trigger waits on the stack and the opponent answers it.
    for _ in 0..=seats {
        driver.pass()?;
    }
    let target = driver.target_named("Siren Lookout")?;
    driver.cast_at("Lightning Bolt", &[target])?;
    passes_until_asked(&mut driver)?;
    while matches!(driver.decision()?.kind, DecisionKind::ChooseCards(_)) {
        driver.answer(Answer::ChooseCards(CardsChoice(Vec::new())))?;
        passes_until_asked(&mut driver)?;
    }
    assert!(
        driver.id_of("Siren Lookout").is_err() || {
            let id = driver.id_of("Siren Lookout")?;
            driver.state().object(id).unwrap().zone() != Zone::Battlefield
        }
    );
    let counters: u32 = driver
        .state()
        .objects()
        .filter_map(|o| o.permanent())
        .map(|p| p.counters_of(&CounterKind::PlusOnePlusOne))
        .sum();
    assert_eq!(counters, 0);
    Ok(())
}
