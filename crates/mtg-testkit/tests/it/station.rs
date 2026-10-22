//! Goal: Station plays as the rules say (CR 702.184) at tables of two, three and four seats: the ability is offered at sorcery speed while another untapped creature can be tapped (a summoning sick one too, never the Spacecraft itself), tapping a creature puts charge counters equal to its power and taps only that creature, the charge counters decide which brackets work (the Spacecraft is a creature with its printed size only from its last bracket, keywords, lords and abilities work from their own thresholds on), a Spacecraft with haste attacks the turn it is a creature, and the log of whole games with such cards replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::keywords::keywords_of;
use mtg_engine::setup::PresetPermanent;
use mtg_engine::turn::Step;
use mtg_engine::{characteristics, replay};
use mtg_testkit::cards::{free_creature, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, sick, tapped, with_counters};
use rstest::rstest;

fn corpus(name: &str) -> Arc<CardDefinition> {
    card(name).unwrap()
}

/// A creature of the kind `how` says ("ready", "sick" or "tapped") with this name and power.
fn member(how: &str, name: &str, power: i32) -> PresetPermanent {
    let card = free_creature(name, power, 2);
    match how {
        "sick" => sick(card),
        "tapped" => tapped(card),
        _ => ready(card),
    }
}

/// Seat 0 controls `ship` with `charge` charge counters, these creatures and `mountains`; seat 1 controls an Ogre.
fn table(
    seats: usize,
    ship: &str,
    charge: u32,
    members: Vec<PresetPermanent>,
    mountains: usize,
) -> Scenario {
    let mut all = vec![with_counters(
        ready(corpus(ship)),
        CounterKind::Charge,
        charge,
    )];
    all.extend(members);
    all.extend((0..mountains).map(|_| ready(mountain())));
    Scenario::new(seats)
        .battlefield(0, all)
        .battlefield(1, [ready(free_creature("Ogre", 3, 4))])
}

/// Answers passively until the stack is empty and nothing resolves.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Stations `ship` with the creature `tapped_for_it` and lets the ability resolve.
fn station(driver: &mut Driver, ship: &str, tapped_for_it: &str) -> Result<(), ScriptError> {
    driver.begin_activation(ship)?;
    if matches!(driver.decision()?.kind, DecisionKind::ChooseCards(_)) {
        let picked = CardsChoice(vec![driver.id_of(tapped_for_it)?]);
        driver.answer(Answer::ChooseCards(picked))?;
    }
    if matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
    }
    settle(driver)
}

fn charge(driver: &Driver, name: &str) -> u32 {
    driver
        .state()
        .object(driver.id_of(name).unwrap())
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::Charge)
}

fn is_tapped(driver: &Driver, name: &str) -> bool {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .tapped()
}

fn is_creature(driver: &Driver, name: &str) -> bool {
    characteristics(driver.state(), driver.id_of(name).unwrap())
        .unwrap()
        .is_creature()
}

fn stats(driver: &Driver, name: &str) -> (i32, i32) {
    let found = characteristics(driver.state(), driver.id_of(name).unwrap()).unwrap();
    (found.power.unwrap(), found.toughness.unwrap())
}

fn has(driver: &Driver, name: &str, keyword: &Keyword) -> bool {
    keywords_of(driver.state(), driver.id_of(name).unwrap())
        .into_iter()
        .any(|found| found == keyword)
}

// The ability

#[rstest]
#[case::another_creature(1, 0, Step::Main1, &[("ready", "Bear", 2)], true)]
#[case::a_summoning_sick_creature(1, 0, Step::Main1, &[("sick", "Cub", 1)], true)]
#[case::a_creature_without_power(1, 0, Step::Main1, &[("ready", "Wall", 0)], true)]
#[case::only_a_tapped_creature(1, 0, Step::Main1, &[("tapped", "Bear", 2)], false)]
#[case::no_other_creature(1, 0, Step::Main1, &[], false)]
#[case::in_the_other_players_turn(2, 1, Step::Main1, &[("ready", "Bear", 2)], false)]
#[case::in_combat(1, 0, Step::BeginCombat, &[("ready", "Bear", 2)], false)]
fn station_is_offered_at_sorcery_speed_while_another_creature_can_be_tapped(
    #[values(2, 3, 4)] seats: usize,
    #[case] turn: u32,
    #[case] active: usize,
    #[case] step: Step,
    #[case] members: &[(&str, &str, i32)],
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let members = members
        .iter()
        .map(|(how, name, power)| member(how, name, *power))
        .collect();
    let scenario = table(seats, "Galvanizing Sawship", 0, members, 0).at(turn, active, step);
    let driver = Driver::from_scenario(scenario)?;
    let seen = !driver.offered_abilities("Galvanizing Sawship")?.is_empty();
    assert_eq!(seen, offered);
    Ok(())
}

