//! Goal: "each green creature that entered this turn" is exactly the creatures that came onto the battlefield during the turn: Oran-Rief, the Vastwood gives its counters to a green creature cast this turn and never to a green creature that was there before or to one cast after the ability resolved, and the log of such games replays to the live state, at tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_engine::turn::Step;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{bear, elf, forest};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const RIEF: &str = "Oran-Rief, the Vastwood";

/// Seat 0 controls Oran-Rief, a Forest and a Grizzly Bears that has been there all along, and holds a Llanowar Elf.
fn table(seats: usize) -> Scenario {
    Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [elf()])
        .battlefield(
            0,
            [ready(card(RIEF).unwrap()), ready(forest()), ready(bear())],
        )
}

/// Answers passively until the stack is empty and nothing resolves.
fn settle(driver: &mut Driver) -> Result<Vec<Event>, ScriptError> {
    let mut events = Vec::new();
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(events);
        }
        events.extend(driver.answer_passively()?);
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// The names of the creatures that were given a +1/+1 counter by these events.
fn grown(driver: &Driver, events: &[Event]) -> Vec<String> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::CountersChanged(change)
                if change.delta > 0 && change.counter == CounterKind::PlusOnePlusOne =>
            {
                change.on.object()
            }
            _ => None,
        })
        .map(|object| {
            driver
                .state()
                .object(object.id)
                .unwrap()
                .card()
                .name
                .clone()
        })
        .collect()
}

fn give_counters(driver: &mut Driver) -> Result<Vec<Event>, ScriptError> {
    let abilities = driver.offered_abilities(RIEF)?;
    let counters = *abilities.last().unwrap();
    let mut events = driver.begin_ability(RIEF, counters)?;
    events.extend(settle(driver)?);
    Ok(events)
}

#[rstest]
fn the_counters_go_to_the_creature_cast_this_turn_and_to_nothing_else(
    #[values(2, 3, 4)] seats: usize,
) {
    let mut driver = Driver::from_scenario(table(seats)).unwrap();
    let mut events = driver.cast_tapping("Llanowar Elf", &["Forest"]).unwrap();
    events.extend(settle(&mut driver).unwrap());
    events.extend(give_counters(&mut driver).unwrap());
    assert_eq!(grown(&driver, &events), ["Llanowar Elf"]);
    let setup = table(seats).setup().unwrap();
    assert_eq!(
        replay(&setup, &events_of(&driver)).unwrap().state_hash(),
        driver.state().state_hash()
    );
}

#[rstest]
fn a_creature_cast_after_the_ability_resolved_gets_nothing(#[values(2, 3, 4)] seats: usize) {
    let mut driver = Driver::from_scenario(table(seats)).unwrap();
    let mut events = give_counters(&mut driver).unwrap();
    events.extend(driver.cast_tapping("Llanowar Elf", &["Forest"]).unwrap());
    events.extend(settle(&mut driver).unwrap());
    assert_eq!(grown(&driver, &events), Vec::<String>::new());
}

#[rstest]
fn bot_games_with_the_entered_this_turn_cards_keep_every_invariant_and_replay(
    #[values(2, 3, 4)] seats: usize,
) {
    use crate::common::seeds::fuzz_games;
    use mtg_testkit::{entered_fuzz_setup, play_game};

    let mut counters = 0usize;
    for seed in 0..fuzz_games(30) {
        let setup = entered_fuzz_setup(seats, seed).unwrap();
        let outcome = play_game(setup.clone(), seed)
            .unwrap_or_else(|failure| panic!("seed {seed}: {failure}"));
        let replayed = replay(&setup, &outcome.events).unwrap();
        assert_eq!(replayed.state_hash(), outcome.final_hash, "seed {seed}");
        counters += outcome
            .events
            .iter()
            .filter(|event| {
                matches!(event, Event::CountersChanged(change)
                    if change.delta > 0 && change.counter == CounterKind::PlusOnePlusOne)
            })
            .count();
    }
    assert!(counters > 0, "a card gave counters to what entered");
}

/// Every event the driver has seen, in order.
fn events_of(driver: &Driver) -> Vec<Event> {
    driver.engine().events().to_vec()
}
