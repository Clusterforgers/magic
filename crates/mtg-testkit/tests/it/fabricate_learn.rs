//! Goal: fabricate, learn and the city's blessing play whole at tables of two, three and four seats as the rules say: a fabricating creature takes its counters or makes Servo tokens by the player's choice, Hardened Scales, Doubling Season and Parallel Lives apply, a creature that is gone when its trigger resolves only makes the tokens; a learning player discards a card to draw one or declines; ascend gives the blessing at ten permanents, once, for good and to that player alone, and the logs replay to the states they were played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::color::Color;
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::keywords::has_keyword;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{creature, land};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// Seat 0 holds the spell and has `lands` basic lands of the colour besides `board`.
fn table(seats: usize, spell: &str, color: Color, lands: usize, board: &[&str]) -> Scenario {
    let mut permanents: Vec<_> = (0..lands).map(|_| ready(land(color))).collect();
    permanents.extend(board.iter().map(|name| ready(card(name).unwrap())));
    Scenario::new(seats)
        .hand(0, [card(spell).unwrap()])
        .battlefield(0, permanents)
}

/// Passes priority and answers each question to take the counters with `counters`, until nothing is on the stack;
/// it counts the questions.
fn resolve(driver: &mut Driver, counters: bool) -> Result<usize, ScriptError> {
    let mut asked = 0;
    for _ in 0..40 {
        match &driver.decision()?.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(asked),
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            DecisionKind::YesNo(_) => {
                asked += 1;
                driver.answer(Answer::YesNo(YesNoChoice(counters)))?;
            }
            other => panic!("unexpected {}", other.name()),
        }
    }
    panic!("the stack did not empty");
}

fn counters_on(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| {
            permanent.counters_of(&CounterKind::PlusOnePlusOne)
        })
}

fn servos(driver: &Driver, seat: usize) -> usize {
    driver
        .state()
        .objects()
        .filter(|object| object.zone() == Zone::Battlefield && object.controller() == pid(seat))
        .filter(|object| object.card().name == "Servo")
        .count()
}

fn blessings(driver: &Driver) -> usize {
    driver
        .engine()
        .events()
        .iter()
        .filter(|event| matches!(event, Event::CitysBlessingGained { .. }))
        .count()
}

#[rstest]
#[case::one_counter("Highspire Artisan", Color::Green, &[], true, 1, 0)]
#[case::one_servo("Highspire Artisan", Color::Green, &[], false, 0, 1)]
#[case::two_counters("Weaponcraft Enthusiast", Color::Black, &[], true, 2, 0)]
#[case::two_servos("Weaponcraft Enthusiast", Color::Black, &[], false, 0, 2)]
#[case::hardened_scales_adds_a_counter("Weaponcraft Enthusiast", Color::Black, &["Hardened Scales"], true, 3, 0)]
#[case::hardened_scales_leaves_the_servos("Weaponcraft Enthusiast", Color::Black, &["Hardened Scales"], false, 0, 2)]
#[case::doubling_season_doubles_the_counters("Highspire Artisan", Color::Green, &["Doubling Season"], true, 2, 0)]
#[case::doubling_season_doubles_the_servos("Highspire Artisan", Color::Green, &["Doubling Season"], false, 0, 2)]
#[case::parallel_lives_doubles_the_servos("Weaponcraft Enthusiast", Color::Black, &["Parallel Lives"], false, 0, 4)]
#[case::parallel_lives_leaves_the_counters("Weaponcraft Enthusiast", Color::Black, &["Parallel Lives"], true, 2, 0)]
fn a_fabricating_creature_takes_its_counters_or_makes_servos_by_choice(
    #[values(2, 3, 4)] seats: usize,
    #[case] spell: &str,
    #[case] color: Color,
    #[case] board: &[&str],
    #[case] take_counters: bool,
    #[case] counters: u32,
    #[case] tokens: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats, spell, color, 3, board);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast(spell)?;
    assert_eq!(resolve(&mut driver, take_counters)?, 1, "one question");
    assert_eq!(counters_on(&driver, spell), counters);
    assert_eq!(servos(&driver, 0), tokens);
    for seat in 1..seats {
        assert_eq!(servos(&driver, seat), 0);
    }
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}

#[rstest]
fn a_creature_that_is_gone_when_its_trigger_resolves_only_makes_the_servos(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut lands: Vec<_> = (0..3).map(|_| ready(land(Color::Green))).collect();
    lands.extend((0..3).map(|_| ready(land(Color::Red))));
    let scenario = Scenario::new(seats)
        .hand(
            0,
            [
                card("Highspire Artisan").unwrap(),
                card("Lightning Bolt").unwrap(),
            ],
        )
        .battlefield(0, lands);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Highspire Artisan")?;
    // Pass until the creature has entered and its trigger waits on the stack.
    for _ in 0..8 {
        let state = driver.state();
        let waiting = state
            .stack()
            .iter()
            .any(|id| state.object(*id).is_some_and(|o| o.ability().is_some()));
        if waiting {
            break;
        }
        driver.pass()?;
    }
    let target = driver.target_named("Highspire Artisan")?;
    driver.cast_at("Lightning Bolt", &[target])?;
    assert_eq!(
        resolve(&mut driver, true)?,
        0,
        "no question for a creature that is gone"
    );
    assert_eq!(servos(&driver, 0), 1);
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}

