//! Goal: the day and night effects and conditions work at tables of two, three and four seats: "it becomes day" and "it becomes night" set the designation and log nothing when it already is that, "if it's neither day nor night, it becomes day as ~ enters" starts the day once (also for creatures that enter together), "whenever day becomes night or night becomes day" fires for each change between the two and never for the first day, the turn change counts the same way, a daybound permanent turns over at once when an effect makes it night, a spell that costs less at night and the toggle of The Celestus read the designation, and every log replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::DayNight::{self, Day, Night};
use mtg_engine::characteristics::characteristics;
use mtg_engine::decision::DecisionKind;
use mtg_engine::setup::PresetPermanent;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{forest, island, mountain, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, day_maker, mass_landing, ready};
use rstest::rstest;

use crate::common::{name_of, names, pid};

/// A table of `seats` where seat 0 holds `hand` and controls `field`, in its first main phase.
fn table(seats: usize, hand: Vec<Arc<CardDefinition>>, field: Vec<PresetPermanent>) -> Scenario {
    let mut scenario = Scenario::new(seats).at(1, 0, Step::Main1);
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 30]);
    }
    scenario.hand(0, hand).battlefield(0, field)
}

fn lands(count: usize) -> Vec<PresetPermanent> {
    (0..count).map(|_| ready(mountain())).collect()
}

/// Every land the cards of the tables need: two Mountains, two Plains and two Islands.
fn mixed_lands() -> Vec<PresetPermanent> {
    let mut found = Vec::new();
    for _ in 0..2 {
        found.extend([ready(mountain()), ready(plains()), ready(island())]);
    }
    found
}

/// Answers every decision passively until the stack is empty, which resolves what is on it.
fn resolve_all(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack never emptied".to_owned()))
}

/// Casts a free card and lets it resolve.
fn cast_free(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    driver.cast(name)?;
    resolve_all(driver)
}

fn changes(driver: &Driver) -> Vec<DayNight> {
    driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::DayNightChanged { to } => Some(*to),
            _ => None,
        })
        .collect()
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn replays(driver: &Driver) {
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    assert_eq!(replayed.day_night(), driver.state().day_night());
}

#[rstest]
fn a_permanent_starts_the_day_as_it_enters_only_when_it_is_neither_day_nor_night(
    #[values(2, 3, 4)] seats: usize,
    #[values(None, Some(Day), Some(Night))] before: Option<DayNight>,
    #[values("Sunrise Cavalier", "Firmament Sage")] name: &str,
) -> Result<(), ScriptError> {
    let hand = vec![
        day_maker("Dawn", Day),
        day_maker("Dusk", Night),
        card(name).unwrap(),
    ];
    let mut driver = Driver::from_scenario(table(seats, hand, mixed_lands()))?;
    match before {
        Some(Day) => cast_free(&mut driver, "Dawn")?,
        Some(Night) => cast_free(&mut driver, "Dusk")?,
        None => {}
    }
    let paid: &[&str] = if name == "Sunrise Cavalier" {
        &["Mountain", "Plains", "Island"]
    } else {
        &["Island", "Island", "Mountain", "Plains"]
    };
    driver.cast_tapping(name, paid)?;
    resolve_all(&mut driver)?;
    assert!(names(driver.state(), Zone::Battlefield).contains(&name.to_owned()));
    assert_eq!(driver.state().day_night(), Some(before.unwrap_or(Day)));
    // The one change is the first one: the card's own, or the spell's that came before it.
    assert_eq!(changes(&driver), [before.unwrap_or(Day)]);
    // The first day is no change between day and night, so the cards' triggers wait for a real one.
    let triggers = driver
        .engine()
        .events()
        .iter()
        .filter(|event| matches!(event, Event::AbilityPutOnStack(_)))
        .count();
    assert_eq!(triggers, 0);
    replays(&driver);
    Ok(())
}

#[rstest]
fn creatures_that_enter_together_start_the_day_once(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] creatures: usize,
) -> Result<(), ScriptError> {
    let mut hand = vec![mass_landing("Landing")];
    hand.extend((0..creatures).map(|_| card("Firmament Sage").unwrap()));
    let mut driver = Driver::from_scenario(table(seats, hand, Vec::new()))?;
    cast_free(&mut driver, "Landing")?;
    let sages = names(driver.state(), Zone::Battlefield)
        .iter()
        .filter(|name| *name == "Firmament Sage")
        .count();
    assert_eq!(sages, creatures);
    assert_eq!(changes(&driver), [Day]);
    assert_eq!(driver.state().day_night(), Some(Day));
    replays(&driver);
    Ok(())
}

