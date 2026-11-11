//! Goal: a shield over the next draw ("the next time you would draw a card this turn, ... instead") or the next loss of the game replaces exactly one such event of its controller and then ends, and a cost that exiles the top card of a library lets the effects read that card (`Paid`), at tables of two to four seats, and the log replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_core::mana::ManaSymbol;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{GameSetup, replay};
use mtg_testkit::cards::{creature_costing, free_creature, mountain, swamp};
use mtg_testkit::{Driver, Scenario, ScriptError, corpus, ready};
use rstest::rstest;

use crate::common::pid;

fn replays(driver: &Driver, setup: &GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

/// Seat 0 controls `words`, a Mountain and Howling Mine at the upkeep of its second turn; every library has ten cards and
/// every other seat holds one card.
fn words_table(seats: usize, words: &str) -> Scenario {
    let library = || (0..10).map(|i| free_creature(&format!("Pad {i}"), 1, 1));
    (0..seats).fold(
        Scenario::new(seats).at(2, 0, Step::Upkeep).battlefield(
            0,
            vec![
                ready(corpus::card(words).unwrap()),
                ready(mountain()),
                ready(corpus::card("Howling Mine").unwrap()),
            ],
        ),
        |scenario, seat| {
            let scenario = scenario.library(seat, library());
            if seat == 0 {
                scenario
            } else {
                scenario.hand(seat, vec![free_creature(&format!("Held {seat}"), 1, 1)])
            }
        },
    )
}

#[rstest]
#[case::worship("Words of Worship", 5, 0, 0)]
#[case::wilding("Words of Wilding", 0, 1, 0)]
#[case::waste("Words of Waste", 0, 0, 1)]
fn the_next_draw_is_replaced_once_and_the_later_draw_happens(
    #[case] words: &str,
    #[case] life_gained: i32,
    #[case] bears: usize,
    #[case] discarded: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let setup = words_table(seats, words).setup().unwrap();
    let mut driver = Driver::from_scenario(words_table(seats, words))?;
    let life = driver.state().player(pid(0)).unwrap().life();
    driver.begin_activation(words)?;
    driver.pay_tapping(&["Mountain"], 0)?;
    driver.pass_until(Step::Main1)?;
    let state = driver.state();
    // The draw step's draw is replaced; Howling Mine's draw after it is not.
    assert_eq!(state.zone(Zone::Hand(pid(0))).len(), 1, "{words}");
    assert_eq!(state.zone(Zone::Library(pid(0))).len(), 9, "{words}");
    assert_eq!(state.player(pid(0)).unwrap().life(), life + life_gained);
    let made = state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| state.object(**id).is_some_and(|o| o.card().name == "Bear"))
        .count();
    assert_eq!(made, bears);
    for seat in 1..seats {
        assert_eq!(state.zone(Zone::Graveyard(pid(seat))).len(), discarded);
        assert_eq!(state.zone(Zone::Hand(pid(seat))).len(), 1 - discarded);
    }
    replays(&driver, &setup);
    Ok(())
}

/// Passes priority until the stack is empty.
fn resolve_stack(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        if driver.state().zone(Zone::Stack).is_empty() {
            return Ok(());
        }
        driver.pass()?;
    }
    Ok(())
}

/// Seat 0 controls Phyrexian Devourer in its first main phase; the top card of its library costs `generic` and the next
/// one costs one.
fn devourer_table(seats: usize, generic: usize) -> Scenario {
    let top = creature_costing("Top", &vec![ManaSymbol::Colorless; generic], 1, 1);
    let next = creature_costing("Next", &[ManaSymbol::Colorless], 1, 1);
    (1..seats).fold(
        Scenario::new(seats)
            .at(2, 0, Step::Main1)
            .battlefield(0, vec![ready(corpus::card("Phyrexian Devourer").unwrap())])
            .library(0, vec![top, next]),
        |scenario, seat| {
            scenario.library(
                seat,
                (0..3).map(|i| free_creature(&format!("Pad {i}"), 1, 1)),
            )
        },
    )
}

