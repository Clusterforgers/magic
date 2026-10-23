//! Goal: the creatures tapped to crew a Vehicle or saddle a Mount are remembered for the turn: the abilities of Subterranean Schooner, Getaway Car, Giant Beaver and Rambling Possum choose only among the creatures that paid for their own permanent (not another Vehicle's riders, not a creature that did not pay), act on the one chosen, a Mount may return any number of its saddlers and none, the record ends with the turn, and the log of such games replays to the live state, at tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::{CardsChoice, Target};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, characteristics, replay};
use mtg_testkit::cards::free_creature;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const ME: PlayerId = PlayerId(0);

fn corpus(name: &str) -> Arc<CardDefinition> {
    card(name).unwrap()
}

/// Seat 0 controls these corpus permanents and the riders Rider A, Rider B and Rider C (power 2 each); seat 1 controls
/// an Ogre.
fn table(seats: usize, permanents: &[&str]) -> Scenario {
    let mut mine: Vec<_> = permanents.iter().map(|name| ready(corpus(name))).collect();
    mine.extend(["Rider A", "Rider B", "Rider C"].map(|name| ready(free_creature(name, 2, 2))));
    Scenario::new(seats)
        .battlefield(0, mine)
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

/// Pays the crew or saddle ability of `name` with these creatures and lets it resolve.
fn pay_for(driver: &mut Driver, name: &str, riders: &[&str]) -> Result<(), ScriptError> {
    driver.begin_activation(name)?;
    if matches!(driver.decision()?.kind, DecisionKind::ChooseCards(_)) {
        let ids = riders.iter().map(|rider| driver.id_of(rider).unwrap());
        driver.answer(Answer::ChooseCards(CardsChoice(ids.collect())))?;
    }
    settle(driver)
}

/// Pays for `name` in the first main step and attacks seat 1 with it.
fn pay_and_attack(driver: &mut Driver, name: &str, riders: &[&str]) -> Result<(), ScriptError> {
    driver.pass_until(Step::Main1)?;
    pay_for(driver, name, riders)?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[(name, 1)])?;
    Ok(())
}

fn name_of(driver: &Driver, target: &Target) -> String {
    let object = target.object().unwrap();
    driver
        .state()
        .object(object.id)
        .unwrap()
        .card()
        .name
        .clone()
}

/// The names the pending target question offers, sorted, and the fewest targets it takes.
fn offered(driver: &Driver) -> Result<(Vec<String>, u32), ScriptError> {
    let DecisionKind::ChooseTargets(options) = &driver.decision()?.kind else {
        return Err(ScriptError::Stuck("a question about targets".into()));
    };
    let mut names: Vec<String> = options
        .legal
        .iter()
        .map(|target| name_of(driver, target))
        .collect();
    names.sort();
    Ok((names, options.min))
}

fn assert_replays(driver: &Driver, scenario: &Scenario) {
    let replayed = replay(&scenario.clone().setup().unwrap(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

fn on_battlefield(driver: &Driver, name: &str) -> bool {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .any(|id| driver.state().object(*id).unwrap().card().name == name)
}

#[rstest]
#[case::schooner("Subterranean Schooner", &["Rider A", "Rider B"], 1)]
#[case::schooner_other_pair("Subterranean Schooner", &["Rider B", "Rider C"], 1)]
#[case::car("Getaway Car", &["Rider A", "Rider B"], 0)]
#[case::car_other_pair("Getaway Car", &["Rider A", "Rider C"], 0)]
#[case::beaver("Giant Beaver", &["Rider A", "Rider B"], 1)]
#[case::beaver_other_pair("Giant Beaver", &["Rider B", "Rider C"], 1)]
fn only_the_creatures_that_paid_can_be_chosen(
    #[case] name: &str,
    #[case] riders: &[&str],
    #[case] fewest: u32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, &[name]))?;
    pay_and_attack(&mut driver, name, riders)?;
    let mut paid: Vec<String> = riders.iter().map(|rider| (*rider).to_owned()).collect();
    paid.sort();
    assert_eq!(offered(&driver)?, (paid, fewest));
    Ok(())
}

#[rstest]
fn the_riders_of_another_vehicle_are_not_offered(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let both = ["Subterranean Schooner", "Getaway Car"];
    let mut driver = Driver::from_scenario(table(seats, &both))?;
    driver.pass_until(Step::Main1)?;
    pay_for(&mut driver, "Getaway Car", &["Rider C"])?;
    pay_for(
        &mut driver,
        "Subterranean Schooner",
        &["Rider A", "Rider B"],
    )?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Subterranean Schooner", 1)])?;
    assert_eq!(
        offered(&driver)?.0,
        ["Rider A", "Rider B"],
        "Rider C paid for the Car"
    );
    Ok(())
}

#[rstest]
fn a_vehicle_that_was_not_crewed_this_turn_has_no_rider_to_choose_next_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats, &["Subterranean Schooner"]);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    driver.pass_until(Step::Main1)?;
    pay_for(
        &mut driver,
        "Subterranean Schooner",
        &["Rider A", "Rider B"],
    )?;
    // The next turn of seat 0: crewed by Rider C alone, so Riders A and B paid on a turn that is over.
    let next = u32::try_from(seats).unwrap() + 1;
    driver.idle_until(next, Step::Main1)?;
    pay_for(&mut driver, "Subterranean Schooner", &["Rider C"])?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Subterranean Schooner", 1)])?;
    // Riders A and B paid on a turn that is over, so only Rider C is offered.
    assert_eq!(offered(&driver)?, (vec!["Rider C".to_owned()], 1));
    let target = driver.target_named("Rider C")?;
    driver.choose_targets(&[target])?;
    settle(&mut driver)?;
    let explored: Vec<_> = driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::Explored { object, .. } => Some(*object),
            _ => None,
        })
        .collect();
    assert_eq!(explored, [driver.id_of("Rider C")?]);
    assert_replays(&driver, &scenario);
    Ok(())
}

