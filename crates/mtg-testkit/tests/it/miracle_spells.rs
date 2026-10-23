//! Goal: the spells that move every object a phrase names (Terminus, Devastation Tide, Redress Fate) and Reforge the Soul run for each seat of a table, and a miracle cost with X (Entreat the Angels) announces its own X in the cost that replaces the printed one, while the printed cost still pays X for each of its symbols, at two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CastWay;
use mtg_engine::choose::{NumberChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::object::PermanentState;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, PresetPermanent};
use mtg_testkit::cards::{forest, free_creature, island, mountain, plains, study};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::{lands, pay};
use crate::common::{name_of, names, pid};

/// Everyone passes: the top of the stack resolves.
fn settle(driver: &mut Driver, seats: usize) -> Result<(), ScriptError> {
    for _ in 0..seats {
        driver.pass()?;
    }
    Ok(())
}

/// A table where every seat holds a creature called `Bear <seat>` and three Forests in the library, and seat 0 holds
/// the card `spell` with `lands` on the table.
fn table(seats: usize, spell: &str, mana: &[PresetPermanent]) -> Scenario {
    let mut scenario = Scenario::new(seats).hand_named(0, [spell]);
    for seat in 0..seats {
        let mut permanents = vec![ready(free_creature(&format!("Bear {seat}"), 2, 2))];
        if seat == 0 {
            permanents.extend(mana.to_vec());
        }
        scenario = scenario
            .battlefield(seat, permanents)
            .library(seat, vec![forest(); 3]);
    }
    scenario
}

fn cast_and_resolve(scenario: Scenario, spell: &str, seats: usize) -> Result<Driver, ScriptError> {
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast(spell)?;
    pay(&mut driver)?;
    settle(&mut driver, seats)?;
    Ok(driver)
}

#[rstest]
fn terminus_puts_every_creature_on_the_bottom_of_its_owners_library(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let driver = cast_and_resolve(
        table(seats, "Terminus", &lands(6, plains)),
        "Terminus",
        seats,
    )?;
    let state = driver.state();
    assert!(
        !names(state, Zone::Battlefield)
            .iter()
            .any(|name| name.starts_with("Bear"))
    );
    for seat in 0..seats {
        let library = names(state, Zone::Library(pid(seat)));
        assert_eq!(library.len(), 4, "seat {seat}");
        assert_eq!(
            library.first(),
            Some(&format!("Bear {seat}")),
            "seat {seat}"
        );
        assert_eq!(library.last().map(String::as_str), Some("Forest"));
    }
    Ok(())
}

#[rstest]
fn devastation_tide_returns_every_nonland_permanent_and_keeps_the_lands(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let driver = cast_and_resolve(
        table(seats, "Devastation Tide", &lands(5, island)),
        "Devastation Tide",
        seats,
    )?;
    let state = driver.state();
    assert_eq!(names(state, Zone::Battlefield), vec!["Island"; 5]);
    for seat in 0..seats {
        assert!(
            names(state, Zone::Hand(pid(seat))).contains(&format!("Bear {seat}")),
            "seat {seat}"
        );
    }
    Ok(())
}

#[rstest]
fn redress_fate_returns_the_artifacts_and_enchantments_of_your_graveyard_only(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats, "Redress Fate", &lands(8, plains))
        .graveyard_named(0, ["Sol Ring", "Greater Good", "Terminus"])
        .graveyard_named(1, ["Sol Ring"]);
    let driver = cast_and_resolve(scenario, "Redress Fate", seats)?;
    let state = driver.state();
    let on_table = names(state, Zone::Battlefield);
    assert_eq!(
        on_table.iter().filter(|name| *name == "Sol Ring").count(),
        1
    );
    assert!(on_table.contains(&"Greater Good".to_owned()));
    assert!(!on_table.contains(&"Terminus".to_owned()));
    assert_eq!(names(state, Zone::Graveyard(pid(1))), ["Sol Ring"]);
    assert!(names(state, Zone::Graveyard(pid(0))).contains(&"Terminus".to_owned()));
    Ok(())
}

#[rstest]
fn reforge_the_soul_gives_every_seat_a_new_hand_of_seven(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 2, 5)] held: usize,
) -> Result<(), ScriptError> {
    let mut scenario = table(seats, "Reforge the Soul", &lands(5, mountain));
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 9]);
    }
    let extra = vec![study("Held"); held];
    let mut hand = vec![mtg_testkit::corpus::card("Reforge the Soul").unwrap()];
    hand.extend(extra.clone());
    scenario = scenario.hand(0, hand);
    for seat in 1..seats {
        scenario = scenario.hand(seat, extra.clone());
    }
    let driver = cast_and_resolve(scenario, "Reforge the Soul", seats)?;
    let state = driver.state();
    for seat in 0..seats {
        assert_eq!(state.zone(Zone::Hand(pid(seat))).len(), 7, "seat {seat}");
        assert!(
            names(state, Zone::Hand(pid(seat)))
                .iter()
                .all(|name| name == "Forest"),
            "seat {seat}"
        );
        let held_in_graveyard = names(state, Zone::Graveyard(pid(seat)))
            .iter()
            .filter(|name| *name == "Held")
            .count();
        assert_eq!(held_in_graveyard, held, "seat {seat}");
    }
    assert!(names(state, Zone::Graveyard(pid(0))).contains(&"Reforge the Soul".to_owned()));
    Ok(())
}

