//! Goal: a Room is cast as one door and enters with that door unlocked and only its characteristics, the other door is unlocked as a special action at sorcery timing for its cost (refused while a spell is on the stack), each unlock triggers that door's "when you unlock this door" and the last one Eerie's "whenever you fully unlock a Room", a Room put onto the battlefield without being cast has both doors locked and only its types, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::ObjectId;
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_engine::zones::Zone;
use mtg_engine::{GameSetup, replay};
use mtg_testkit::cards::{island, plains};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::{choose_way, pay};
use crate::common::{name_of, pid};

const ROOM: &str = "Grand Entryway // Elegant Rotunda";

fn scenario(hand: &[&str], battlefield: &[&str]) -> Scenario {
    let mut lands: Vec<_> = (0..6).map(|_| ready(plains())).collect();
    lands.extend((0..3).map(|_| ready(island())));
    lands.extend(
        battlefield
            .iter()
            .map(|name| ready(mtg_testkit::corpus::card(name).unwrap())),
    );
    Scenario::new(2)
        .library(0, vec![plains(); 12])
        .library(1, vec![plains(); 12])
        .hand_named(0, hand.iter().copied())
        .battlefield(0, lands)
}

/// Answers every question passively until seat 0 holds priority on an empty stack.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        let decision = driver.decision()?;
        if decision.player == pid(0)
            && matches!(decision.kind, DecisionKind::Priority(_))
            && driver.state().zone(Zone::Stack).is_empty()
        {
            return Ok(());
        }
        if matches!(decision.kind, DecisionKind::Priority(_)) {
            driver.pass()?;
        } else {
            driver.answer_passively()?;
        }
    }
    panic!("the stack does not settle");
}

fn room(driver: &Driver) -> ObjectId {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| driver.state().dk1c5_doors(*id).is_some())
        .expect("a Room is on the battlefield")
}

fn unlockable(driver: &Driver) -> Vec<(String, String)> {
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        return Vec::new();
    };
    options
        .unlockable
        .iter()
        .map(|o| (o.name.clone(), o.cost.as_ref().unwrap().to_string()))
        .collect()
}

fn hand(driver: &Driver) -> usize {
    driver.state().zone(Zone::Hand(pid(0))).len()
}

fn glimmers(driver: &Driver) -> usize {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| name_of(driver.state(), **id) == "Glimmer")
        .count()
}

fn unlock(driver: &mut Driver, id: ObjectId, door: u8) -> Result<(), ScriptError> {
    driver.answer(Answer::Priority(PriorityAction::UnlockDoor(id, door)))?;
    pay(driver)?;
    settle(driver)
}

#[rstest]
#[case::entryway_first(0, "Grand Entryway", "Elegant Rotunda", "{2}{W}", 1)]
#[case::rotunda_first(1, "Elegant Rotunda", "Grand Entryway", "{1}{W}", 2)]
fn a_room_is_cast_as_one_door_and_the_other_is_unlocked_for_its_cost(
    #[case] door: u8,
    #[case] cast: &str,
    #[case] other: &str,
    #[case] cost: &str,
    #[case] draws: usize,
) -> Result<(), ScriptError> {
    let build = || scenario(&[ROOM], &["Entity Tracker"]);
    let setup: GameSetup = build().setup().unwrap();
    let mut driver = Driver::from_scenario(build())?;
    driver.begin_cast(ROOM)?;
    choose_way(&mut driver, cast)?;
    pay(&mut driver)?;
    settle(&mut driver)?;
    let id = room(&driver);
    let mut doors = [false, false];
    doors[usize::from(door)] = true;
    assert_eq!(driver.state().dk1c5_doors(id), Some(doors));
    assert_eq!(name_of(driver.state(), id), cast, "only the door cast");
    assert_eq!(glimmers(&driver), usize::from(door == 0), "the door's unlock trigger");
    assert_eq!(unlockable(&driver), [(other.to_owned(), cost.to_owned())]);
    let before = hand(&driver);
    unlock(&mut driver, id, 1 - door)?;
    assert_eq!(driver.state().dk1c5_doors(id), Some([true, true]));
    assert_eq!(name_of(driver.state(), id), ROOM, "both doors");
    assert_eq!(driver.state().object(id).unwrap().card().mana_value(), 5);
    assert_eq!(glimmers(&driver), 1, "Grand Entryway unlocked once");
    assert_eq!(hand(&driver) - before, draws, "Eerie: fully unlocked and each enchantment that entered");
    assert!(unlockable(&driver).is_empty());
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

#[test]
fn a_room_not_cast_has_both_doors_locked_and_only_its_types() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(scenario(&[], &[ROOM]))?;
    let id = room(&driver);
    assert_eq!(driver.state().dk1c5_doors(id), Some([false, false]));
    let card = driver.state().object(id).unwrap().card();
    assert_eq!((card.name.as_str(), card.mana_value()), ("", 0));
    assert!(card.rules.is_empty());
    assert_eq!(
        unlockable(&driver),
        [
            ("Grand Entryway".to_owned(), "{1}{W}".to_owned()),
            ("Elegant Rotunda".to_owned(), "{2}{W}".to_owned())
        ]
    );
    unlock(&mut driver, id, 0)?;
    assert_eq!(driver.state().dk1c5_doors(id), Some([true, false]));
    assert_eq!(glimmers(&driver), 1);
    Ok(())
}

#[test]
fn a_door_is_not_unlocked_while_a_spell_is_on_the_stack() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(scenario(&["Entity Tracker"], &[ROOM]))?;
    let id = room(&driver);
    driver.cast("Entity Tracker")?;
    if driver.decision()?.player == pid(0) && !driver.state().zone(Zone::Stack).is_empty() {
        assert!(unlockable(&driver).is_empty());
        assert!(
            driver
                .answer(Answer::Priority(PriorityAction::UnlockDoor(id, 0)))
                .is_err()
        );
    }
    Ok(())
}
