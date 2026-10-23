//! Goal: a mana ability whose cost taps another untapped permanent or sacrifices one (Springleaf Drum, Jaspera Sentinel, Holdout Settlement, Skirk Prospector) is a source of a payment: the engine picks the permanents, never one that is tapped, never one twice, never one that another source of the payment uses for its mana, a summoning-sick creature may be tapped, only the payer's permanents of the battlefield are offered, and whole bot games with these sources finish and keep every invariant at tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::{Arc, Mutex};

use common::pid;
use common::seeds::for_each_seed;
use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::decision::DecisionKind;
use mtg_engine::events::ZoneCause;
use mtg_engine::format::Normal;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameSetup};
use mtg_testkit::cards::{forest, island, mountain, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{
    Driver, PlayOptions, Scenario, ScriptError, play_game_with, ready, sick, tapped,
};
use rstest::rstest;

fn definition(name: &str) -> Arc<CardDefinition> {
    match name {
        "Forest" => forest(),
        "Island" => island(),
        "Mountain" => mountain(),
        "Plains" => plains(),
        "Swamp" => swamp(),
        other => card(other).unwrap(),
    }
}

fn spell(cost: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Spell")
            .with_type(CardType::Instant)
            .with_mana_cost(ManaCost::parse(cost).unwrap()),
    )
}

/// A permanent by name; `sick:` and `tapped:` in front of it say how it starts.
fn permanent(spec: &str) -> mtg_engine::setup::PresetPermanent {
    match spec.split_once(':') {
        Some(("sick", name)) => sick(definition(name)),
        Some(("tapped", name)) => tapped(definition(name)),
        _ => ready(definition(spec)),
    }
}

/// Seat 0 holds two spells of `cost` and has `mine` in play; every seat has a library.
fn table(seats: usize, cost: &str, mine: &[&str]) -> Result<Driver, ScriptError> {
    let mut scenario = Scenario::new(seats)
        .hand(0, [spell(cost), spell(cost)])
        .battlefield(0, mine.iter().map(|spec| permanent(spec)));
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..12).map(|_| plains()));
    }
    Driver::from_scenario(scenario)
}

fn castable(driver: &Driver) -> bool {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => !options.castable.is_empty(),
        other => panic!("priority was expected, got {other:?}"),
    }
}

/// Casts a spell whenever one is castable and counts the casts.
fn casts(driver: &mut Driver) -> usize {
    let mut casts = 0;
    while castable(driver) {
        driver.cast("Spell").unwrap();
        casts += 1;
    }
    casts
}

#[rstest]
#[case::a_drum_taps_a_creature("{R}", &["Springleaf Drum", "Grizzly Bears"], 1)]
#[case::a_drum_needs_a_creature("{R}", &["Springleaf Drum"], 0)]
#[case::a_summoning_sick_creature_may_be_tapped("{R}", &["Springleaf Drum", "sick:Grizzly Bears"], 1)]
#[case::a_tapped_creature_may_not("{R}", &["Springleaf Drum", "tapped:Grizzly Bears"], 0)]
#[case::the_drum_does_not_tap_a_creature_twice("{R}{G}", &["Springleaf Drum", "Jaspera Sentinel", "Grizzly Bears"], 0)]
#[case::two_creatures_for_two_sources("{R}{G}", &["Springleaf Drum", "Jaspera Sentinel", "Grizzly Bears", "Grizzly Bears"], 1)]
#[case::the_sentinel_does_not_tap_itself_for_its_cost("{R}", &["Jaspera Sentinel"], 0)]
#[case::the_sentinel_with_a_creature("{R}", &["Jaspera Sentinel", "Grizzly Bears"], 1)]
#[case::a_source_is_not_tapped_for_the_cost_of_another("{R}{G}", &["Springleaf Drum", "Jaspera Sentinel"], 0)]
#[case::the_settlement_makes_a_colour_with_a_creature("{R}", &["Holdout Settlement", "Grizzly Bears"], 1)]
#[case::the_settlement_alone_makes_colourless("{1}", &["Holdout Settlement"], 1)]
#[case::the_settlement_alone_makes_no_colour("{R}", &["Holdout Settlement"], 0)]
#[case::a_prospector_sacrifices_itself("{R}", &["Skirk Prospector"], 1)]
#[case::a_goblin_for_two_prospectors("{R}{R}", &["Skirk Prospector", "Skirk Prospector"], 1)]
#[case::each_prospector_adds_one("{R}{R}{R}", &["Skirk Prospector", "Skirk Prospector"], 0)]
#[case::one_prospector_two_goblins("{R}{R}", &["Skirk Prospector", "Goblin Spy", "Putrid Goblin"], 1)]
#[case::one_prospector_three_goblins("{R}{R}{R}", &["Skirk Prospector", "Goblin Spy", "Putrid Goblin", "Boggart Ram-Gang"], 1)]
#[case::a_land_beside_the_uses("{R}{R}{R}", &["Skirk Prospector", "Goblin Spy", "Putrid Goblin", "Mountain"], 1)]
#[case::a_creature_that_is_no_goblin_is_kept("{R}{R}", &["Skirk Prospector", "Grizzly Bears", "Goblin Spy", "Putrid Goblin"], 1)]
#[case::each_use_needs_a_goblin("{R}{R}{R}{R}", &["Skirk Prospector", "Grizzly Bears", "Goblin Spy", "Putrid Goblin"], 0)]
#[case::a_prospector_sacrifices_the_cheaper_goblin("{R}", &["Skirk Prospector", "Boggart Ram-Gang"], 1)]
#[case::lands_pay_before_the_sources_that_take("{R}", &["Mountain", "Springleaf Drum", "Grizzly Bears"], 2)]
fn a_source_that_takes_another_permanent_pays_when_the_payment_can_take_it(
    #[case] cost: &str,
    #[case] mine: &[&str],
    #[case] expected: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, cost, mine)?;
    assert_eq!(casts(&mut driver), expected);
    Ok(())
}

