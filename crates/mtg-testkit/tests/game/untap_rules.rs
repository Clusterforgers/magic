//! Goal: a permanent that "doesn't untap" stays tapped through its controller's untap step while every other permanent untaps, at tables of two to four seats: the card itself (Basalt Monolith, Goblin Sharpshooter), an Aura's creature (Fall from Favor) until its controller becomes the monarch, the monarch designation kept by the Aura's controller, and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::decision::{Answer, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{free_creature, island, monarcher};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, attached_to, ready, tapped};
use rstest::rstest;

/// Every seat has a library of Islands so that no draw loses the game.
fn libraries(mut scenario: Scenario, seats: usize) -> Scenario {
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..20).map(|_| island()));
    }
    scenario
}

fn is_tapped(driver: &Driver, id: ObjectId) -> bool {
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .is_some_and(mtg_engine::object::PermanentState::tapped)
}

/// Lets whatever is on the stack resolve.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// The card taps itself and stays tapped through its controller's next untap step; a creature tapped beside it untaps.
#[rstest]
#[case::monolith_two_seats("Basalt Monolith", 2)]
#[case::monolith_three_seats("Basalt Monolith", 3)]
#[case::monolith_four_seats("Basalt Monolith", 4)]
#[case::sharpshooter_two_seats("Goblin Sharpshooter", 2)]
#[case::sharpshooter_three_seats("Goblin Sharpshooter", 3)]
#[case::sharpshooter_four_seats("Goblin Sharpshooter", 4)]
fn the_card_stays_tapped_in_its_controllers_untap_step(
    #[case] name: &str,
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let game = || {
        libraries(
            Scenario::new(seats).battlefield(
                0,
                vec![
                    tapped(card(name).unwrap()),
                    tapped(free_creature("Bear", 2, 2)),
                ],
            ),
            seats,
        )
    };
    let setup = game().setup().unwrap();
    let mut driver = Driver::from_scenario(game())?;
    let (held, free) = (driver.id_of(name)?, driver.id_of("Bear")?);
    // Seat 0's next turn comes after one turn of every seat.
    driver.idle_until(u32::try_from(seats).unwrap() + 1, Step::Main1)?;
    assert!(is_tapped(&driver, held), "{name} did not untap");
    assert!(!is_tapped(&driver, free), "the Bear untapped");
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

/// Well Rested gives its creature a trigger on becoming untapped: two counters, 2 life and a card, once in the turn.
#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn well_rested_pays_when_the_creature_untaps(#[case] seats: usize) -> Result<(), ScriptError> {
    let game = || {
        libraries(
            Scenario::new(seats).battlefield(
                0,
                vec![
                    tapped(free_creature("Bear", 2, 2)),
                    attached_to(ready(card("Well Rested").unwrap()), 0, 0),
                ],
            ),
            seats,
        )
    };
    let setup = game().setup().unwrap();
    let mut driver = Driver::from_scenario(game())?;
    let bear = driver.id_of("Bear")?;
    let life = |driver: &Driver| {
        driver
            .state()
            .player(PlayerId::from_index(0))
            .unwrap()
            .life()
    };
    let before = life(&driver);
    driver.idle_until(u32::try_from(seats).unwrap() + 1, Step::Main1)?;
    settle(&mut driver)?;
    let counters = driver
        .state()
        .object(bear)
        .and_then(|object| object.permanent())
        .map(|permanent| permanent.counters_of(&CounterKind::PlusOnePlusOne));
    assert_eq!(counters, Some(2));
    assert_eq!(life(&driver), before + 2);
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

/// Fall from Favor taps the creature and makes its controller of the Aura the monarch; the creature of another seat stays
/// tapped while that seat is not the monarch and untaps in the untap step after it becomes the monarch.
#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn an_enchanted_creature_untaps_once_its_controller_is_the_monarch(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let game = || {
        libraries(
            Scenario::new(seats)
                .hand(0, [card("Fall from Favor").unwrap()])
                .battlefield(0, (0..3).map(|_| ready(island())).collect::<Vec<_>>())
                .hand(1, [monarcher("Crown")])
                .battlefield(1, vec![ready(island()), ready(free_creature("Bear", 2, 2))]),
            seats,
        )
    };
    let setup = game().setup().unwrap();
    let mut driver = Driver::from_scenario(game())?;
    let bear = driver.id_of("Bear")?;
    let target = driver.target_named("Bear")?;
    driver.begin_cast("Fall from Favor")?;
    driver.choose_targets(&[target])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver)?;
    assert!(is_tapped(&driver, bear), "the enters trigger taps the Bear");
    assert_eq!(
        driver.state().monarch().map(mtg_core::ids::PlayerId::index),
        Some(0)
    );
    // Seat 1's untap step: seat 0 is the monarch, so the Bear stays tapped.
    driver.idle_until(2, Step::Main1)?;
    assert!(
        is_tapped(&driver, bear),
        "the Bear untapped for a foe monarch"
    );
    // Seat 1 becomes the monarch; at its next untap step the Bear untaps.
    driver.cast("Crown")?;
    settle(&mut driver)?;
    assert_eq!(
        driver.state().monarch().map(mtg_core::ids::PlayerId::index),
        Some(1)
    );
    driver.idle_until(u32::try_from(seats).unwrap() + 2, Step::Main1)?;
    assert!(
        !is_tapped(&driver, bear),
        "the Bear stayed tapped for its own monarch"
    );
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
