//! Goal: vanishing and fading play whole at tables of two, three and four seats as their rules say: the permanent enters with its time or fade counters, the upkeep of its controller (and nobody else's) removes one, vanishing sacrifices it when the last is removed by anything and fading when an upkeep finds none, a counter put on in response saves a fading permanent, an intervening "if" keeps a vanishing permanent without counters from triggering, a change of control moves the upkeep, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{name_of, pid};
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_engine::decision::DecisionKind;
use mtg_engine::effects::{Duration, Modification};
use mtg_engine::manual::ManualAction;
use mtg_engine::setup::PresetPermanent;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, GameSetup, replay};
use mtg_testkit::cards::{free_creature, keyworded, keyworded_creature};
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

fn time() -> CounterKind {
    CounterKind::Other("time".into())
}

fn fade() -> CounterKind {
    CounterKind::Other("fade".into())
}

/// The log replays to the live state, and an engine rebuilt from it stands at the same decision.
#[allow(clippy::unnecessary_wraps)] // the tests end with it, as they end with their last script step
fn assert_replays(driver: &Driver, setup: &GameSetup) -> Result<(), ScriptError> {
    let log = driver.engine().events().to_vec();
    let replayed = replay(setup, &log).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    let resumed = Engine::resume(setup.clone(), log).unwrap();
    assert_eq!(resumed.state_hash(), driver.state().state_hash());
    assert_eq!(resumed.decision(), driver.engine().decision());
    Ok(())
}

/// Passes priority until the stack is empty.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    while !driver.state().stack().is_empty()
        && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
    {
        driver.pass()?;
    }
    Ok(())
}

fn counters_on(driver: &Driver, name: &str, kind: &CounterKind) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| permanent.counters_of(kind))
}

fn on_battlefield(driver: &Driver, name: &str) -> bool {
    driver.id_of(name).is_ok()
}

fn in_graveyard(driver: &Driver, seat: usize, name: &str) -> bool {
    driver
        .state()
        .zone(Zone::Graveyard(pid(seat)))
        .iter()
        .any(|id| name_of(driver.state(), *id) == name)
}

/// Seat 0 begins turn 1 in its first main phase with `hand` and `permanents`; every other seat has a creature.
fn table(
    seats: usize,
    hand: Vec<std::sync::Arc<mtg_cards::CardDefinition>>,
    permanents: Vec<PresetPermanent>,
) -> Scenario {
    let mut table = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, hand)
        .battlefield(0, permanents);
    for seat in 1..seats {
        table = table.battlefield(seat, [ready(free_creature("Bystander", 1, 1))]);
    }
    table
}

/// The turn on which seat 0 has its `nth` upkeep after the opening turn.
fn upkeep_turn(seats: usize, nth: u32) -> u32 {
    1 + u32::try_from(seats).unwrap() * nth
}

/// Casts the creature `name` from seat 0's hand and lets it resolve.
fn cast_and_resolve(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    driver.cast(name)?;
    settle(driver)
}

#[rstest]
fn a_permanent_counts_down_at_the_upkeeps_of_its_controller_and_is_sacrificed(
    #[values(2, 3, 4)] seats: usize,
    #[values(
        (Keyword::Vanishing(1), time(), 1, 1),
        (Keyword::Vanishing(3), time(), 3, 3),
        (Keyword::Fading(1), fade(), 1, 2),
        (Keyword::Fading(3), fade(), 3, 4),
    )]
    case: (Keyword, CounterKind, u32, u32),
) -> Result<(), ScriptError> {
    let (keyword, counter, start, last_upkeep) = case;
    let build = || {
        table(
            seats,
            vec![keyworded_creature(
                "Counter",
                0,
                &[],
                2,
                2,
                std::slice::from_ref(&keyword),
            )],
            vec![],
        )
    };
    let setup = build().setup()?;
    let mut driver = Driver::from_scenario(build())?;
    cast_and_resolve(&mut driver, "Counter")?;
    assert_eq!(
        counters_on(&driver, "Counter", &counter),
        start,
        "enters with its counters"
    );
    for nth in 1..=last_upkeep {
        // The upkeep of the next seat leaves the counters alone.
        driver.idle_until(upkeep_turn(seats, nth - 1) + 1, Step::Upkeep)?;
        assert_eq!(driver.state().stack(), []);
        assert_eq!(
            counters_on(&driver, "Counter", &counter),
            start.saturating_sub(nth - 1)
        );
        driver.idle_until(upkeep_turn(seats, nth), Step::Upkeep)?;
        assert_eq!(driver.state().stack().len(), 1, "the upkeep trigger waits");
        settle(&mut driver)?;
        if nth < last_upkeep {
            assert!(on_battlefield(&driver, "Counter"), "upkeep {nth}");
            assert_eq!(counters_on(&driver, "Counter", &counter), start - nth);
        }
    }
    assert!(!on_battlefield(&driver, "Counter"));
    assert!(in_graveyard(&driver, 0, "Counter"));
    assert_replays(&driver, &setup)
}