#[rstest]
#[case::three_counters_and_it_stays(3, true)]
#[case::six_counters_make_it_seven_and_it_goes(6, false)]
fn the_exiled_cards_mana_value_is_the_number_of_counters(
    #[case] generic: usize,
    #[case] stays: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let setup = devourer_table(seats, generic).setup().unwrap();
    let mut driver = Driver::from_scenario(devourer_table(seats, generic))?;
    let devourer = driver.id_of("Phyrexian Devourer")?;
    driver.begin_activation("Phyrexian Devourer")?;
    if matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
        driver.pay_tapping(&[], 0)?;
    }
    resolve_stack(&mut driver)?;
    let state = driver.state();
    assert!(
        state
            .zone(Zone::Exile)
            .iter()
            .any(|id| state.object(*id).is_some_and(|o| o.card().name == "Top")),
        "the top card paid the cost"
    );
    if stays {
        let counters = state
            .object(devourer)
            .and_then(|object| object.permanent())
            .map_or(0, |permanent| {
                permanent.counters_of(&CounterKind::PlusOnePlusOne)
            });
        assert_eq!(counters, u32::try_from(generic).unwrap());
    } else {
        // Its power became 7: the state trigger sacrificed it.
        assert!(!driver.state().zone(Zone::Battlefield).contains(&devourer));
    }
    replays(&driver, &setup);
    Ok(())
}

/// Seat 0 is at 1 life in its first main phase with Stunning Reversal and two Lightning Bolts in hand, four Swamps and two
/// Mountains; every library has ten cards.
fn reversal_table(seats: usize) -> Scenario {
    let lands = vec![
        ready(swamp()),
        ready(swamp()),
        ready(swamp()),
        ready(swamp()),
        ready(mountain()),
        ready(mountain()),
    ];
    (0..seats).fold(
        Scenario::new(seats)
            .at(2, 0, Step::Main1)
            .life(0, 1)
            .battlefield(0, lands)
            .hand(
                0,
                vec![
                    corpus::card("Stunning Reversal").unwrap(),
                    corpus::card("Lightning Bolt").unwrap(),
                    corpus::card("Lightning Bolt").unwrap(),
                ],
            ),
        |scenario, seat| {
            scenario.library(
                seat,
                (0..10).map(|i| free_creature(&format!("Pad {i}"), 1, 1)),
            )
        },
    )
}

#[rstest]
fn the_next_loss_of_the_game_is_replaced_once(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let setup = reversal_table(seats).setup().unwrap();
    let mut driver = Driver::from_scenario(reversal_table(seats))?;
    driver.cast_tapping("Stunning Reversal", &["Swamp", "Swamp", "Swamp", "Swamp"])?;
    resolve_stack(&mut driver)?;
    assert!(
        driver.state().zone(Zone::Exile).iter().any(|id| driver
            .state()
            .object(*id)
            .is_some_and(|o| o.card().name == "Stunning Reversal")),
        "the spell exiles itself"
    );
    let me = driver.target_player(0);
    driver.cast_at("Lightning Bolt", &[me])?;
    resolve_stack(&mut driver)?;
    let state = driver.state();
    let player = state.player(pid(0)).unwrap();
    assert!(player.is_in_game(), "the first loss is replaced");
    assert_eq!(player.life(), 1);
    assert_eq!(
        state.zone(Zone::Hand(pid(0))).len(),
        1 + 7,
        "a Bolt and seven cards"
    );
    replays(&driver, &setup);
    let me = driver.target_player(0);
    driver.cast_at("Lightning Bolt", &[me])?;
    // The game may be over for two seats: a pass then has no one to ask.
    let _ = resolve_stack(&mut driver);
    assert!(
        !driver.state().player(pid(0)).unwrap().is_in_game(),
        "the shield is used up"
    );
    Ok(())
}