#[rstest]
#[case::night_from_neither(&[Night], &[Night], 0)]
#[case::day_from_neither(&[Day], &[Day], 0)]
#[case::night_again(&[Night, Night], &[Night], 0)]
#[case::night_then_day(&[Night, Day], &[Night, Day], 1)]
#[case::day_then_night(&[Day, Night], &[Day, Night], 1)]
#[case::day_twice_then_night(&[Day, Day, Night], &[Day, Night], 1)]
#[case::four_makers_three_changes(&[Day, Night, Day, Night], &[Day, Night, Day, Night], 3)]
#[case::night_day_day(&[Night, Day, Day], &[Night, Day], 1)]
fn becoming_day_or_night_triggers_a_watcher_for_each_change_between_the_two_and_logs_no_other(
    #[values(2, 3, 4)] seats: usize,
    #[case] casts: &[DayNight],
    #[case] logged: &[DayNight],
    #[case] damage: i32,
) -> Result<(), ScriptError> {
    let hand: Vec<_> = casts
        .iter()
        .enumerate()
        .map(|(at, to)| day_maker(&format!("Maker {at}"), *to))
        .collect();
    // Seat 1 controls a Brimstone Vandal, which pings each opponent when day becomes night or night becomes day.
    let mut scenario = table(seats, hand, Vec::new());
    scenario = scenario.battlefield(1, [ready(card("Brimstone Vandal").unwrap())]);
    let mut driver = Driver::from_scenario(scenario)?;
    for at in 0..casts.len() {
        cast_free(&mut driver, &format!("Maker {at}"))?;
    }
    assert_eq!(changes(&driver), logged);
    assert_eq!(driver.state().day_night(), casts.last().copied());
    let story: Vec<String> = driver
        .engine()
        .events()
        .iter()
        .filter(|event| {
            matches!(
                event,
                Event::DayNightChanged { .. }
                    | Event::AbilityPutOnStack(_)
                    | Event::LifeLost { .. }
            )
        })
        .map(|event| format!("{event:?}"))
        .collect();
    for seat in 0..seats {
        assert_eq!(
            life(&driver, seat),
            if seat == 1 { 20 } else { 20 - damage },
            "{story:#?}"
        );
    }
    replays(&driver);
    Ok(())
}

/// Every seat has forests to cast with and Fog to cast, seat 1 controls a Tavern Ruffian, which starts the day, and a
/// Brimstone Vandal, which pings each opponent whenever day becomes night or night becomes day.
fn turns(seats: usize) -> Result<Driver, ScriptError> {
    let mut scenario = Scenario::new(seats).at(1, 0, Step::Main1);
    for seat in 0..seats {
        let watchers = if seat == 1 {
            vec![
                ready(card("Tavern Ruffian // Tavern Smasher").unwrap()),
                ready(card("Brimstone Vandal").unwrap()),
            ]
        } else {
            Vec::new()
        };
        scenario = scenario
            .hand_named(seat, ["Fog"; 6])
            .library(seat, vec![forest(); 30])
            .battlefield(
                seat,
                watchers.into_iter().chain((0..6).map(|_| ready(forest()))),
            );
    }
    Driver::from_scenario(scenario)
}

#[rstest]
#[case::no_spell_makes_night(&[0], 1)]
#[case::one_spell_keeps_the_day(&[1], 0)]
#[case::two_spells_keep_the_day(&[2], 0)]
#[case::night_after_night(&[0, 0], 1)]
#[case::one_spell_keeps_the_night(&[0, 1], 1)]
#[case::two_spells_make_day(&[0, 2], 2)]
#[case::three_spells_make_day(&[0, 3], 2)]
#[case::day_night_day_night(&[0, 2, 0], 3)]
#[case::spells_do_not_carry_over(&[0, 1, 1, 2], 2)]
fn the_turn_change_triggers_a_watcher_for_each_change_and_never_for_the_first_day(
    #[values(2, 3, 4)] seats: usize,
    #[case] casts: &[usize],
    #[case] expected: i32,
) -> Result<(), ScriptError> {
    let mut driver = turns(seats)?;
    for (turn, spells) in casts.iter().enumerate() {
        driver.idle_until(u32::try_from(turn).unwrap() + 1, Step::Main1)?;
        for _ in 0..*spells {
            driver.cast_tapping("Fog", &["Forest"])?;
            resolve_all(&mut driver)?;
        }
    }
    driver.idle_until(u32::try_from(casts.len()).unwrap() + 1, Step::Upkeep)?;
    resolve_all(&mut driver)?;
    let began = driver
        .engine()
        .events()
        .iter()
        .filter(|event| matches!(event, Event::DayNightChanged { .. }))
        .count();
    // The first day (the Ruffian's) is not a change between the two.
    assert_eq!(
        i32::try_from(began).unwrap(),
        expected + 1,
        "{:?}",
        changes(&driver)
    );
    for seat in (0..seats).filter(|seat| *seat != 1) {
        assert_eq!(life(&driver, seat), 20 - expected, "seat {seat}");
    }
    assert_eq!(life(&driver, 1), 20);
    replays(&driver);
    Ok(())
}

