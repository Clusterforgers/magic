//! Goal: the cards that alone held back two precon cards each play as printed at tables of two, three and four seats: the Equipment whose creature halves the life of the player it hurts (Quietus Spike, Scytheclaw).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::pid;
use mtg_engine::decision::DecisionKind;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{free_creature, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, attached_to, ready};
use rstest::rstest;

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn assert_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// Plays on to the second main phase of the turn, blocking as `blocks` says.
fn through_combat(driver: &mut Driver, blocks: &[(&str, &str)]) -> Result<(), ScriptError> {
    for _ in 0..80 {
        if driver.state().turn().step() == Step::Main2
            && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        {
            return Ok(());
        }
        match &driver.decision()?.kind {
            DecisionKind::DeclareBlockers(_) => {
                driver.block(blocks)?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("combat".into()))
}

#[rstest]
fn the_equipped_creature_halves_the_life_of_the_player_it_hurts(
    #[values("Quietus Spike", "Scytheclaw")] equipment: &str,
    #[values(2, 3, 4)] seats: usize,
    #[values(20, 9, 6)] start: i32,
    #[values(false, true)] blocked: bool,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .battlefield(
            0,
            [
                ready(free_creature("Knight", 2, 2)),
                ready(free_creature("Squire", 1, 1)),
                attached_to(ready(card(equipment).unwrap()), 0, 0),
            ],
        )
        .battlefield(1, [ready(free_creature("Wall", 0, 9))])
        .life(1, start);
    for seat in 2..seats {
        scenario = scenario.battlefield(seat, [ready(swamp())]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Knight", 1), ("Squire", 1)])?;
    let blocks: &[(&str, &str)] = if blocked { &[("Wall", "Knight")] } else { &[] };
    through_combat(&mut driver, blocks)?;
    // The Germ of Scytheclaw is a creature of its own; the Knight carries the +1/+1 and the Spike's deathtouch.
    let power = if equipment == "Scytheclaw" { 3 } else { 2 };
    let expected = if blocked {
        // Only the Squire hurts the player, and it carries nothing.
        start - 1
    } else {
        let hurt = start - power - 1;
        hurt - (hurt + 1) / 2
    };
    assert_eq!(life(&driver, 1), expected);
    for seat in (0..seats).filter(|seat| *seat != 1) {
        assert_eq!(life(&driver, seat), 20, "seat {seat}");
    }
    assert_replays(&driver);
    Ok(())
}