#[rstest]
#[case::first("Rider A", "Rider B")]
#[case::second("Rider B", "Rider A")]
fn the_schooner_explores_the_rider_chosen(
    #[case] chosen: &str,
    #[case] other: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats, &["Subterranean Schooner"]);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    pay_and_attack(
        &mut driver,
        "Subterranean Schooner",
        &["Rider A", "Rider B"],
    )?;
    let target = driver.target_named(chosen)?;
    driver.choose_targets(&[target])?;
    settle(&mut driver)?;
    let explored: Vec<_> = driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::Explored { object, .. } => Some(*object),
            _ => None,
        })
        .collect();
    assert_eq!(explored, [driver.id_of(chosen)?]);
    assert!(!explored.contains(&driver.id_of(other)?));
    assert_replays(&driver, &scenario);
    Ok(())
}

#[rstest]
#[case::returns_one(Some("Rider A"))]
#[case::returns_the_other(Some("Rider B"))]
#[case::returns_none(None)]
fn the_car_returns_the_rider_chosen_to_its_owners_hand_or_none(
    #[case] chosen: Option<&str>,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats, &["Getaway Car"]);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    pay_and_attack(&mut driver, "Getaway Car", &["Rider A", "Rider B"])?;
    let targets: Vec<Target> = match chosen {
        Some(name) => vec![driver.target_named(name)?],
        None => Vec::new(),
    };
    driver.choose_targets(&targets)?;
    settle(&mut driver)?;
    for rider in ["Rider A", "Rider B"] {
        let returned = chosen == Some(rider);
        assert_eq!(on_battlefield(&driver, rider), !returned, "{rider}");
    }
    let hand = driver.state().zone(Zone::Hand(ME)).len();
    assert_eq!(hand, usize::from(chosen.is_some()));
    assert!(on_battlefield(&driver, "Rider C"));
    assert_replays(&driver, &scenario);
    Ok(())
}

#[rstest]
#[case::first("Rider A", "Rider B")]
#[case::second("Rider B", "Rider A")]
fn the_beaver_puts_its_counter_on_the_saddler_chosen(
    #[case] chosen: &str,
    #[case] other: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats, &["Giant Beaver"]);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    pay_and_attack(&mut driver, "Giant Beaver", &["Rider A", "Rider B"])?;
    let target = driver.target_named(chosen)?;
    driver.choose_targets(&[target])?;
    settle(&mut driver)?;
    let power = |name: &str| {
        characteristics(driver.state(), driver.id_of(name).unwrap())
            .unwrap()
            .power
            .unwrap()
    };
    assert_eq!((power(chosen), power(other), power("Rider C")), (3, 2, 2));
    assert_replays(&driver, &scenario);
    Ok(())
}

