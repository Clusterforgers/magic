//! Goal: an Aura attached as it transforms (`ReplaceBy::R6603AttachToPlayer`, CR 303.4f, 701.28) at 2 to 4 seats: Curse
//! of Leeches turned into Leeching Lurker as it becomes night is a creature attached to nobody, and turned back as it becomes day it asks its controller for
//! a player and enchants the one chosen, staying on the battlefield; each log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::DayNight;
use mtg_engine::choose::{ChoiceOption, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::corpus::card;
use mtg_testkit::scenario::enchanting;
use mtg_testkit::{Driver, Scenario, ScriptError, day_maker, ready};
use rstest::rstest;

use crate::common::copying::replays;
use crate::common::pid;

const CURSE: &str = "Curse of Leeches // Leeching Lurker";

/// Seat 0 has Curse of Leeches enchanting seat 1, and instants that make it night and day.
fn table(seats: usize) -> Scenario {
    Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [enchanting(ready(card(CURSE).unwrap()), 1)])
        .hand(
            0,
            [
                day_maker("Morning", DayNight::Day),
                day_maker("Dusk", DayNight::Night),
                day_maker("Dawn", DayNight::Day),
            ],
        )
}

/// Casts `spell` and plays on, choosing the player in seat `seat` when asked.
fn flip(driver: &mut Driver, spell: &str, seat: usize) -> Result<(), ScriptError> {
    driver.cast(spell)?;
    for _ in 0..100 {
        if driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
            && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        {
            return Ok(());
        }
        match driver.decision()?.kind.clone() {
            DecisionKind::ChooseOption(options) => {
                let at = options
                    .options
                    .iter()
                    .position(|option| matches!(option, ChoiceOption::Player(p) if *p == pid(seat)))
                    .unwrap();
                driver.answer(Answer::ChooseOption(OptionChoice(at)))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

#[rstest]
fn curse_of_leeches_is_attached_to_a_chosen_player_as_it_transforms_back(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let setup = table(seats).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats))?;
    let id = driver.id_of("Curse of Leeches")?;
    flip(&mut driver, "Morning", 0)?;
    flip(&mut driver, "Dusk", 0)?;
    let lurker = driver.state().object(id).unwrap();
    assert_eq!(lurker.face(), Some(1));
    assert_eq!(lurker.permanent().unwrap().attached_player(), None);
    let chosen = seats - 1;
    flip(&mut driver, "Dawn", chosen)?;
    let curse = driver.state().object(id).unwrap();
    assert_eq!(curse.zone(), Zone::Battlefield);
    assert_ne!(curse.face(), Some(1));
    assert_eq!(
        curse.permanent().unwrap().attached_player(),
        Some(pid(chosen))
    );
    replays(&driver, &setup);
    Ok(())
}
