//! Goal: a commander that pays its tax as life (Liesa, Shroud of Dusk) costs only its mana cost from the command zone
//! and 2 life for each previous cast from there, at 2 to 4 seats after 0, 1 and 2 earlier casts; another commander
//! still pays its tax in mana.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::format::Commander;
use mtg_engine::manual::{ManualAction, ManualZone};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{plains, swamp};
use mtg_testkit::corpus;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::commander_casts::resolve;

const SEAT: PlayerId = PlayerId(0);
const LIESA: &str = "Liesa, Shroud of Dusk";

/// A Commander table where seat 0 has `commander` in its command zone and six Plains and nine Swamps.
fn r5500_table(seats: usize, commander: Arc<CardDefinition>) -> Scenario {
    let mut lands: Vec<_> = (0..6).map(|_| ready(plains())).collect();
    lands.extend((0..9).map(|_| ready(swamp())));
    Scenario::new(seats)
        .format(Arc::new(Commander))
        .commanders(0, [commander])
        .battlefield(0, lands)
}

/// Puts the commander `name` from the battlefield back into the command zone.
fn r5500_back_to_command(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    let id = driver.id_of(name)?;
    driver.manual(ManualAction::Move {
        object: id,
        to: ManualZone::Graveyard,
    })?;
    assert!(matches!(driver.decision()?.kind, DecisionKind::YesNo(_)));
    driver.answer(Answer::YesNo(YesNoChoice(true)))?;
    assert_eq!(
        driver.state().object(id).unwrap().zone(),
        Zone::Command(SEAT)
    );
    Ok(())
}

fn r5500_life(driver: &Driver) -> i32 {
    driver.state().player(SEAT).unwrap().life()
}

/// Each cast of Liesa taps only two Plains and three Swamps, its mana cost, and the cast after `earlier` casts pays
/// `2 * earlier` life.
#[rstest]
fn r5500_liesa_pays_her_tax_in_life(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] casts: u32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(r5500_table(seats, corpus::card(LIESA).unwrap()))?;
    for earlier in 0..casts {
        let before = r5500_life(&driver);
        driver.cast_tapping(LIESA, &["Plains", "Plains", "Swamp", "Swamp", "Swamp"])?;
        assert_eq!(
            r5500_life(&driver),
            before - 2 * i32::try_from(earlier).unwrap(),
            "cast {}",
            earlier + 1
        );
        resolve(&mut driver)?;
        assert_eq!(
            driver.state().object(driver.id_of(LIESA)?).unwrap().zone(),
            Zone::Battlefield
        );
        if earlier + 1 < casts {
            r5500_back_to_command(&mut driver, LIESA)?;
        }
    }
    assert_eq!(driver.state().commander_casts_by(SEAT), casts);
    Ok(())
}

/// A commander without the rule still pays {2} more for its second cast: five lands do not cast it and no life is paid.
#[rstest]
fn r5500_another_commander_pays_its_tax_in_mana(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let plain = Arc::new(
        CardDefinition::new("Plain Legend")
            .with_type(mtg_core::card::CardType::Creature)
            .with_supertype(mtg_core::card::Supertype::Legendary)
            .with_mana_cost(mtg_core::mana::ManaCost::parse("{2}{W}{W}{B}").unwrap())
            .with_power_toughness(3, 3),
    );
    let mut driver = Driver::from_scenario(r5500_table(seats, plain))?;
    driver.cast_tapping(
        "Plain Legend",
        &["Plains", "Plains", "Swamp", "Swamp", "Swamp"],
    )?;
    resolve(&mut driver)?;
    r5500_back_to_command(&mut driver, "Plain Legend")?;
    let before = r5500_life(&driver);
    let short = driver.cast_tapping(
        "Plain Legend",
        &["Plains", "Plains", "Swamp", "Swamp", "Swamp"],
    );
    assert!(short.is_err(), "the tax is two more mana");
    assert_eq!(r5500_life(&driver), before);
    Ok(())
}
