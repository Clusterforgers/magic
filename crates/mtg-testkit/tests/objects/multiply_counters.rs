//! Goal: doubling counters plays whole through Kalonian Hydra, Big Mother Mouser and Vorel of the Hull Clade at two, three and four seats: the attack trigger doubles the +1/+1 counters of each creature its controller has (none, one and many of them) and no one else's, a trigger on itself doubles its own, an ability doubles each kind a target holds and nothing for a target without counters, and the game replays from its log to the same counters.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{creature, forest, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};

use rstest::rstest;

fn charge() -> CounterKind {
    CounterKind::Other("charge".to_owned())
}

/// How many counters of a kind the permanent named holds.
fn held(driver: &Driver, name: &str, kind: &CounterKind) -> Result<u32, ScriptError> {
    let id = driver.id_of(name)?;
    Ok(driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| permanent.counters_of(kind)))
}

/// Passes until the stack is empty again: what was put on it resolves.
fn resolve(driver: &mut Driver) -> Result<Vec<Event>, ScriptError> {
    let start = driver.engine().events().len();
    while !driver.state().zone(Zone::Stack).is_empty() {
        driver.pass()?;
    }
    Ok(driver.engine().events()[start..].to_vec())
}

/// The game replays from its log to the same counters on the permanents named.
fn replays(driver: &Driver, names: &[&str]) -> Result<(), ScriptError> {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    for name in names {
        let id = driver.id_of(name)?;
        for kind in [CounterKind::PlusOnePlusOne, charge()] {
            let counters = |state: &mtg_engine::GameState| {
                state
                    .object(id)
                    .and_then(|object| object.permanent())
                    .map(|permanent| permanent.counters_of(&kind))
            };
            assert_eq!(counters(&state), counters(driver.state()), "{name}");
        }
    }
    Ok(())
}

/// Kalonian Hydra attacks with four +1/+1 counters beside a Wolf and, for each other seat, a Bear.
#[rstest]
#[case::alone(2, 0, 0, 8, 0, 0)]
#[case::a_creature_with_none(2, 0, 5, 8, 0, 5)]
#[case::a_creature_with_one(2, 1, 5, 8, 2, 5)]
#[case::a_creature_with_many(3, 3, 5, 8, 6, 5)]
#[case::at_four_seats(4, 7, 2, 8, 14, 2)]
fn kalonian_hydra_doubles_the_counters_of_each_creature_its_controller_has(
    #[case] seats: usize,
    #[case] wolf: u32,
    #[case] bear: u32,
    #[case] hydra_after: u32,
    #[case] wolf_after: u32,
    #[case] bear_after: u32,
) -> Result<(), ScriptError> {
    let plus = || CounterKind::PlusOnePlusOne;
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(
            0,
            [
                with_counters(ready(card("Kalonian Hydra").unwrap()), plus(), 4),
                with_counters(ready(creature("Wolf", 2, &[], 2, 2)), plus(), wolf),
            ],
        );
    for seat in 1..seats {
        scenario = scenario.battlefield(
            seat,
            [with_counters(
                ready(creature(&format!("Bear {seat}"), 2, &[], 2, 2)),
                plus(),
                bear,
            )],
        );
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.attack(&[("Kalonian Hydra", 1)])?;
    resolve(&mut driver)?;
    assert_eq!(held(&driver, "Kalonian Hydra", &plus())?, hydra_after);
    assert_eq!(held(&driver, "Wolf", &plus())?, wolf_after);
    for seat in 1..seats {
        assert_eq!(held(&driver, &format!("Bear {seat}"), &plus())?, bear_after);
    }
    replays(&driver, &["Kalonian Hydra", "Wolf"])
}

#[rstest]
#[case::two_counters(2, 2, 4)]
#[case::one_counter(3, 1, 2)]
#[case::many_counters(4, 9, 18)]
fn big_mother_mouser_doubles_its_own_counters_when_it_attacks(
    #[case] seats: usize,
    #[case] before: u32,
    #[case] after: u32,
) -> Result<(), ScriptError> {
    let plus = CounterKind::PlusOnePlusOne;
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(
                0,
                [with_counters(
                    ready(card("Big Mother Mouser").unwrap()),
                    plus.clone(),
                    before,
                )],
            ),
    )?;
    driver.attack(&[("Big Mother Mouser", 1)])?;
    resolve(&mut driver)?;
    assert_eq!(held(&driver, "Big Mother Mouser", &plus)?, after);
    replays(&driver, &["Big Mother Mouser"])
}

/// Vorel of the Hull Clade doubles each kind of counter on its target for {G}{U}.
#[rstest]
#[case::two_kinds(2, 3, 2, 6, 4)]
#[case::one_kind(3, 4, 0, 8, 0)]
#[case::no_counters(2, 0, 0, 0, 0)]
#[case::a_single_counter(4, 1, 1, 2, 2)]
fn vorel_doubles_each_kind_of_counter_on_its_target(
    #[case] seats: usize,
    #[case] plus: u32,
    #[case] charged: u32,
    #[case] plus_after: u32,
    #[case] charge_after: u32,
) -> Result<(), ScriptError> {
    let target = with_counters(
        with_counters(
            ready(creature("Wolf", 2, &[], 2, 2)),
            CounterKind::PlusOnePlusOne,
            plus,
        ),
        charge(),
        charged,
    );
    let mut driver = Driver::from_scenario(Scenario::new(seats).battlefield(
        0,
        [
            ready(card("Vorel of the Hull Clade").unwrap()),
            ready(forest()),
            ready(island()),
            target,
        ],
    ))?;
    driver.begin_activation("Vorel of the Hull Clade")?;
    let wolf = driver.target_named("Wolf")?;
    driver.choose_targets(&[wolf])?;
    driver.pay_tapping(&["Forest", "Island"], 0)?;
    resolve(&mut driver)?;
    assert_eq!(
        held(&driver, "Wolf", &CounterKind::PlusOnePlusOne)?,
        plus_after
    );
    assert_eq!(held(&driver, "Wolf", &charge())?, charge_after);
    replays(&driver, &["Wolf"])
}