#[rstest]
fn the_last_time_counter_removed_by_anything_sacrifices_a_vanishing_permanent(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2)] held: u32,
) -> Result<(), ScriptError> {
    let build = || {
        table(
            seats,
            vec![],
            vec![with_counters(
                ready(keyworded("Vanisher", 2, 2, &[Keyword::Vanishing(4)])),
                time(),
                held,
            )],
        )
    };
    let setup = build().setup()?;
    let mut driver = Driver::from_scenario(build())?;
    let vanisher = driver.id_of("Vanisher")?;
    driver.manual(ManualAction::Counters {
        object: vanisher,
        counter: time(),
        delta: -1,
    })?;
    settle(&mut driver)?;
    // One counter off leaves a permanent with two counters alone and sacrifices one that had a single counter.
    assert_eq!(on_battlefield(&driver, "Vanisher"), held == 2);
    assert_eq!(in_graveyard(&driver, 0, "Vanisher"), held == 1);
    assert_replays(&driver, &setup)
}

#[rstest]
fn a_vanishing_permanent_without_counters_does_not_trigger_and_a_fading_one_is_sacrificed(
    #[values(2, 3, 4)] seats: usize,
    #[values(Keyword::Vanishing(2), Keyword::Fading(2))] keyword: Keyword,
) -> Result<(), ScriptError> {
    // A preset permanent did not enter through a spell and has no counters.
    let build = || {
        table(
            seats,
            vec![],
            vec![ready(keyworded(
                "Bare",
                2,
                2,
                std::slice::from_ref(&keyword),
            ))],
        )
    };
    let setup = build().setup()?;
    let mut driver = Driver::from_scenario(build())?;
    driver.idle_until(upkeep_turn(seats, 1), Step::Upkeep)?;
    let fading = matches!(keyword, Keyword::Fading(_));
    assert_eq!(driver.state().stack().len(), usize::from(fading));
    settle(&mut driver)?;
    assert_eq!(on_battlefield(&driver, "Bare"), !fading);
    assert_replays(&driver, &setup)
}

#[rstest]
fn a_fade_counter_put_on_in_response_saves_a_fading_permanent(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let build = || {
        table(
            seats,
            vec![],
            vec![ready(keyworded("Fader", 2, 2, &[Keyword::Fading(2)]))],
        )
    };
    let setup = build().setup()?;
    let mut driver = Driver::from_scenario(build())?;
    driver.idle_until(upkeep_turn(seats, 1), Step::Upkeep)?;
    assert_eq!(driver.state().stack().len(), 1);
    let fader = driver.id_of("Fader")?;
    driver.manual(ManualAction::Counters {
        object: fader,
        counter: fade(),
        delta: 2,
    })?;
    settle(&mut driver)?;
    assert!(on_battlefield(&driver, "Fader"));
    assert_eq!(
        counters_on(&driver, "Fader", &fade()),
        1,
        "one of the two came off"
    );
    assert_replays(&driver, &setup)
}

#[rstest]
fn the_upkeep_of_a_permanent_is_that_of_the_player_who_controls_it(
    #[values(2, 3, 4)] seats: usize,
    #[values(Keyword::Vanishing(2), Keyword::Fading(2))] keyword: Keyword,
) -> Result<(), ScriptError> {
    let counter = if matches!(keyword, Keyword::Fading(_)) {
        fade()
    } else {
        time()
    };
    let build = || {
        table(
            seats,
            vec![],
            vec![with_counters(
                ready(keyworded("Held", 2, 2, std::slice::from_ref(&keyword))),
                counter.clone(),
                2,
            )],
        )
    };
    let setup = build().setup()?;
    let mut driver = Driver::from_scenario(build())?;
    let held = driver.id_of("Held")?;
    driver.add_effect(
        driver.on(held),
        Modification::SetController(pid(1)),
        Duration::Forever,
    )?;
    // Seat 1's upkeep removes the counter; seat 0's own, a round later, finds nothing to do.
    driver.idle_until(2, Step::Upkeep)?;
    assert_eq!(driver.state().stack().len(), 1);
    settle(&mut driver)?;
    assert_eq!(counters_on(&driver, "Held", &counter), 1);
    driver.idle_until(upkeep_turn(seats, 1), Step::Upkeep)?;
    assert_eq!(driver.state().stack(), []);
    assert_replays(&driver, &setup)
}
