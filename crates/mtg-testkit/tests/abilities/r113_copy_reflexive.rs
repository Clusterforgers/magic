//! Goal: a reflexive trigger after entering as a copy (`ReplaceBy::AsCopy::r707_when_you_do`, CR 603.12, 707.9) at 2 to
//! 4 seats: Wall of Stolen Identity entering as a copy is a Wall with defender too, and its "when you do" taps the
//! copied creature, which stays tapped in its controller's untap step while the Wall's controller controls the Wall;
//! declining the copy triggers nothing (the 0/0 Wall dies); each log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_core::card::{CardType, Subtype};
use mtg_core::ids::ObjectId;
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::keywords::keywords_of;
use mtg_engine::turn::Step;
use mtg_testkit::cards::cost;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::replays;

const WALL: &str = "Wall of Stolen Identity";

/// The corpus card with no mana cost.
fn free(name: &str) -> Arc<CardDefinition> {
    let mut card = (*card(name).unwrap()).clone();
    card.mana_cost = Some(cost(0, &[]));
    Arc::new(card)
}

/// A vanilla 3/3 Bear.
fn bear() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Bear")
            .with_type(CardType::Creature)
            .with_subtype("Bear")
            .with_mana_cost(cost(0, &[]))
            .with_power_toughness(3, 3),
    )
}

/// Seat 0 has the Wall in hand; the last seat has an untapped Bear; each library holds Plains.
fn table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [free(WALL)])
        .battlefield(seats - 1, [ready(bear())]);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..4).map(|_| mtg_testkit::cards::plains()));
    }
    scenario
}

/// Plays on until the stack is empty, picking `pick` for the copy.
fn settle(driver: &mut Driver, pick: Option<ObjectId>) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
            && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        {
            return Ok(());
        }
        match driver.decision()?.kind {
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::ChooseCards(_) => {
                driver.answer(Answer::ChooseCards(CardsChoice(pick.into_iter().collect())))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Whether the permanent is tapped.
fn tapped(driver: &Driver, id: ObjectId) -> bool {
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .tapped()
}

#[rstest]
#[case::copied(true)]
#[case::declined(false)]
fn wall_of_stolen_identity_taps_and_locks_what_it_copies(
    #[values(2, 3, 4)] seats: usize,
    #[case] copies: bool,
) -> Result<(), ScriptError> {
    let setup = table(seats).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats))?;
    let bear = driver.id_of("Bear")?;
    driver.begin_cast(WALL)?;
    settle(&mut driver, copies.then_some(bear))?;
    assert_eq!(
        tapped(&driver, bear),
        copies,
        "the copied creature is tapped"
    );
    if copies {
        let wall = driver
            .state()
            .zone(mtg_engine::zones::Zone::Battlefield)
            .iter()
            .copied()
            .find(|id| *id != bear)
            .unwrap();
        let chars = characteristics(driver.state(), wall).unwrap();
        let printed = &driver.state().object(wall).unwrap().card().subtypes;
        assert!(chars.has_subtype(printed, &Subtype("Wall".into())));
        assert!(chars.has_subtype(printed, &Subtype("Bear".into())));
        assert!(keywords_of(driver.state(), wall).contains(&&Keyword::Defender));
        // Its controller's untap step leaves it tapped.
        driver.idle_until(u32::try_from(seats).unwrap(), Step::Main1)?;
        assert!(tapped(&driver, bear), "it doesn't untap");
    }
    replays(&driver, &setup);
    Ok(())
}
