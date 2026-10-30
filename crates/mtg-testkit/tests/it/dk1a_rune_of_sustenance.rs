//! Goal: the compiled Rune of Sustenance gives lifelink to a creature it enchants and, on an Equipment, to the creature that Equipment is attached to (and to no other creature, and not while the Equipment is attached to nothing), the lifelink ends with the Aura, and it draws a card as it enters.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::pid;
use mtg_cards::keyword::Keyword;
use mtg_engine::keywords::has_keyword;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, island, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, attached_to, ready};
use rstest::rstest;

fn lifelink(driver: &Driver, name: &str) -> bool {
    has_keyword(
        driver.state(),
        driver.id_of(name).unwrap(),
        &Keyword::Lifelink,
    )
}

/// Seat 0 controls the Bear, the Wolf and the Bonesplitter, which is on the Bear, and the Rune, which is on `host`
/// (0 the Bear, 1 the Bonesplitter).
fn table(host: usize) -> Result<Driver, ScriptError> {
    let filler = || (0..12).map(|_| free_creature("Filler", 1, 1));
    Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::Main1)
            .library(0, filler())
            .library(1, filler())
            .battlefield(
                0,
                [
                    ready(free_creature("Bear", 2, 2)),
                    attached_to(ready(card("Bonesplitter").unwrap()), 0, 0),
                    ready(free_creature("Wolf", 2, 2)),
                    attached_to(ready(card("Rune of Sustenance").unwrap()), 0, host),
                ],
            ),
    )
}

#[rstest]
#[case::on_a_creature(0, true, false)]
#[case::on_an_equipment(1, true, false)]
fn lifelink_goes_to_the_creature_the_aura_or_its_equipment_is_on(
    #[case] host: usize,
    #[case] bear: bool,
    #[case] wolf: bool,
) -> Result<(), ScriptError> {
    let driver = table(host)?;
    assert_eq!(lifelink(&driver, "Bear"), bear);
    assert_eq!(lifelink(&driver, "Wolf"), wolf);
    Ok(())
}

#[test]
fn an_equipment_that_is_attached_to_nothing_gives_nothing() -> Result<(), ScriptError> {
    let filler = || (0..12).map(|_| free_creature("Filler", 1, 1));
    let driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::Main1)
            .library(0, filler())
            .library(1, filler())
            .battlefield(
                0,
                [
                    ready(free_creature("Bear", 2, 2)),
                    ready(card("Bonesplitter").unwrap()),
                    attached_to(ready(card("Rune of Sustenance").unwrap()), 0, 1),
                ],
            ),
    )?;
    assert!(!lifelink(&driver, "Bear"));
    Ok(())
}

#[test]
fn it_draws_a_card_as_it_enters() -> Result<(), ScriptError> {
    let filler = || (0..12).map(|_| free_creature("Filler", 1, 1));
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::Main1)
            .library(0, filler())
            .library(1, filler())
            .battlefield(
                0,
                [
                    ready(free_creature("Bear", 2, 2)),
                    ready(plains()),
                    ready(island()),
                ],
            )
            .hand(0, [card("Rune of Sustenance").unwrap()]),
    )?;
    let hand = |driver: &Driver| driver.state().zone(Zone::Hand(pid(0))).len();
    assert_eq!(hand(&driver), 1);
    let bear = driver.target_named("Bear")?;
    driver.cast_at("Rune of Sustenance", &[bear])?;
    for _ in 0..20 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        driver.answer_passively()?;
    }
    // The Rune left the hand and its trigger drew one card.
    assert_eq!(hand(&driver), 1);
    assert!(lifelink(&driver, "Bear"));
    Ok(())
}