#[rstest]
#[case::one(1)]
#[case::two(2)]
#[case::three(3)]
#[case::five(5)]
fn tapping_a_creature_puts_charge_counters_equal_to_its_power_and_taps_only_it(
    #[values(2, 3, 4)] seats: usize,
    #[case] power: i32,
) -> Result<(), ScriptError> {
    let members = vec![
        member("ready", "Chosen", power),
        member("ready", "Spare", 4),
    ];
    let mut driver = Driver::from_scenario(table(seats, "Galvanizing Sawship", 0, members, 0))?;
    station(&mut driver, "Galvanizing Sawship", "Chosen")?;
    assert_eq!(charge(&driver, "Galvanizing Sawship"), power.unsigned_abs());
    assert!(is_tapped(&driver, "Chosen"));
    assert!(!is_tapped(&driver, "Spare"));
    assert!(!is_tapped(&driver, "Galvanizing Sawship"));
    Ok(())
}

#[rstest]
fn a_summoning_sick_creature_pays_for_station_and_the_counters_add_up(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let members = vec![member("sick", "Cub", 2), member("ready", "Bear", 3)];
    let mut driver = Driver::from_scenario(table(seats, "Galvanizing Sawship", 0, members, 0))?;
    station(&mut driver, "Galvanizing Sawship", "Cub")?;
    assert_eq!(charge(&driver, "Galvanizing Sawship"), 2);
    assert!(!is_creature(&driver, "Galvanizing Sawship"));
    station(&mut driver, "Galvanizing Sawship", "Bear")?;
    assert_eq!(charge(&driver, "Galvanizing Sawship"), 5);
    assert!(is_creature(&driver, "Galvanizing Sawship"));
    Ok(())
}

// The brackets

#[rstest]
#[case::empty(0, false, false)]
#[case::one_short(2, false, false)]
#[case::at_the_threshold(3, true, true)]
#[case::beyond(10, true, true)]
fn the_last_bracket_makes_the_spacecraft_a_creature_with_its_printed_size_and_keywords(
    #[values(2, 3, 4)] seats: usize,
    #[case] counters: u32,
    #[case] creature: bool,
    #[case] keywords: bool,
) -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(table(seats, "Galvanizing Sawship", counters, vec![], 0))?;
    let ship = "Galvanizing Sawship";
    assert_eq!(is_creature(&driver, ship), creature);
    assert_eq!(has(&driver, ship, &Keyword::Flying), keywords);
    assert_eq!(has(&driver, ship, &Keyword::Haste), keywords);
    assert_eq!(stats(&driver, ship), (6, 5));
    Ok(())
}

#[rstest]
#[case::empty(0, (2, 2), false, false)]
#[case::one_short_of_the_lord(1, (2, 2), false, false)]
#[case::the_lord_bracket(2, (3, 3), false, false)]
#[case::one_short_of_the_top(11, (3, 3), false, false)]
#[case::the_top_bracket(12, (3, 3), true, true)]
#[case::beyond(30, (3, 3), true, true)]
fn a_lord_bracket_and_the_bracket_above_it_work_from_their_own_thresholds_on(
    #[values(2, 3, 4)] seats: usize,
    #[case] counters: u32,
    #[case] bear: (i32, i32),
    #[case] creature: bool,
    #[case] keywords: bool,
) -> Result<(), ScriptError> {
    let members = vec![member("ready", "Bear", 2)];
    let driver = Driver::from_scenario(table(seats, "Lumen-Class Frigate", counters, members, 0))?;
    let ship = "Lumen-Class Frigate";
    assert_eq!(stats(&driver, "Bear"), bear);
    assert_eq!(is_creature(&driver, ship), creature);
    assert_eq!(has(&driver, ship, &Keyword::Flying), keywords);
    assert_eq!(has(&driver, ship, &Keyword::Lifelink), keywords);
    if creature {
        // "Other creatures": the Spacecraft keeps its printed size.
        assert_eq!(stats(&driver, ship), (3, 5));
    }
    Ok(())
}

