//! Goal: Ice Cauldron at 2 to 4 seats: its first ability exiles a nonland card from the hand, lets its owner cast it
//! from exile and notes the type and amount of the mana its {X} spent; its second adds that mana, which pays only for
//! the last card exiled with the Cauldron (CR 106.6, 607.2a): the exiled card is cast with it and the lands stay
//! untapped, while a spell from the hand leaves the mana floating.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_core::mana::ManaType;
use mtg_engine::choose::{CardsChoice, NumberChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, forest, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0 controls Ice Cauldron and two Islands and holds two blue 2/2s for {1}{U}.
fn cauldron(seats: usize) -> Result<Driver, ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(
            0,
            vec![
                creature("Kept Drake", 1, &[Color::Blue], 2, 2),
                creature("Hand Drake", 1, &[Color::Blue], 2, 2),
            ],
        )
        .battlefield(
            0,
            [
                ready(card("Ice Cauldron").unwrap()),
                ready(island()),
                ready(island()),
            ],
        );
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 8]);
    }
    Driver::from_scenario(scenario)
}

/// Blue mana floating in seat 0's pool that pays only for the exiled card.
fn blue(driver: &Driver) -> usize {
    let pool = driver.state().player(PlayerId(0)).unwrap().mana();
    assert_eq!(
        pool.get(ManaType::Blue),
        0,
        "no blue mana pays for anything"
    );
    pool.restricted()
        .iter()
        .filter(|mana| mana.mana == ManaType::Blue)
        .count()
}

/// Untapped Islands of seat 0.
fn untapped_islands(driver: &Driver) -> usize {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter(|object| object.card().name == "Island")
        .filter(|object| object.permanent().is_some_and(|p| !p.tapped()))
        .count()
}

/// The card named `name` in `zone`.
fn named(driver: &Driver, zone: Zone, name: &str) -> mtg_core::ids::ObjectId {
    *driver
        .state()
        .zone(zone)
        .iter()
        .find(|id| driver.state().object(**id).unwrap().card().name == name)
        .expect("the card is there")
}

/// Activates the first ability for X = 2 with both Islands, exiles Kept Drake, and waits for seat 0's next main phase.
fn charge(driver: &mut Driver, seats: usize) -> Result<(), ScriptError> {
    driver.begin_ability("Ice Cauldron", 0)?;
    for _ in 0..30 {
        let kind = driver.decision()?.kind.clone();
        match kind {
            DecisionKind::ChooseNumber(_) => {
                driver.answer(Answer::ChooseNumber(NumberChoice::Number(2)))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(true)))?;
            }
            DecisionKind::ChooseCards(_) => {
                let drake = named(driver, Zone::Hand(PlayerId(0)), "Kept Drake");
                driver.answer(Answer::ChooseCards(CardsChoice(vec![drake])))?;
            }
            _ if driver.state().stack().is_empty() => break,
            _ => {
                driver.pass()?;
            }
        }
    }
    named(driver, Zone::Exile, "Kept Drake");
    driver.idle_until(u32::try_from(seats).unwrap() + 1, Step::Main1)?;
    assert_eq!(driver.offered_abilities("Ice Cauldron")?, vec![1]);
    driver.begin_ability("Ice Cauldron", 1)?;
    assert_eq!(blue(driver), 2, "the noted type and amount");
    Ok(())
}

#[rstest]
fn r114_ice_cauldron_mana_casts_the_exiled_card(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = cauldron(seats)?;
    charge(&mut driver, seats)?;
    driver.begin_cast("Kept Drake")?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    assert_eq!(blue(&driver), 0, "the Cauldron's mana paid");
    assert_eq!(untapped_islands(&driver), 2, "no land was tapped");
    Ok(())
}

#[rstest]
fn r114_ice_cauldron_mana_does_not_pay_another_spell(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = cauldron(seats)?;
    charge(&mut driver, seats)?;
    driver.begin_cast("Hand Drake")?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    assert_eq!(blue(&driver), 2, "the Cauldron's mana stays floating");
    assert_eq!(untapped_islands(&driver), 0, "the Islands paid");
    Ok(())
}
