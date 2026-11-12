//! Goal: cards that put, remove, move and read counters play whole, as their cards say, at tables of two, three and four seats: Tireless Tracker grows when a Clue is sacrificed, Cathars' Crusade puts a counter on each creature you control, Inspiring Call counts and protects the creatures that have a +1/+1 counter and no others, Caress of Phyrexia gives a player poison counters and ten of them lose the game, a -1/-1 counter on a creature with a +1/+1 counter takes a pair off by state-based actions and one on a small creature kills it, counters move from one creature to another and come off one at a time, the counters a creature had when it died are read after it has gone, energy and experience counters on a player are put on, spent and counted, power and toughness follow the counters through the layers (after a set, with a pump), and a spell whose target is gone puts nothing on anything; and the log of such games replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_cards::value::CounterKind;
use mtg_engine::choose::Target;
use mtg_engine::events::DeathCause;
use mtg_engine::keywords::has_keyword;
use mtg_engine::player::{LossReason, PlayerStatus};
use mtg_engine::zones::Zone;
use mtg_engine::{Event, characteristics, replay};
use mtg_testkit::cards::{forest, free_creature, island, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::counter_cards::{
    counter_siphon, death_watcher, energy_cell, growth_spurt, sap, soul_siphon, veteran_recruiter,
};
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

const PLUS: CounterKind = CounterKind::PlusOnePlusOne;
const MINUS: CounterKind = CounterKind::MinusOneMinusOne;

fn resolve_the_stack(driver: &mut Driver) -> Result<(), ScriptError> {
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    Ok(())
}

fn stats(driver: &Driver, name: &str) -> (i32, i32) {
    let chars = characteristics(driver.state(), driver.id_of(name).unwrap()).unwrap();
    (chars.power.unwrap(), chars.toughness.unwrap())
}

fn counters_on(driver: &Driver, name: &str, kind: &CounterKind) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| permanent.counters_of(kind))
}

fn held_by(driver: &Driver, seat: usize, kind: &CounterKind) -> u32 {
    driver.state().player(pid(seat)).unwrap().counters_of(kind)
}

fn hand(driver: &Driver, seat: usize) -> usize {
    driver.state().zone(Zone::Hand(pid(seat))).len()
}

/// The counters put on and taken off in `events`, as (what holds them, kind, change), in order.
fn changes(driver: &Driver, events: &[Event]) -> Vec<(String, CounterKind, i32)> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::CountersChanged(changed) => {
                let holder = match changed.on {
                    Target::Player(player) => format!("seat {}", player.index()),
                    Target::Object(object) => driver
                        .state()
                        .object(object.id)
                        .unwrap()
                        .card()
                        .name
                        .clone(),
                };
                Some((holder, changed.counter.clone(), changed.delta))
            }
            _ => None,
        })
        .collect()
}

fn named(driver: &Driver, name: &str) -> Target {
    driver.target_named(name).unwrap()
}

fn after(driver: &Driver, begun: usize) -> Vec<Event> {
    driver.engine().events()[begun..].to_vec()
}

