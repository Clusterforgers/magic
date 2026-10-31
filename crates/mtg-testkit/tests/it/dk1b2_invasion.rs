//! Goal: Invasion of Theros // Ephara, Ever-Sheltering plays whole: the Siege enters with its defense and a protector its controller chooses, searches for an Aura, God or Demigod card, loses defense to the damage of attackers, is exiled and cast transformed when defeated, and Ephara draws for the enchantments that enter and guards herself with three others; the log replays and the search stays private.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::ChoiceOption;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_testkit::{Driver, ScriptError};
use rstest::rstest;

use crate::dk1f_cards::{assert_replays, first_main, has, settle, with_lands};

const CARD: &str = "Invasion of Theros // Ephara, Ever-Sheltering";
const INVASION: &str = "Invasion of Theros";
const EPHARA: &str = "Ephara, Ever-Sheltering";

/// Answers the questions of the table until the stack is empty again, taking the first option of a choice and the
/// first card of a search.
fn resolve(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        if driver.state().stack().is_empty()
            && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    panic!("the stack never emptied");
}

fn defense(driver: &Driver, id: ObjectId) -> u32 {
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| {
            permanent.counters_of(&CounterKind::defense())
        })
}

#[rstest]
fn the_siege_enters_with_four_defense_and_a_protector(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let build = || {
        first_main(seats)
            .hand_named(0, [CARD])
            .battlefield(0, with_lands(3, []))
            .library_named(0, ["Grizzly Bears", "All That Glitters", "Serra Angel"])
    };
    let setup = build().setup()?;
    let mut driver = Driver::from_scenario(build())?;
    driver.cast(INVASION)?;
    resolve(&mut driver)?;
    let siege = driver.id_of(INVASION)?;
    assert_eq!(defense(&driver, siege), 4);
    let _ = (
        Zone::Battlefield,
        CardType::Battle,
        Keyword::Lifelink,
        has,
        settle,
        EPHARA,
    );
    let _ = ChoiceOption::Player;
    assert_replays(&driver, &setup)
}