#[rstest]
#[case::none(&[])]
#[case::one(&["Rider B"])]
#[case::both(&["Rider A", "Rider B"])]
fn the_possum_returns_any_number_of_its_saddlers(
    #[case] returned: &[&str],
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats, &["Rambling Possum"]);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    pay_and_attack(&mut driver, "Rambling Possum", &["Rider A", "Rider B"])?;
    settle_until_asked(&mut driver)?;
    let DecisionKind::ChooseCards(options) = driver.decision()?.kind.clone() else {
        panic!("the possum asks which saddlers to return");
    };
    let mut from: Vec<String> = options
        .from
        .iter()
        .map(|id| driver.state().object(*id).unwrap().card().name.clone())
        .collect();
    from.sort();
    assert_eq!(
        from,
        ["Rider A", "Rider B"],
        "Rider C and the Possum paid nothing"
    );
    assert_eq!((options.min, options.max), (0, 2));
    let ids = returned.iter().map(|rider| driver.id_of(rider).unwrap());
    driver.answer(Answer::ChooseCards(CardsChoice(ids.collect())))?;
    settle(&mut driver)?;
    for rider in ["Rider A", "Rider B"] {
        assert_eq!(
            on_battlefield(&driver, rider),
            !returned.contains(&rider),
            "{rider}"
        );
    }
    assert_eq!(driver.state().zone(Zone::Hand(ME)).len(), returned.len());
    let stats = characteristics(driver.state(), driver.id_of("Rambling Possum")?).unwrap();
    assert_eq!(
        (stats.power, stats.toughness),
        (Some(4), Some(5)),
        "+1/+2 first"
    );
    assert_replays(&driver, &scenario);
    Ok(())
}