/// Seat 0 holds Study and Entreat the Angels is on top of its library; the table has an Island and `plains` Plains.
fn miracle_table(seats: usize, plains_count: usize) -> Scenario {
    let mut battlefield = vec![ready(island())];
    battlefield.extend(lands(plains_count, plains));
    let mut library = vec![mtg_testkit::corpus::card("Entreat the Angels").unwrap()];
    library.extend(vec![forest(); 8]);
    let mut scenario = Scenario::new(seats)
        .hand(0, vec![study("Study")])
        .library(0, library)
        .battlefield(0, battlefield);
    for seat in 1..seats {
        scenario = scenario.library(seat, vec![forest(); 8]);
    }
    scenario
}

/// Study resolves, the trigger resolves and the question of the miracle is pending.
fn asked(seats: usize, plains_count: usize) -> Result<Driver, ScriptError> {
    let mut driver = Driver::from_scenario(miracle_table(seats, plains_count))?;
    driver.cast("Study")?;
    settle(&mut driver, seats)?;
    settle(&mut driver, seats)?;
    Ok(driver)
}

fn angels(driver: &Driver) -> usize {
    names(driver.state(), Zone::Battlefield)
        .iter()
        .filter(|name| name.contains("Angel"))
        .count()
}

#[rstest]
fn a_miracle_cost_with_x_announces_x_and_pays_one_mana_for_each(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 3)] x: u32,
) -> Result<(), ScriptError> {
    // Seven Plains: {W}{W} and up to five for X.
    let mut driver = asked(seats, 7)?;
    let DecisionKind::YesNo(question) = &driver.decision()?.kind else {
        panic!("the miracle is asked: {}", driver.decision()?.kind.name());
    };
    assert_eq!(
        question.text,
        "Cast Entreat the Angels for its miracle cost {X}{W}{W}?"
    );
    driver.answer(Answer::YesNo(YesNoChoice(true)))?;
    let DecisionKind::ChooseNumber(options) = &driver.decision()?.kind else {
        panic!("X is asked: {}", driver.decision()?.kind.name());
    };
    assert_eq!((options.min, options.max), (0, Some(5)));
    driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
    let events = driver.answer(Answer::PayMana(PayChoice::Auto))?;
    let way = events.iter().find_map(|event| match event {
        Event::SpellCast { choices, .. } => Some((choices.way, choices.x)),
        _ => None,
    });
    assert_eq!(way, Some((Some(CastWay::Miracle), Some(x))));
    let tapped = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| name_of(driver.state(), **id) == "Plains")
        .filter(|id| {
            driver
                .state()
                .object(**id)
                .is_some_and(|o| o.permanent().is_some_and(PermanentState::tapped))
        })
        .count();
    assert_eq!(tapped, usize::try_from(x).unwrap() + 2);
    settle(&mut driver, seats)?;
    assert_eq!(angels(&driver), usize::try_from(x).unwrap());
    Ok(())
}

#[rstest]
fn a_miracle_with_x_and_only_the_colored_mana_has_one_value_of_x(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = asked(seats, 2)?;
    driver.answer(Answer::YesNo(YesNoChoice(true)))?;
    assert_eq!(
        driver.decision()?.kind.name(),
        "pay-mana",
        "X is 0, not asked"
    );
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver, seats)?;
    assert_eq!(angels(&driver), 0);
    Ok(())
}

#[rstest]
fn a_miracle_with_x_that_cannot_pay_its_colored_mana_is_not_asked(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let driver = asked(seats, 1)?;
    assert_eq!(driver.state().zone(Zone::Stack), []);
    assert!(names(driver.state(), Zone::Hand(pid(0))).contains(&"Entreat the Angels".to_owned()));
    Ok(())
}

#[rstest]
fn the_printed_cost_pays_x_for_each_of_its_two_symbols(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] x: u32,
) -> Result<(), ScriptError> {
    // {X}{X}{W}{W}{W} with eight Plains: three for the colors and two for each X, up to 2.
    let mut scenario = Scenario::new(seats)
        .hand_named(0, ["Entreat the Angels"])
        .battlefield(0, lands(8, plains));
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 3]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Entreat the Angels")?;
    let DecisionKind::ChooseNumber(options) = &driver.decision()?.kind else {
        panic!("X is asked: {}", driver.decision()?.kind.name());
    };
    assert_eq!((options.min, options.max), (0, Some(2)));
    driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
    pay(&mut driver)?;
    let tapped = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            driver
                .state()
                .object(**id)
                .is_some_and(|o| o.permanent().is_some_and(PermanentState::tapped))
        })
        .count();
    assert_eq!(tapped, 3 + 2 * usize::try_from(x).unwrap());
    settle(&mut driver, seats)?;
    assert_eq!(angels(&driver), usize::try_from(x).unwrap());
    Ok(())
}

#[rstest]
fn terminus_ends_a_token_and_orders_only_the_cards(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats, "Terminus", &lands(11, plains))
        .hand_named(0, ["Entreat the Angels", "Terminus"]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Entreat the Angels")?;
    driver.answer(Answer::ChooseNumber(NumberChoice::Number(1)))?;
    pay(&mut driver)?;
    settle(&mut driver, seats)?;
    assert_eq!(angels(&driver), 1);
    driver.begin_cast("Terminus")?;
    pay(&mut driver)?;
    settle(&mut driver, seats)?;
    assert_eq!(angels(&driver), 0);
    let library = names(driver.state(), Zone::Library(pid(0)));
    assert_eq!(
        library.len(),
        4,
        "the token is not in the library: {library:?}"
    );
    Ok(())
}