#[rstest]
#[case::below_the_bracket(7, false)]
#[case::at_the_bracket(8, true)]
#[case::above_it(9, true)]
fn an_activated_ability_of_a_bracket_is_offered_only_from_its_threshold_on(
    #[values(2, 3, 4)] seats: usize,
    #[case] counters: u32,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(table(seats, "Debris Field Crusher", counters, vec![], 2))?;
    let seen = !driver.offered_abilities("Debris Field Crusher")?.is_empty();
    assert_eq!(seen, offered);
    Ok(())
}

#[rstest]
fn stationing_moves_the_spacecraft_through_its_brackets(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let members = vec![member("ready", "Bear", 2), member("ready", "Elk", 3)];
    let mut driver = Driver::from_scenario(table(seats, "Lumen-Class Frigate", 0, members, 0))?;
    let ship = "Lumen-Class Frigate";
    assert!(!has(&driver, ship, &Keyword::Flying));
    station(&mut driver, ship, "Bear")?;
    assert_eq!(charge(&driver, ship), 2);
    // The lord bracket pumps the creature that is left, and the Bear that paid.
    assert_eq!(stats(&driver, "Elk"), (4, 3));
    assert!(!is_creature(&driver, ship));
    station(&mut driver, ship, "Elk")?;
    // The Elk has the power the lord bracket gives it, 3 and 1 more.
    assert_eq!(charge(&driver, ship), 6);
    assert!(!is_creature(&driver, ship));
    Ok(())
}

// Attacking

#[rstest]
#[case::as_a_creature(3, true)]
#[case::not_yet_a_creature(2, false)]
fn a_spacecraft_with_haste_attacks_the_turn_it_becomes_a_creature(
    #[values(2, 3, 4)] seats: usize,
    #[case] counters: u32,
    #[case] attacks: bool,
) -> Result<(), ScriptError> {
    let ship = with_counters(
        sick(corpus("Galvanizing Sawship")),
        CounterKind::Charge,
        counters,
    );
    let scenario = Scenario::new(seats)
        .battlefield(0, [ship])
        .battlefield(1, [ready(free_creature("Ogre", 3, 4))]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    let result = driver.attack(&[("Galvanizing Sawship", 1)]);
    assert_eq!(result.is_ok(), attacks, "{result:?}");
    if attacks {
        driver.idle_until(1, Step::Main2)?;
        assert_eq!(driver.state().player(pid(1)).unwrap().life(), 14);
    }
    Ok(())
}

// Whole games

#[test]
fn bot_games_with_station_keep_every_invariant_and_replay() {
    use std::sync::Mutex;

    use mtg_engine::Event;
    use mtg_testkit::{PlayOptions, play_game_with, station_fuzz_setup};

    use crate::common::seeds::{for_each_seed, fuzz_games};

    // Spacecraft stationed, charge counters put, Spacecraft that attacked, and the lord, draw and sacrifice brackets.
    let seen = Mutex::new([0u64; 4]);
    let games = fuzz_games(12);
    for_each_seed(0..games, |seed| {
        for players in [2, 3, 4] {
            let outcome = play_game_with(
                station_fuzz_setup(players, seed),
                seed,
                PlayOptions::default(),
            )
            .unwrap_or_else(|failure| panic!("{failure}"));
            let replayed = replay(&station_fuzz_setup(players, seed), &outcome.events).unwrap();
            assert_eq!(replayed.state_hash(), outcome.final_hash, "seed {seed}");
            let spacecraft = |id| {
                replayed.object(id).is_some_and(|object| {
                    mtg_testkit::station_cards::STATION_CARDS.contains(&object.card().name.as_str())
                })
            };
            let mut seen = seen.lock().unwrap();
            for event in &outcome.events {
                match event {
                    Event::AbilityActivated { source, .. } if spacecraft(source.id) => seen[0] += 1,
                    Event::CountersChanged(changed)
                        if changed.delta > 0 && changed.counter == CounterKind::Charge =>
                    {
                        seen[1] += 1;
                    }
                    Event::AttackersDeclared { attackers, .. } => {
                        seen[2] +=
                            attackers.iter().filter(|(id, _)| spacecraft(*id)).count() as u64;
                    }
                    _ => {}
                }
            }
            seen[3] += 1;
        }
    });
    let [stationed, charged, attacked, games_played] = seen.into_inner().unwrap();
    assert!(games_played > 0);
    assert!(stationed > 0, "some Spacecraft was activated");
    assert!(charged > 0, "some Spacecraft got charge counters");
    assert!(attacked > 0, "some Spacecraft attacked as a creature");
}