/// Seat 0 holds Pop Quiz and two creature cards, with three Islands.
fn quiz_table(seats: usize) -> Scenario {
    let bear = |name: &str| creature(name, 2, &[Color::Green], 2, 2);
    Scenario::new(seats)
        .hand(
            0,
            [card("Pop Quiz").unwrap(), bear("Bear A"), bear("Bear B")],
        )
        .battlefield(
            0,
            (0..3).map(|_| ready(land(Color::Blue))).collect::<Vec<_>>(),
        )
}

fn learned(driver: &Driver) -> usize {
    driver
        .engine()
        .events()
        .iter()
        .filter(|event| matches!(event, Event::Learned { .. }))
        .count()
}

#[rstest]
#[case::discard_the_first(Some(0), 3, 2)]
#[case::discard_the_second(Some(1), 3, 2)]
#[case::decline(None, 3, 1)]
fn learning_discards_a_card_to_draw_one_or_declines(
    #[values(2, 3, 4)] seats: usize,
    #[case] pick: Option<usize>,
    #[case] hand: usize,
    #[case] graveyard: usize,
) -> Result<(), ScriptError> {
    let scenario = quiz_table(seats);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Pop Quiz")?;
    let mut asked = 0;
    for _ in 0..40 {
        match &driver.decision()?.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            DecisionKind::ChooseCards(options) => {
                asked += 1;
                assert_eq!(
                    (options.min, options.max),
                    (0, 1),
                    "the discard is optional"
                );
                let chosen: Vec<_> = pick.map(|n| options.from[n]).into_iter().collect();
                driver.answer(Answer::ChooseCards(CardsChoice(chosen)))?;
            }
            other => panic!("unexpected {}", other.name()),
        }
    }
    assert_eq!(asked, 1);
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))).len(), hand);
    assert_eq!(
        driver.state().zone(Zone::Graveyard(pid(0))).len(),
        graveyard
    );
    assert_eq!(learned(&driver), 1);
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}

/// Seat 0 controls the ascender and `others` Plains; seat 1 controls `foes` Plains.
fn city(seats: usize, ascender: &str, others: usize, foes: usize) -> Scenario {
    let mut ours: Vec<_> = (0..others).map(|_| ready(land(Color::White))).collect();
    ours.push(ready(card(ascender).unwrap()));
    Scenario::new(seats).battlefield(0, ours).battlefield(
        1,
        (0..foes)
            .map(|_| ready(land(Color::White)))
            .collect::<Vec<_>>(),
    )
}

#[rstest]
#[case::ten_permanents(9, true)]
#[case::more_than_ten(14, true)]
#[case::nine_permanents(8, false)]
fn ascend_gives_the_blessing_at_ten_permanents(
    #[values(2, 3, 4)] seats: usize,
    #[case] others: usize,
    #[case] blessed: bool,
) -> Result<(), ScriptError> {
    let scenario = city(seats, "Skymarcher Aspirant", others, 12);
    let setup = scenario.clone().setup().unwrap();
    let driver = Driver::from_scenario(scenario)?;
    assert_eq!(driver.state().has_citys_blessing(pid(0)), blessed);
    assert_eq!(blessings(&driver), usize::from(blessed));
    // The opponent controls twelve permanents and none has ascend: nothing for them.
    assert!(!driver.state().has_citys_blessing(pid(1)));
    let id = driver.id_of("Skymarcher Aspirant")?;
    assert_eq!(has_keyword(driver.state(), id, &Keyword::Flying), blessed);
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}

#[rstest]
fn the_blessing_stays_once_gained_when_permanents_leave(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // Eight Plains, a Mountain and the Aspirant are ten permanents.
    let mut ours: Vec<_> = (0..8).map(|_| ready(land(Color::White))).collect();
    ours.push(ready(land(Color::Red)));
    ours.push(ready(card("Skymarcher Aspirant").unwrap()));
    let scenario = Scenario::new(seats)
        .hand(0, [card("Lightning Bolt").unwrap()])
        .battlefield(0, ours);
    let mut driver = Driver::from_scenario(scenario)?;
    assert!(driver.state().has_citys_blessing(pid(0)));
    let target = driver.target_named("Skymarcher Aspirant")?;
    driver.cast_at("Lightning Bolt", &[target])?;
    resolve(&mut driver, true)?;
    let aspirant = driver
        .state()
        .objects()
        .any(|o| o.card().name == "Skymarcher Aspirant" && o.zone() == Zone::Battlefield);
    assert!(!aspirant, "the Aspirant died");
    assert!(driver.state().has_citys_blessing(pid(0)));
    assert_eq!(blessings(&driver), 1, "given once");
    Ok(())
}