#[rstest]
#[case::energy_and_a_creature(1, &["Aether Hub", "Springleaf Drum", "Grizzly Bears"], true)]
#[case::more_energy_than_needed(3, &["Aether Hub", "Springleaf Drum", "Grizzly Bears"], true)]
#[case::no_energy_no_second_colour(0, &["Aether Hub", "Springleaf Drum", "Grizzly Bears"], false)]
#[case::no_creature_to_tap(1, &["Aether Hub", "Springleaf Drum"], false)]
#[case::a_tapped_creature_is_not_there_to_tap(1, &["Aether Hub", "Springleaf Drum", "tapped:Grizzly Bears"], false)]
fn a_payment_may_pay_energy_and_take_a_permanent(
    #[case] energy: u32,
    #[case] mine: &[&str],
    #[case] castable_now: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let counter = mtg_cards::value::CounterKind::Other("energy".into());
    let mut scenario = Scenario::new(seats)
        .hand(0, [spell("{W}{U}")])
        .counters(0, counter.clone(), energy)
        .battlefield(0, mine.iter().map(|spec| permanent(spec)));
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..12).map(|_| plains()));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    assert_eq!(castable(&driver), castable_now);
    if castable_now {
        let events = driver.cast("Spell")?;
        assert_eq!(
            names(&driver, &events, false).len(),
            3,
            "the hub, the drum and the creature"
        );
        let left = driver.state().player(pid(0)).unwrap().counters_of(&counter);
        assert_eq!(left, energy - 1, "one energy for the hub");
    }
    Ok(())
}

fn names(driver: &Driver, events: &[Event], sacrificed: bool) -> Vec<String> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::PermanentTapped { object } if !sacrificed => Some(*object),
            Event::ZoneChanged {
                object,
                cause: ZoneCause::Sacrificed,
                ..
            } if sacrificed => Some(*object),
            _ => None,
        })
        .map(|id| driver.state().object(id).unwrap().card().name.clone())
        .collect()
}

#[test]
fn the_drum_taps_itself_and_the_creature_and_adds_one_mana() -> Result<(), ScriptError> {
    let mut driver = table(2, "{R}", &["Springleaf Drum", "Grizzly Bears"])?;
    let events = driver.cast("Spell")?;
    let mut tapped = names(&driver, &events, false);
    tapped.sort();
    assert_eq!(tapped, ["Grizzly Bears", "Springleaf Drum"]);
    let added = events
        .iter()
        .filter(|event| matches!(event, Event::ManaAdded { .. }))
        .count();
    assert_eq!(added, 1);
    Ok(())
}

#[test]
fn a_land_pays_before_a_source_that_taps_a_creature() -> Result<(), ScriptError> {
    let mut driver = table(2, "{R}", &["Mountain", "Springleaf Drum", "Grizzly Bears"])?;
    let events = driver.cast("Spell")?;
    assert_eq!(names(&driver, &events, false), ["Mountain"]);
    Ok(())
}

#[test]
fn the_prospector_sacrifices_the_goblin_it_must_and_adds_red() -> Result<(), ScriptError> {
    let mut driver = table(2, "{R}", &["Skirk Prospector"])?;
    let events = driver.cast("Spell")?;
    assert_eq!(names(&driver, &events, true), ["Skirk Prospector"]);
    let goblin = driver
        .state()
        .zone(Zone::Graveyard(pid(0)))
        .iter()
        .any(|id| driver.state().object(*id).unwrap().card().name == "Skirk Prospector");
    assert!(goblin);
    Ok(())
}

