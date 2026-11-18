//! Goal: a reusable Satchel card (a Delves boss's) is back in the Satchel after it resolves, after a counter, after it
//! loses its target and after its permanent dies, costing {2} more for each earlier cast; a once card is gone after
//! one cast, whatever happened to it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, island, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// How the spell from the Satchel leaves the stack.
#[derive(Debug, Clone, Copy)]
enum Ends {
    /// Lightning Bolt at seat 1 resolves.
    Resolves,
    /// Seat 1 counters Lightning Bolt with Counterspell.
    Countered,
    /// Seat 1 bolts the Grizzly Bears that Giant Growth targets, so it is removed from the stack as it would resolve.
    Fizzles,
    /// Grizzly Bears resolves, and seat 1 bolts it.
    Dies,
}

impl Ends {
    fn card(self) -> &'static str {
        match self {
            Ends::Resolves | Ends::Countered => "Lightning Bolt",
            Ends::Fizzles => "Giant Growth",
            Ends::Dies => "Grizzly Bears",
        }
    }
}

/// Seat 0 holds the card in its Satchel, reusable or once, and Grizzly Bears on the battlefield for Giant Growth; seat
/// 1 holds Counterspell and Lightning Bolt with two Islands and a Mountain.
fn table(seats: usize, ends: Ends, reusable: bool) -> Scenario {
    let satchel = vec![card(ends.card()).unwrap()];
    let scenario = Scenario::new(seats);
    let scenario = if reusable {
        scenario.satchel_reusable(0, satchel)
    } else {
        scenario.satchel(0, satchel)
    };
    let mut lands: Vec<_> = (0..6).map(|_| ready(mountain())).collect();
    lands.extend((0..4).map(|_| ready(forest())));
    let mut own = vec![ready(card("Grizzly Bears").unwrap())];
    own.extend(lands);
    scenario
        .battlefield(0, own)
        .hand(
            1,
            [
                card("Counterspell").unwrap(),
                card("Lightning Bolt").unwrap(),
            ],
        )
        .battlefield(1, [ready(island()), ready(island()), ready(mountain())])
}

/// The object `id` as a target, as it is now.
fn object_target(driver: &Driver, id: mtg_core::ids::ObjectId) -> mtg_engine::choose::Target {
    mtg_engine::choose::Target::Object(driver.state().object(id).unwrap().reference())
}

fn resolve_stack(driver: &mut Driver) -> Result<(), ScriptError> {
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    Ok(())
}

/// Seat 0 casts the card in its first main phase and seat 1 answers it as `ends` says.
fn play(driver: &mut Driver, ends: Ends) -> Result<(), ScriptError> {
    let satchel = driver.state().satchel(pid(0))[0];
    match ends {
        Ends::Resolves => {
            let target = driver.target_player(1);
            driver.cast_at("Lightning Bolt", &[target])?;
        }
        Ends::Countered => {
            let target = driver.target_player(1);
            driver.cast_at("Lightning Bolt", &[target])?;
            driver.pass()?;
            let spell = object_target(driver, satchel);
            driver.cast_at("Counterspell", &[spell])?;
        }
        Ends::Fizzles => {
            let bears = driver.target_named("Grizzly Bears")?;
            driver.cast_at("Giant Growth", &[bears])?;
            driver.pass()?;
            driver.cast_at("Lightning Bolt", &[bears])?;
        }
        Ends::Dies => {
            driver.cast("Grizzly Bears")?;
            resolve_stack(driver)?;
            let bears = object_target(driver, satchel);
            driver.pass()?;
            driver.cast_at("Lightning Bolt", &[bears])?;
        }
    }
    resolve_stack(driver)
}

#[rstest]
fn a_reusable_card_returns_and_a_once_card_does_not(
    #[values(Ends::Resolves, Ends::Countered, Ends::Fizzles, Ends::Dies)] ends: Ends,
    #[values(true, false)] reusable: bool,
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, ends, reusable))?;
    driver.idle_until(1, Step::Main1)?;
    let id = driver.state().satchel(pid(0))[0];
    play(&mut driver, ends)?;
    let state = driver.state();
    assert_eq!(state.satchel_reusable(id), reusable);
    if reusable {
        assert_eq!(state.satchel(pid(0)), [id], "{ends:?}");
        assert_eq!(state.satchel_casts(id), 1);
        assert_eq!(state.satchel_tax(id), 2);
    } else {
        assert!(state.satchel(pid(0)).is_empty(), "{ends:?}");
        assert_eq!(state.object(id).unwrap().zone(), Zone::Graveyard(pid(0)));
        assert_eq!(state.satchel_tax(id), 0);
    }
    Ok(())
}

#[rstest]
#[case::first(0, 1)]
#[case::second(1, 3)]
#[case::third(2, 5)]
fn each_earlier_cast_adds_two_to_the_cost(
    #[case] earlier: u32,
    #[case] lands: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(2, Ends::Resolves, true))?;
    driver.idle_until(1, Step::Main1)?;
    let id = driver.state().satchel(pid(0))[0];
    for _ in 0..earlier {
        let target = driver.target_player(1);
        driver.cast_at("Lightning Bolt", &[target])?;
        resolve_stack(&mut driver)?;
    }
    let untapped = |driver: &Driver| {
        driver
            .state()
            .zone(Zone::Battlefield)
            .iter()
            .filter_map(|id| driver.state().object(*id))
            .filter(|object| object.controller() == pid(0) && object.card().name != "Grizzly Bears")
            .filter(|object| object.permanent().is_some_and(|p| !p.tapped()))
            .count()
    };
    let before = untapped(&driver);
    let target = driver.target_player(1);
    driver.cast_at("Lightning Bolt", &[target])?;
    assert_eq!(before - untapped(&driver), lands);
    resolve_stack(&mut driver)?;
    assert_eq!(driver.state().satchel_casts(id), earlier + 1);
    Ok(())
}