/// Answers passively until a card choice is pending.
fn settle_until_asked(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..50 {
        if matches!(driver.decision()?.kind, DecisionKind::ChooseCards(_)) {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the choice of saddlers".into()))
}

#[rstest]
fn a_mount_that_attacks_unsaddled_does_nothing_with_the_record(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats, &["Rambling Possum"]);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Rambling Possum", 1)])?;
    settle(&mut driver)?;
    let stats = characteristics(driver.state(), driver.id_of("Rambling Possum")?).unwrap();
    assert_eq!((stats.power, stats.toughness), (Some(3), Some(3)));
    assert_replays(&driver, &scenario);
    Ok(())
}

fn is_creature(driver: &Driver, name: &str) -> bool {
    characteristics(driver.state(), driver.id_of(name).unwrap())
        .unwrap()
        .is_creature()
}

/// Seat 0 controls a Mobilizer Mech, the other Vehicles `others`, Rider A and Rider B (power 2 each, Crew 3 needs both);
/// seat 1 controls a Vehicle of its own.
fn mech_table(seats: usize, others: &[&str]) -> Scenario {
    let mut mine = vec![ready(corpus("Mobilizer Mech"))];
    mine.extend(others.iter().map(|name| ready(corpus(name))));
    mine.extend(["Rider A", "Rider B"].map(|name| ready(free_creature(name, 2, 2))));
    Scenario::new(seats)
        .battlefield(0, mine)
        .battlefield(1, [ready(corpus("Getaway Car"))])
}

#[rstest]
#[case::the_other_vehicle(&["Getaway Car"], Some("Getaway Car"))]
#[case::the_other_vehicle_of_two(&["Getaway Car", "Subterranean Schooner"], Some("Subterranean Schooner"))]
#[case::none_chosen(&["Getaway Car"], None)]
fn the_mech_animates_up_to_one_other_vehicle_of_its_controller_when_it_is_crewed(
    #[case] others: &[&str],
    #[case] chosen: Option<&str>,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = mech_table(seats, others);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    driver.pass_until(Step::Main1)?;
    driver.begin_activation("Mobilizer Mech")?;
    let ids = ["Rider A", "Rider B"].map(|rider| driver.id_of(rider).unwrap());
    driver.answer(Answer::ChooseCards(CardsChoice(ids.to_vec())))?;
    // The trigger waits for its target after the crew resolves: not the Mech, not the Vehicle of seat 1.
    for _ in 0..50 {
        if matches!(driver.decision()?.kind, DecisionKind::ChooseTargets(_)) {
            break;
        }
        driver.answer_passively()?;
    }
    let mut expected: Vec<String> = others.iter().map(|name| (*name).to_owned()).collect();
    expected.sort();
    assert_eq!(offered(&driver)?, (expected, 0));
    let targets: Vec<Target> = match chosen {
        Some(name) => vec![driver.target_named(name)?],
        None => Vec::new(),
    };
    driver.choose_targets(&targets)?;
    settle(&mut driver)?;
    assert!(is_creature(&driver, "Mobilizer Mech"));
    for name in others {
        assert_eq!(is_creature(&driver, name), chosen == Some(*name), "{name}");
    }
    assert_replays(&driver, &scenario);
    Ok(())
}

/// Seat 0 controls a Stubborn Burrowfiend and Rider A, Rider B (power 2 each, Saddle 2 needs one), `graveyard` creature
/// cards are in its graveyard and `library` creature cards are on top of its library, then lands.
fn burrow_table(seats: usize, graveyard: usize, library: usize) -> Scenario {
    let cards = |prefix: &str, count: usize| -> Vec<Arc<CardDefinition>> {
        (0..count)
            .map(|at| free_creature(&format!("{prefix} {at}"), 1, 1))
            .collect()
    };
    let mut top = cards("Milled", library);
    top.extend((0..5).map(|_| mtg_testkit::cards::mountain()));
    Scenario::new(seats)
        .battlefield(
            0,
            [
                ready(corpus("Stubborn Burrowfiend")),
                ready(free_creature("Rider A", 2, 2)),
                ready(free_creature("Rider B", 2, 2)),
            ],
        )
        .graveyard(0, cards("Buried", graveyard))
        .library(0, top)
}

#[rstest]
#[case::nothing_in_the_yard_two_milled(0, 2, 2)]
#[case::one_milled(0, 1, 1)]
#[case::none_milled(0, 0, 0)]
#[case::the_graveyard_counts_too(3, 2, 5)]
#[case::the_graveyard_alone(3, 0, 3)]
fn the_burrowfiend_counts_the_creature_cards_after_it_mills(
    #[case] graveyard: usize,
    #[case] library: usize,
    #[case] bonus: i32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = burrow_table(seats, graveyard, library);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    driver.pass_until(Step::Main1)?;
    pay_for(&mut driver, "Stubborn Burrowfiend", &["Rider A"])?;
    let stats = characteristics(driver.state(), driver.id_of("Stubborn Burrowfiend")?).unwrap();
    // Two cards are milled from the top of the library (the creatures first, then lands), then the size counts.
    assert_eq!(
        (stats.power, stats.toughness),
        (Some(2 + bonus), Some(2 + bonus))
    );
    assert_eq!(
        driver.state().zone(Zone::Graveyard(ME)).len(),
        graveyard + 2
    );
    assert_replays(&driver, &scenario);
    Ok(())
}

#[rstest]
fn only_the_first_saddle_of_the_turn_mills_and_grows(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = burrow_table(seats, 0, 4);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    driver.pass_until(Step::Main1)?;
    pay_for(&mut driver, "Stubborn Burrowfiend", &["Rider A"])?;
    let yard = driver.state().zone(Zone::Graveyard(ME)).len();
    // Riders are tapped to pay; the Mount saddled again by Rider B is the second time this turn.
    pay_for(&mut driver, "Stubborn Burrowfiend", &["Rider B"])?;
    assert_eq!(driver.state().zone(Zone::Graveyard(ME)).len(), yard);
    let stats = characteristics(driver.state(), driver.id_of("Stubborn Burrowfiend")?).unwrap();
    assert_eq!((stats.power, stats.toughness), (Some(4), Some(4)));
    assert_replays(&driver, &scenario);
    Ok(())
}

const READERS: [&str; 6] = [
    "Mobilizer Mech",
    "Getaway Car",
    "Subterranean Schooner",
    "Giant Beaver",
    "Rambling Possum",
    "Stubborn Burrowfiend",
];

#[test]
fn bot_games_with_the_crew_record_cards_keep_every_invariant_and_replay() {
    use std::sync::Mutex;

    use mtg_testkit::level_cards::SADDLE_CARDS;
    use mtg_testkit::vehicle_cards::VEHICLE_CARDS;
    use mtg_testkit::{PlayOptions, level_fuzz_setup, play_game_with, vehicle_fuzz_setup};

    use crate::common::seeds::{for_each_seed, fuzz_games};

    for name in READERS {
        assert!(
            VEHICLE_CARDS.contains(&name) || SADDLE_CARDS.contains(&name),
            "{name} is in a fuzz deck"
        );
    }
    // Crews and saddles of these cards in all the games.
    let seen = Mutex::new(0u64);
    for_each_seed(0..fuzz_games(12), |seed| {
        for players in [2, 3, 4] {
            for setup in [
                vehicle_fuzz_setup(players, seed),
                level_fuzz_setup(players, seed),
            ] {
                let outcome = play_game_with(setup.clone(), seed, PlayOptions::default())
                    .unwrap_or_else(|failure| panic!("{failure}"));
                let replayed = replay(&setup, &outcome.events).unwrap();
                assert_eq!(replayed.state_hash(), outcome.final_hash, "seed {seed}");
                let crews = outcome
                    .events
                    .iter()
                    .filter(|event| match event {
                        Event::Crewed { object, .. } => replayed
                            .object(*object)
                            .is_some_and(|o| READERS.contains(&o.card().name.as_str())),
                        _ => false,
                    })
                    .count();
                *seen.lock().unwrap() += u64::try_from(crews).unwrap();
            }
        }
    });
    assert!(
        seen.into_inner().unwrap() > 0,
        "some card that reads the crews was crewed or saddled"
    );
}