#[rstest]
fn the_prospector_sacrifices_a_different_goblin_for_each_use(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(
        seats,
        "{R}{R}",
        &["Skirk Prospector", "Grizzly Bears", "Goblin Spy", "Putrid Goblin"],
    )?;
    let events = driver.cast("Spell")?;
    let mut gone = names(&driver, &events, true);
    gone.sort();
    assert_eq!(gone, ["Goblin Spy", "Putrid Goblin"], "never itself or the Bears");
    let added = events
        .iter()
        .filter(|event| matches!(event, Event::ManaAdded { .. }))
        .count();
    assert_eq!(added, 2, "one red for each use");
    Ok(())
}

#[test]
fn a_creature_that_makes_mana_is_not_also_taken_by_a_cost() -> Result<(), ScriptError> {
    // The only creature is also a source of the payment: the Drum cannot tap it.
    let mut driver = table(2, "{R}{G}", &["Springleaf Drum", "Llanowar Elves"])?;
    assert_eq!(casts(&mut driver), 0);
    Ok(())
}

#[test]
fn only_permanents_of_the_payer_on_the_battlefield_are_offered_to_take() -> Result<(), ScriptError>
{
    let scenario = Scenario::new(2)
        .hand(
            0,
            [
                spell("{R}"),
                card("Boggart Ram-Gang").unwrap(),
                card("Grizzly Bears").unwrap(),
            ],
        )
        .library(0, (0..6).map(|_| card("Boggart Ram-Gang").unwrap()))
        .library(1, (0..6).map(|_| plains()))
        .graveyard(0, [card("Boggart Ram-Gang").unwrap()])
        .battlefield(
            0,
            [
                ready(card("Skirk Prospector").unwrap()),
                ready(card("Springleaf Drum").unwrap()),
                ready(card("Grizzly Bears").unwrap()),
            ],
        )
        .battlefield(
            1,
            [
                ready(card("Grizzly Bears").unwrap()),
                ready(card("Boggart Ram-Gang").unwrap()),
            ],
        );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Spell")?;
    let options = driver.pay_options()?;
    let offered: Vec<_> = options
        .sources
        .iter()
        .flat_map(|source| {
            source
                .takes
                .iter()
                .flat_map(|take| take.from.iter().copied())
        })
        .collect();
    assert_ne!(offered.len(), 0, "the sources name what they may take");
    for id in offered {
        let object = driver.state().object(id).unwrap();
        assert_eq!(object.zone(), Zone::Battlefield, "{}", object.card().name);
        assert_eq!(object.controller(), pid(0), "{}", object.card().name);
        // What it names is on the table, which every seat sees.
        assert!(driver.state().may_look_at(pid(1), id));
    }
    Ok(())
}

fn deck() -> Vec<Arc<CardDefinition>> {
    [
        ("Forest", 7),
        ("Island", 5),
        ("Mountain", 7),
        ("Plains", 5),
        ("Swamp", 4),
        ("Springleaf Drum", 3),
        ("Jaspera Sentinel", 3),
        ("Holdout Settlement", 3),
        ("Skirk Prospector", 4),
        ("Boggart Ram-Gang", 3),
        ("Llanowar Elves", 2),
        ("Grizzly Bears", 4),
        ("Serra Angel", 2),
        ("Giant Growth", 3),
        ("Lightning Bolt", 3),
        ("Counterspell", 3),
    ]
    .into_iter()
    .flat_map(|(name, copies)| (0..copies).map(move |_| definition(name)))
    .collect()
}

/// The payments in a campaign that took a permanent: a sacrifice or a tap of a permanent the payer pays no mana from.
fn taking_payments(events: &[Event]) -> u64 {
    events
        .iter()
        .zip(events.iter().skip(1))
        .filter(|(before, after)| {
            matches!(
                before,
                Event::PermanentTapped { .. }
                    | Event::ZoneChanged {
                        cause: ZoneCause::Sacrificed,
                        ..
                    }
            ) && matches!(
                after,
                Event::PermanentTapped { .. } | Event::ManaAdded { .. }
            )
        })
        .count() as u64
}

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn bot_games_with_these_sources_finish_and_keep_every_invariant(#[case] players: usize) {
    let seen = Mutex::new(0u64);
    let games: u64 = std::env::var("MTG_FUZZ_GAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20);
    for_each_seed(0..games, |seed| {
        let decks = (0..players).map(|_| deck()).collect();
        let setup = GameSetup::new(Arc::new(Normal), seed, decks);
        let options = PlayOptions {
            partial_payments: seed % 2 == 0,
            ..PlayOptions::default()
        };
        let outcome =
            play_game_with(setup, seed, options).unwrap_or_else(|failure| panic!("{failure}"));
        *seen.lock().unwrap() += taking_payments(&outcome.events);
    });
    assert!(
        seen.into_inner().unwrap() > 0,
        "the games should use these sources"
    );
}
