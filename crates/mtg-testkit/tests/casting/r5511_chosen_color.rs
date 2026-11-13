//! Goal: a replacement of the mana a land makes with mana of a color its controller chooses (Pulse of Llanowar, Harvest
//! Mage) offers each color for each land it covers and adds the color picked, in the same amount or as one mana, and
//! nothing before Harvest Mage's ability resolves; at 2 to 4 seats, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_core::mana::ManaType;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{forest, island, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn pool(driver: &Driver, seat: usize, mana: ManaType) -> u32 {
    driver
        .state()
        .player(PlayerId(u64::try_from(seat).unwrap()))
        .unwrap()
        .mana()
        .get(mana)
}

fn assert_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

#[rstest]
#[case::forest_for_blue("Forest", ManaType::Blue)]
#[case::forest_for_red("Forest", ManaType::Red)]
#[case::swamp_for_white("Swamp", ManaType::White)]
fn r5511_pulse_lets_a_basic_land_make_any_color(
    #[case] land: &str,
    #[case] color: ManaType,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(
                0,
                [
                    ready(card("Pulse of Llanowar").unwrap()),
                    ready(forest()),
                    ready(swamp()),
                ],
            )
            .battlefield(1, [ready(island())]),
    )?;
    driver.tap_for_mana_of(land, color)?;
    assert_eq!(pool(&driver, 0, color), 1);
    assert_replays(&driver);
    Ok(())
}

#[rstest]
fn r5511_harvest_mage_makes_one_mana_of_a_color_until_end_of_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .hand(0, vec![forest()])
            .battlefield(
                0,
                [
                    ready(card("Harvest Mage").unwrap()),
                    ready(forest()),
                    ready(forest()),
                ],
            ),
    )?;
    assert!(
        driver.tap_for_mana_of("Forest", ManaType::Black).is_err(),
        "a Forest makes green before the ability resolves"
    );
    driver.begin_activation("Harvest Mage")?;
    loop {
        match &driver.decision()?.kind {
            DecisionKind::ChooseCards(options) => {
                let pick = options.from.iter().take(1).copied().collect();
                driver.answer(Answer::ChooseCards(mtg_engine::choose::CardsChoice(pick)))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => break,
        }
    }
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    driver.tap_for_mana_of("Forest", ManaType::Black)?;
    assert_eq!(pool(&driver, 0, ManaType::Black), 1);
    assert_replays(&driver);
    Ok(())
}
