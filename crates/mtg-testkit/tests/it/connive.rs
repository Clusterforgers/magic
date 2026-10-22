//! Goal: Raffine's Informant, Echo Inspector and the other connive cards play whole at tables of two, three and four seats, as their cards say: the controller draws a card and discards a card of their choice from the hand and the card just drawn, a nonland discard gives the creature a +1/+1 counter (more under a replacement that adds counters) and a land does not, a creature that is gone when the trigger resolves gets nothing, and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::{name_of, pid};
use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::color::Color;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{forest, land};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const LIBRARY: [&str; 2] = ["Serra Angel", "Counterspell"];

/// Seat 0 holds `creature` and a Counterspell, controls four lands of `color`, `extra` besides; every seat has `top` above
/// the library.
fn table(
    seats: usize,
    creature: &str,
    color: Color,
    top: Arc<CardDefinition>,
    extra: &[&str],
) -> Scenario {
    let mut lands: Vec<_> = (0..4).map(|_| ready(land(color))).collect();
    lands.extend(extra.iter().map(|name| ready(card(name).unwrap())));
    let mut library = vec![top];
    library.extend(LIBRARY.iter().map(|name| card(name).unwrap()));
    let mut scenario = Scenario::new(seats)
        .hand(0, [card(creature).unwrap(), card("Counterspell").unwrap()])
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

#[rstest]
fn the_controller_draws_and_discards_a_card_of_their_choice_and_a_nonland_discard_gives_a_counter(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] land_on_top: bool,
    #[values(true, false)] discard_the_drawn_card: bool,
    #[values(
        ("Raffine's Informant", Color::White),
        ("Red Room Recruit", Color::Black),
        ("Revel Ruiner", Color::Black),
        ("Echo Inspector", Color::Blue)
    )]
    creature: (&str, Color),
) -> Result<(), ScriptError> {
    let (creature, color) = creature;
    let (top, drawn) = if land_on_top {
        (forest(), "Forest")
    } else {
        (card("Grizzly Bears").unwrap(), "Grizzly Bears")
    };
    let scenario = table(seats, creature, color, top, &[]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast(creature)?;
    passes_until_asked(&mut driver)?;
    let decision = driver.decision()?.clone();
    let DecisionKind::ChooseCards(options) = &decision.kind else {
        panic!("the connive asks which card to discard");
    };
    assert_eq!(decision.player, pid(0));
    let mut offered: Vec<String> = options
        .from
        .iter()
        .map(|id| name_of(driver.state(), *id))
        .collect();
    offered.sort();
    let mut expected = vec!["Counterspell".to_owned(), drawn.to_owned()];
    expected.sort();
    assert_eq!(offered, expected);
    assert_eq!((options.min, options.max), (1, 1));
    let discard = if discard_the_drawn_card {
        drawn
    } else {
        "Counterspell"
    };
    let chosen = options
        .from
        .iter()
        .copied()
        .find(|id| name_of(driver.state(), *id) == discard)
        .unwrap();
    driver.answer(Answer::ChooseCards(CardsChoice(vec![chosen])))?;
    passes_until_asked(&mut driver)?;
    let nonland = discard != "Forest";
    assert_eq!(plus_counters(&driver, creature), u32::from(nonland));
    assert_eq!(zone_names(&driver, Zone::Graveyard(pid(0))), [discard]);
    let kept = if discard_the_drawn_card {
        "Counterspell"
    } else {
        drawn
    };
    assert_eq!(zone_names(&driver, Zone::Hand(pid(0))), [kept]);
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}

#[rstest]
fn a_replacement_that_adds_counters_adds_to_the_counter_a_connive_gives(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(
        seats,
        "Echo Inspector",
        Color::Blue,
        card("Grizzly Bears").unwrap(),
        &["Hardened Scales"],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Echo Inspector")?;
    passes_until_asked(&mut driver)?;
    let DecisionKind::ChooseCards(options) = &driver.decision()?.clone().kind else {
        panic!("the connive asks which card to discard");
    };
    driver.answer(Answer::ChooseCards(CardsChoice(vec![options.from[0]])))?;
    passes_until_asked(&mut driver)?;
    assert_eq!(plus_counters(&driver, "Echo Inspector"), 2);
    Ok(())
}

#[rstest]
fn a_creature_that_is_destroyed_with_its_trigger_on_the_stack_gets_no_counter(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = table(
        seats,
        "Echo Inspector",
        Color::Blue,
        card("Grizzly Bears").unwrap(),
        &[],
    )
    .hand(1, [card("Lightning Bolt").unwrap()])
    .battlefield(1, [ready(land(Color::Red))]);
    for seat in 1..seats {
        scenario = scenario.library(seat, vec![card("Grizzly Bears").unwrap(); 3]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Echo Inspector")?;
    for _ in 0..=seats {
        driver.pass()?;
    }
    let target = driver.target_named("Echo Inspector")?;
    driver.cast_at("Lightning Bolt", &[target])?;
    passes_until_asked(&mut driver)?;
    while let DecisionKind::ChooseCards(options) = &driver.decision()?.clone().kind {
        driver.answer(Answer::ChooseCards(CardsChoice(vec![options.from[0]])))?;
        passes_until_asked(&mut driver)?;
    }
    let counters: u32 = driver
        .state()
        .objects()
        .filter_map(mtg_engine::object::GameObject::permanent)
        .map(|p| p.counters_of(&CounterKind::PlusOnePlusOne))
        .sum();
    assert_eq!(counters, 0);
    assert!(
        driver
            .engine()
            .events()
            .iter()
            .any(|event| matches!(event, Event::CardDiscarded { .. })),
        "the connive still draws and discards"
    );
    Ok(())
}
