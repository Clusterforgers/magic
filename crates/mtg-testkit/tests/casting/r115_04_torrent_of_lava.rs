//! Goal: a static ability of a spell that works while the spell is on the stack (`Static.from: Stack`) grants an activated ability whose `Ref::Granter` is that spell, and plays Torrent of Lava at tables of two to four seats: while it waits on the stack each creature has "{T}: Prevent the next 1 damage that would be dealt to this creature by Torrent of Lava this turn", a creature whose player taps it takes 1 less of the X damage, the others take all of it, fliers none, the ability is gone once the spell has resolved, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::keyword::Keyword;
use mtg_core::color::Color;
use mtg_engine::choose::NumberChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, keyworded_creature, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::lands;
use crate::common::pid;

/// Seat 0 holds Torrent of Lava and a 2/2; the last seat has two 2/2s and a flier.
fn r115_table(seats: usize) -> Scenario {
    let mut mine: Vec<_> = lands(5, mountain);
    mine.push(ready(creature("Own Bear", 1, &[Color::Green], 2, 2)));
    let mut scenario = Scenario::new(seats)
        .hand(0, vec![card("Torrent of Lava").unwrap()])
        .battlefield(0, mine)
        .at(1, 0, mtg_engine::turn::Step::Main1);
    for seat in 1..seats {
        if seat == seats - 1 {
            scenario = scenario.battlefield(
                seat,
                [
                    ready(creature("Tapper Bear", 1, &[Color::Green], 2, 2)),
                    ready(creature("Idle Bear", 1, &[Color::Green], 2, 2)),
                    ready(keyworded_creature(
                        "Bird",
                        1,
                        &[Color::White],
                        1,
                        1,
                        &[Keyword::Flying],
                    )),
                ],
            );
        }
    }
    scenario
}

/// Where the named card is now.
fn r115_zone(driver: &Driver, name: &str) -> Option<Zone> {
    driver
        .state()
        .objects()
        .find(|object| object.card().name == name)
        .map(mtg_engine::object::GameObject::zone)
}

#[rstest]
fn r115_a_tapped_creature_takes_one_less(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] x: u32,
) -> Result<(), ScriptError> {
    let setup = r115_table(seats).setup().unwrap();
    let mut driver = Driver::from_scenario(r115_table(seats))?;
    assert!(
        driver.offered_abilities("Own Bear")?.is_empty(),
        "no ability before the spell"
    );
    driver.begin_cast("Torrent of Lava")?;
    driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    assert_eq!(driver.state().zone(Zone::Stack).len(), 1);
    assert_eq!(
        driver.offered_abilities("Own Bear")?.len(),
        1,
        "each creature has the ability"
    );
    let mut tapped = false;
    for _ in 0..100 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_) if driver.state().zone(Zone::Stack).is_empty() => break,
            DecisionKind::Priority(_) if decision.player == pid(seats - 1) && !tapped => {
                driver.begin_activation("Tapper Bear")?;
                tapped = true;
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    assert!(
        tapped,
        "the creature had the ability while the spell was on the stack"
    );
    let bear = driver.id_of("Tapper Bear");
    if x <= 2 {
        let bear = bear?;
        let object = driver.state().object(bear).unwrap();
        assert_eq!(object.zone(), Zone::Battlefield);
        assert_eq!(
            object.permanent().unwrap().damage(),
            x - 1,
            "1 of the damage is prevented"
        );
    } else {
        assert!(matches!(
            r115_zone(&driver, "Tapper Bear"),
            Some(Zone::Graveyard(_))
        ));
    }
    let idle_dies = x >= 2;
    assert_eq!(
        matches!(r115_zone(&driver, "Idle Bear"), Some(Zone::Graveyard(_))),
        idle_dies
    );
    assert_eq!(
        matches!(r115_zone(&driver, "Own Bear"), Some(Zone::Graveyard(_))),
        idle_dies
    );
    assert_eq!(
        r115_zone(&driver, "Bird"),
        Some(Zone::Battlefield),
        "fliers are spared"
    );
    if !idle_dies {
        assert!(
            driver.offered_abilities("Own Bear")?.is_empty(),
            "the ability ends with the spell"
        );
    }
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