#[rstest]
#[case::night_turns_it_over(&[Night], "Tavern Smasher")]
#[case::day_changes_nothing(&[Day], "Tavern Ruffian")]
#[case::night_then_day(&[Night, Day], "Tavern Ruffian")]
#[case::day_then_night(&[Day, Night], "Tavern Smasher")]
#[case::night_twice(&[Night, Night], "Tavern Smasher")]
fn an_effect_that_makes_it_night_turns_daybound_permanents_over_at_once(
    #[values(2, 3, 4)] seats: usize,
    #[case] casts: &[DayNight],
    #[case] face: &str,
) -> Result<(), ScriptError> {
    let hand: Vec<_> = casts
        .iter()
        .enumerate()
        .map(|(at, to)| day_maker(&format!("Maker {at}"), *to))
        .collect();
    let mut scenario = table(seats, hand, Vec::new());
    scenario = scenario.battlefield(
        1,
        [ready(card("Tavern Ruffian // Tavern Smasher").unwrap())],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    // The permanent starts the day when the game begins.
    assert_eq!(driver.state().day_night(), Some(Day));
    for at in 0..casts.len() {
        cast_free(&mut driver, &format!("Maker {at}"))?;
    }
    let ruffian = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| name_of(driver.state(), *id).starts_with("Tavern"))
        .unwrap();
    assert_eq!(name_of(driver.state(), ruffian), face);
    let found = characteristics(driver.state(), ruffian).unwrap();
    assert_eq!(
        found.power,
        Some(if face == "Tavern Smasher" { 6 } else { 2 })
    );
    replays(&driver);
    Ok(())
}

#[rstest]
fn a_spell_that_costs_less_at_night_is_castable_with_less_mana_only_at_night(
    #[values(2, 3, 4)] seats: usize,
    #[values(None, Some(Day), Some(Night))] now: Option<DayNight>,
    #[values(1, 3)] mountains: usize,
) -> Result<(), ScriptError> {
    let hand = vec![
        day_maker("Dawn", Day),
        day_maker("Dusk", Night),
        card("Moonrager's Slash").unwrap(),
    ];
    let mut driver = Driver::from_scenario(table(seats, hand, lands(mountains)))?;
    match now {
        Some(Day) => cast_free(&mut driver, "Dawn")?,
        Some(Night) => cast_free(&mut driver, "Dusk")?,
        None => {}
    }
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        panic!("priority was expected");
    };
    let castable: Vec<String> = options
        .castable
        .iter()
        .map(|id| name_of(driver.state(), *id))
        .collect();
    let affordable = mountains == 3 || now == Some(Night);
    assert_eq!(
        castable.contains(&"Moonrager's Slash".to_owned()),
        affordable,
        "{castable:?}"
    );
    if affordable {
        let target = driver.target_player(seats - 1);
        driver.cast_at("Moonrager's Slash", &[target])?;
        resolve_all(&mut driver)?;
        assert_eq!(life(&driver, seats - 1), 17);
    }
    replays(&driver);
    Ok(())
}

#[rstest]
#[case::from_neither(None, Night, 0)]
#[case::from_day(Some(Day), Night, 1)]
#[case::from_night(Some(Night), Day, 1)]
fn the_toggle_of_the_celestus_makes_it_the_other_one_and_night_when_it_is_neither(
    #[values(2, 3, 4)] seats: usize,
    #[case] before: Option<DayNight>,
    #[case] after: DayNight,
    #[case] gained: i32,
) -> Result<(), ScriptError> {
    let hand = vec![day_maker("Dawn", Day), day_maker("Dusk", Night)];
    let field: Vec<PresetPermanent> = lands(3)
        .into_iter()
        .chain([ready(card("The Celestus").unwrap())])
        .collect();
    let mut driver = Driver::from_scenario(table(seats, hand, field))?;
    match before {
        Some(Day) => cast_free(&mut driver, "Dawn")?,
        Some(Night) => cast_free(&mut driver, "Dusk")?,
        None => {}
    }
    driver.begin_activation("The Celestus")?;
    driver.pay_tapping(&["Mountain"; 3], 0)?;
    resolve_all(&mut driver)?;
    assert_eq!(driver.state().day_night(), Some(after));
    // The Celestus gains its controller 1 life whenever day becomes night or night becomes day; it may loot, and a
    // passive player declines.
    assert_eq!(life(&driver, 0), 20 + gained);
    replays(&driver);
    Ok(())
}