fn replays(driver: &Driver, setup: &mtg_engine::GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

// Tireless Tracker

#[rstest]
fn tireless_tracker_grows_when_a_clue_is_sacrificed(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(seats)
            .hand(0, [forest()])
            .battlefield(
                0,
                [
                    ready(card("Tireless Tracker").unwrap()),
                    ready(forest()),
                    ready(forest()),
                ],
            )
            .library(0, [island(), island(), island()])
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.play_land("Forest")?;
    resolve_the_stack(&mut driver)?;
    assert_eq!(stats(&driver, "Tireless Tracker"), (3, 2));
    let cards = hand(&driver, 0);

    let begun = driver.engine().events().len();
    driver.begin_activation("Clue")?;
    driver.pay_tapping(&["Forest", "Forest"], 0)?;
    resolve_the_stack(&mut driver)?;

    let events = after(&driver, begun);
    assert_eq!(
        changes(&driver, &events),
        [("Tireless Tracker".to_owned(), PLUS, 1)]
    );
    assert_eq!(counters_on(&driver, "Tireless Tracker", &PLUS), 1);
    assert_eq!(stats(&driver, "Tireless Tracker"), (4, 3));
    assert_eq!(hand(&driver, 0), cards + 1, "the Clue drew its card");
    replays(&driver, &setup);
    Ok(())
}

// Cathars' Crusade

#[rstest]
fn cathars_crusade_puts_a_counter_on_each_creature_you_control_when_one_enters(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(seats)
            .hand(0, [card("Grizzly Bears").unwrap()])
            .battlefield(
                0,
                [
                    ready(card("Cathars' Crusade").unwrap()),
                    ready(free_creature("Wolf", 3, 3)),
                    ready(forest()),
                    ready(forest()),
                ],
            )
            .battlefield(1, [ready(free_creature("Ogre", 4, 4))])
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let begun = driver.engine().events().len();
    driver.cast("Grizzly Bears")?;
    resolve_the_stack(&mut driver)?;

    let events = after(&driver, begun);
    assert_eq!(
        changes(&driver, &events),
        [
            ("Grizzly Bears".to_owned(), PLUS, 1),
            ("Wolf".to_owned(), PLUS, 1)
        ],
        "one counter on each creature, in the order of their ids"
    );
    assert_eq!(stats(&driver, "Wolf"), (4, 4));
    assert_eq!(stats(&driver, "Grizzly Bears"), (3, 3));
    assert_eq!(stats(&driver, "Ogre"), (4, 4));
    replays(&driver, &setup);
    Ok(())
}

// Inspiring Call

#[rstest]
#[case::another_with_a_plus_counter(vec![(PLUS, 1)], 2, true)]
#[case::another_with_two(vec![(PLUS, 2)], 2, true)]
#[case::another_with_a_minus_counter(vec![(MINUS, 1)], 1, false)]
#[case::another_with_a_charge_counter(vec![(CounterKind::Charge, 2)], 1, false)]
#[case::another_with_none(vec![], 1, false)]
fn inspiring_call_counts_and_protects_the_creatures_with_a_plus_counter(
    #[values(2, 3, 4)] seats: usize,
    #[case] other_counters: Vec<(CounterKind, u32)>,
    #[case] drawn: usize,
    #[case] other_protected: bool,
) -> Result<(), ScriptError> {
    let mut other = ready(free_creature("Other", 3, 3));
    other.counters = other_counters;
    let scenario = || {
        Scenario::new(seats)
            .hand(0, [card("Inspiring Call").unwrap()])
            .battlefield(
                0,
                [
                    with_counters(ready(free_creature("Bear", 2, 2)), PLUS, 2),
                    other.clone(),
                    ready(forest()),
                    ready(forest()),
                    ready(forest()),
                ],
            )
            .battlefield(
                1,
                [with_counters(ready(free_creature("Foe", 2, 2)), PLUS, 1)],
            )
            .library(0, [island(), island(), island(), island()])
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let cards = hand(&driver, 0);
    driver.cast("Inspiring Call")?;
    resolve_the_stack(&mut driver)?;

    assert_eq!(hand(&driver, 0), cards - 1 + drawn);
    let indestructible = |name: &str| {
        has_keyword(
            driver.state(),
            driver.id_of(name).unwrap(),
            &mtg_cards::keyword::Keyword::Indestructible,
        )
    };
    assert!(indestructible("Bear"));
    assert_eq!(indestructible("Other"), other_protected);
    assert!(!indestructible("Foe"), "a creature of another seat");
    replays(&driver, &setup);
    Ok(())
}

// Caress of Phyrexia

#[rstest]
#[case::the_first_three(0, false)]
#[case::six(3, false)]
#[case::the_tenth(7, true)]
#[case::more_than_ten(9, true)]
fn caress_of_phyrexia_gives_poison_counters_and_ten_lose_the_game(
    #[values(2, 3, 4)] seats: usize,
    #[case] already: u32,
    #[case] loses: bool,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(seats)
            .hand(0, [card("Caress of Phyrexia").unwrap()])
            .battlefield(0, (0..5).map(|_| ready(swamp())))
            .counters(1, PLUS, 1)
            .counters(1, CounterKind::Poison, already)
            .library(1, [island(), island(), island(), island()])
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let target = driver.target_player(1);
    let begun = driver.engine().events().len();
    driver.cast_at("Caress of Phyrexia", &[target])?;
    resolve_the_stack(&mut driver)?;

    let events = after(&driver, begun);
    assert_eq!(
        changes(&driver, &events),
        [("seat 1".to_owned(), CounterKind::Poison, 3)]
    );
    assert_eq!(held_by(&driver, 1, &CounterKind::Poison), already + 3);
    assert_eq!(held_by(&driver, 1, &PLUS), 1, "other kinds stay");
    assert_eq!(driver.state().player(pid(1)).unwrap().life(), 17);
    let lost = driver.state().player(pid(1)).unwrap().status().clone();
    if loses {
        assert_eq!(lost, PlayerStatus::Lost(LossReason::Poison));
        assert!(events.iter().any(|event| matches!(
            event,
            Event::PlayerLost { player, reason: LossReason::Poison } if *player == pid(1)
        )));
        assert_eq!(driver.state().result().is_some(), seats == 2);
    } else {
        assert_eq!(lost, PlayerStatus::InGame);
        assert_eq!(hand(&driver, 1), 3, "and drew three cards");
    }
    replays(&driver, &setup);
    Ok(())
}

// Counters against counters

fn small_table(seats: usize, hand: Vec<std::sync::Arc<mtg_cards::CardDefinition>>) -> Scenario {
    Scenario::new(seats)
        .hand(0, hand)
        .battlefield(
            0,
            [
                ready(forest()),
                ready(swamp()),
                ready(island()),
                ready(forest()),
            ],
        )
        .library(0, [island(), island(), island()])
}

#[rstest]
fn a_minus_counter_on_a_creature_with_a_plus_counter_takes_a_pair_off_by_state_based_actions(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        small_table(seats, vec![sap().unwrap()]).battlefield(
            1,
            [with_counters(ready(free_creature("Bear", 2, 2)), PLUS, 1)],
        )
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let target = named(&driver, "Bear");
    let begun = driver.engine().events().len();
    driver.cast_at("Sap", &[target])?;
    resolve_the_stack(&mut driver)?;
    let events = changes(&driver, &after(&driver, begun));
    assert_eq!(
        events,
        [
            ("Bear".to_owned(), MINUS, 1),
            ("Bear".to_owned(), PLUS, -1),
            ("Bear".to_owned(), MINUS, -1),
        ],
        "the counter, then the pair by state-based actions"
    );
    assert_eq!(counters_on(&driver, "Bear", &PLUS), 0);
    assert_eq!(counters_on(&driver, "Bear", &MINUS), 0);
    assert_eq!(stats(&driver, "Bear"), (2, 2));
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn a_minus_counter_kills_a_creature_with_one_toughness_and_its_counters_go_with_it(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        small_table(seats, vec![sap().unwrap()])
            .battlefield(1, [ready(free_creature("Mouse", 1, 1))])
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let target = named(&driver, "Mouse");
    let begun = driver.engine().events().len();
    driver.cast_at("Sap", &[target])?;
    resolve_the_stack(&mut driver)?;
    let events = after(&driver, begun);
    assert!(events.iter().any(|event| matches!(
        event,
        Event::CreatureDied {
            cause: DeathCause::ZeroToughness,
            ..
        }
    )));
    assert!(driver.id_of("Mouse").is_err(), "it is in a graveyard");
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn a_pump_that_follows_a_minus_counter_takes_the_pair_off_and_leaves_the_rest(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        small_table(seats, vec![growth_spurt().unwrap()]).battlefield(
            1,
            [with_counters(ready(free_creature("Brute", 3, 3)), MINUS, 2)],
        )
    };
    let mut driver = Driver::from_scenario(scenario())?;
    assert_eq!(stats(&driver, "Brute"), (1, 1));
    let target = named(&driver, "Brute");
    driver.cast_at("Growth Spurt", &[target])?;
    resolve_the_stack(&mut driver)?;
    assert_eq!(counters_on(&driver, "Brute", &PLUS), 0);
    assert_eq!(counters_on(&driver, "Brute", &MINUS), 1);
    assert_eq!(stats(&driver, "Brute"), (2, 2));
    Ok(())
}

// Moving and removing counters

#[rstest]
fn counter_siphon_moves_every_plus_counter_from_one_creature_to_another(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        small_table(seats, vec![counter_siphon().unwrap()]).battlefield(
            1,
            [
                with_counters(ready(free_creature("Giver", 2, 2)), PLUS, 3),
                ready(free_creature("Taker", 2, 2)),
            ],
        )
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    assert_eq!(stats(&driver, "Giver"), (5, 5));
    let (from, to) = (named(&driver, "Giver"), named(&driver, "Taker"));
    let begun = driver.engine().events().len();
    driver.begin_cast("Counter Siphon")?;
    driver.choose_targets(&[from])?;
    driver.choose_targets(&[to])?;
    driver.answer(mtg_engine::decision::Answer::PayMana(
        mtg_engine::decision::PayChoice::Auto,
    ))?;
    resolve_the_stack(&mut driver)?;
    assert_eq!(
        changes(&driver, &after(&driver, begun)),
        [
            ("Giver".to_owned(), PLUS, -3),
            ("Taker".to_owned(), PLUS, 3)
        ]
    );
    assert_eq!(stats(&driver, "Giver"), (2, 2));
    assert_eq!(stats(&driver, "Taker"), (5, 5));
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn soul_siphon_takes_one_counter_off_and_draws_a_card(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(seats)
            .hand(0, [soul_siphon().unwrap()])
            .battlefield(
                0,
                [
                    ready(island()),
                    with_counters(ready(free_creature("Bear", 2, 2)), PLUS, 2),
                ],
            )
            .library(0, [island(), island(), island()])
    };
    let mut driver = Driver::from_scenario(scenario())?;
    let target = named(&driver, "Bear");
    let cards = hand(&driver, 0);
    let begun = driver.engine().events().len();
    driver.cast_at("Soul Siphon", &[target])?;
    resolve_the_stack(&mut driver)?;
    assert_eq!(
        changes(&driver, &after(&driver, begun)),
        [("Bear".to_owned(), PLUS, -1)]
    );
    assert_eq!(stats(&driver, "Bear"), (3, 3));
    assert_eq!(hand(&driver, 0), cards, "one card cast, one drawn");
    Ok(())
}

// Last known information

#[rstest]
#[case::two_counters(2, 2)]
#[case::none(0, 0)]
fn the_counters_a_creature_had_when_it_died_are_read_after_it_has_gone(
    #[values(2, 3, 4)] seats: usize,
    #[case] counters: u32,
    #[case] drawn: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(seats)
            .hand(0, [card("Doom Blade").unwrap()])
            .battlefield(
                0,
                [
                    ready(death_watcher().unwrap()),
                    with_counters(ready(free_creature("Wolf", 3, 3)), PLUS, counters),
                    ready(swamp()),
                    ready(swamp()),
                ],
            )
            .library(0, [island(), island(), island(), island()])
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let target = named(&driver, "Wolf");
    let cards = hand(&driver, 0);
    driver.cast_at("Doom Blade", &[target])?;
    resolve_the_stack(&mut driver)?;
    assert!(driver.id_of("Wolf").is_err(), "the Wolf is dead");
    assert_eq!(hand(&driver, 0), cards - 1 + drawn);
    replays(&driver, &setup);
    Ok(())
}

// Counters on players

#[rstest]
fn energy_counters_are_put_on_a_player_spent_and_counted(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(seats)
            .hand(0, [energy_cell().unwrap()])
            .battlefield(0, [ready(forest())])
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.cast("Energy Cell")?;
    resolve_the_stack(&mut driver)?;
    let energy = CounterKind::Other("energy".into());
    assert_eq!(held_by(&driver, 0, &energy), 2);
    let begun = driver.engine().events().len();
    driver.begin_activation("Energy Cell")?;
    resolve_the_stack(&mut driver)?;
    assert_eq!(
        changes(&driver, &after(&driver, begun)),
        [("seat 0".to_owned(), energy.clone(), -1)]
    );
    assert_eq!(held_by(&driver, 0, &energy), 1);
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), 21);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn experience_counters_grow_as_creatures_die_and_are_counted_at_the_end_step(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(seats)
            .hand(0, [card("Doom Blade").unwrap()])
            .battlefield(
                0,
                [
                    ready(veteran_recruiter().unwrap()),
                    ready(free_creature("Wolf", 3, 3)),
                    ready(swamp()),
                    ready(swamp()),
                ],
            )
            .library(0, [island(), island(), island(), island()])
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let target = named(&driver, "Wolf");
    driver.cast_at("Doom Blade", &[target])?;
    resolve_the_stack(&mut driver)?;
    let experience = CounterKind::Other("experience".into());
    assert_eq!(held_by(&driver, 0, &experience), 1);
    let life = driver.state().player(pid(0)).unwrap().life();
    driver.idle_until(1, mtg_engine::turn::Step::End)?;
    resolve_the_stack(&mut driver)?;
    assert_eq!(
        driver.state().player(pid(0)).unwrap().life(),
        life + 1,
        "the end step gains one life for the one experience counter"
    );
    replays(&driver, &setup);
    Ok(())
}

// Layers

#[rstest]
#[case::humility("Humility", (3, 3))]
fn power_and_toughness_follow_the_counters_after_a_set_and_with_a_pump(
    #[values(2, 3, 4)] seats: usize,
    #[case] set: &str,
    #[case] expected: (i32, i32),
) -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(Scenario::new(seats).battlefield(
        0,
        [
            ready(card(set).unwrap()),
            with_counters(ready(free_creature("Bear", 2, 2)), PLUS, 2),
        ],
    ))?;
    // Humility makes every creature 1/1 (layer 7b), and the two counters come on top (layer 7c).
    assert_eq!(stats(&driver, "Bear"), expected);
    Ok(())
}

#[rstest]
fn a_pump_and_counters_add_up_and_the_pump_ends_with_the_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [card("Giant Growth").unwrap()])
            .battlefield(
                0,
                [
                    with_counters(ready(free_creature("Bear", 2, 2)), PLUS, 2),
                    ready(forest()),
                ],
            ),
    )?;
    assert_eq!(stats(&driver, "Bear"), (4, 4));
    let target = named(&driver, "Bear");
    driver.cast_at("Giant Growth", &[target])?;
    resolve_the_stack(&mut driver)?;
    assert_eq!(stats(&driver, "Bear"), (7, 7));
    driver.idle_until(2, mtg_engine::turn::Step::Main1)?;
    assert_eq!(stats(&driver, "Bear"), (4, 4), "the counters stay");
    Ok(())
}

// A target that is gone

#[rstest]
fn a_spell_whose_target_is_gone_puts_no_counter_on_anything(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(seats)
            .hand(0, [growth_spurt().unwrap()])
            .hand(1, [card("Lightning Bolt").unwrap()])
            .battlefield(0, [ready(forest()), ready(free_creature("Mouse", 1, 1))])
            .battlefield(1, [ready(mtg_testkit::cards::mountain())])
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let target = named(&driver, "Mouse");
    let begun = driver.engine().events().len();
    driver.cast_at("Growth Spurt", &[target])?;
    driver.pass()?;
    driver.cast_at("Lightning Bolt", &[target])?;
    resolve_the_stack(&mut driver)?;
    assert!(driver.id_of("Mouse").is_err(), "the Mouse died");
    assert_eq!(changes(&driver, &after(&driver, begun)), []);
    replays(&driver, &setup);
    Ok(())
}
