//! Goal: the putter of counters an effect names (`PutCountersOf.by`, `Event::R115CountersPutBy`, CR 122.6) and the player who caused a counters event (`Ref::R115Causer`) play Bold Plagiarist at tables of two to four seats: an opponent who puts counters on a creature they control makes the trigger put the same number and kind on Bold Plagiarist, put by that opponent, so its controller's "if you would put counters" replacement (Lae'zel) leaves them alone; counters its controller puts, or an opponent puts on another player's creature, trigger nothing; and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_core::color::Color;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, forest};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// Seat 0 has Bold Plagiarist (and Lae'zel when `laezel`); `caster` holds Dragonscale Boon and forests, in the first
/// main phase of their turn; `owner` has the creature it targets.
fn r115_table(seats: usize, caster: usize, owner: usize, laezel: bool) -> Scenario {
    let mut scenario = Scenario::new(seats).at(1, caster, mtg_engine::turn::Step::Main1);
    for seat in 0..seats {
        let mut board = Vec::new();
        if seat == 0 {
            board.push(ready(card("Bold Plagiarist").unwrap()));
            if laezel {
                board.push(ready(card("Lae'zel, Vlaakith's Champion").unwrap()));
            }
        }
        if seat == caster {
            board.extend((0..4).map(|_| ready(forest())));
        }
        if seat == owner {
            board.push(ready(creature("Target Bear", 1, &[Color::Green], 2, 2)));
        }
        scenario = scenario.battlefield(seat, board);
    }
    scenario.hand(caster, vec![card("Dragonscale Boon").unwrap()])
}

/// The +1/+1 counters on the named permanent.
fn r115_counters(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::PlusOnePlusOne)
}

#[rstest]
#[case::an_opponent_on_their_own(1, 1, false, 2, 2)]
#[case::put_by_the_opponent_not_by_you(1, 1, true, 2, 2)]
#[case::your_own_creature(0, 0, true, 3, 0)]
#[case::another_players_creature(1, 2, false, 2, 0)]
fn r115_the_opponent_puts_the_same_counters(
    #[values(2, 3, 4)] seats: usize,
    #[case] caster: usize,
    #[case] owner: usize,
    #[case] laezel: bool,
    #[case] on_target: u32,
    #[case] copied: u32,
) -> Result<(), ScriptError> {
    if owner >= seats {
        return Ok(());
    }
    let setup = r115_table(seats, caster, owner, laezel).setup().unwrap();
    let mut driver = Driver::from_scenario(r115_table(seats, caster, owner, laezel))?;
    driver.begin_cast("Dragonscale Boon")?;
    let bear = driver.target_named("Target Bear")?;
    for _ in 0..100 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::ChooseTargets(_) => {
                driver.choose_targets(&[bear])?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::Priority(_)
                if driver.state().zone(Zone::Stack).is_empty()
                    && driver.state().pending_triggers().is_empty() =>
            {
                break;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    assert_eq!(r115_counters(&driver, "Target Bear"), on_target);
    assert_eq!(r115_counters(&driver, "Bold Plagiarist"), copied);
    let put_by = driver
        .engine()
        .events()
        .iter()
        .any(|event| matches!(event, mtg_engine::Event::R115CountersPutBy { player } if *player == pid(caster)));
    assert_eq!(put_by, copied > 0, "the opponent puts the copied counters");
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
