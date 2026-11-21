//! Goal: a copy effect of a static ability (`Modification::R707CopyOf`, CR 707.2, 613.2a) at 2 to 4 seats: Volrath's
//! Shapeshifter is a copy of the top card of its owner's graveyard while that card is a creature card, with "{2}: Discard
//! a card." besides, is itself again once a noncreature card lands on top, and follows the top card as it changes;
//! each log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_engine::characteristics::characteristics;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_testkit::cards::{cost, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::replays;

const VOLRATH: &str = "Volrath's Shapeshifter";

/// A vanilla creature card.
fn creature(name: &str, power: i32) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_mana_cost(cost(0, &[]))
            .with_power_toughness(power, power),
    )
}

/// Lightning Bolt with no mana cost.
fn bolt() -> Arc<CardDefinition> {
    let mut card = (*card("Lightning Bolt").unwrap()).clone();
    card.mana_cost = Some(cost(0, &[]));
    Arc::new(card)
}

/// Seat 0 has the Shapeshifter, `graveyard` in its graveyard (the last card on top) and a free Lightning Bolt.
fn table(seats: usize, graveyard: Vec<Arc<CardDefinition>>) -> Scenario {
    Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(card(VOLRATH).unwrap())])
        .graveyard(0, graveyard)
        .hand(0, [bolt()])
}

/// The name, power and number of activated abilities the permanent shows.
fn shown(driver: &Driver, id: ObjectId) -> (String, i32, usize) {
    let state = driver.state();
    let object = state.object(id).unwrap();
    let chars = characteristics(state, id).unwrap();
    let activated = mtg_engine::ability_reader::abilities_of(state, id)
        .iter()
        .filter(|(_, body)| matches!(**body, Ability::Activated { .. }))
        .count();
    (
        chars.name_for(&object.card().name),
        chars.power.unwrap(),
        activated,
    )
}

/// The Shapeshifter's id, whatever it is a copy of.
fn shapeshifter(driver: &Driver) -> ObjectId {
    *driver
        .state()
        .zone(mtg_engine::zones::Zone::Battlefield)
        .first()
        .unwrap()
}

#[rstest]
#[case::a_creature_on_top(vec![creature("Elf", 1), creature("Giant", 4)], ("Giant", 4, 1))]
#[case::a_creature_under_a_land(vec![creature("Giant", 4), plains()], (VOLRATH, 0, 1))]
#[case::an_empty_graveyard(Vec::new(), (VOLRATH, 0, 1))]
fn volraths_shapeshifter_has_the_text_of_the_top_creature_card(
    #[values(2, 3, 4)] seats: usize,
    #[case] graveyard: Vec<Arc<CardDefinition>>,
    #[case] expected: (&str, i32, usize),
) -> Result<(), ScriptError> {
    let setup = table(seats, graveyard.clone()).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, graveyard))?;
    driver.answer_passively()?;
    let id = shapeshifter(&driver);
    let (name, power, activated) = expected;
    assert_eq!(shown(&driver, id), (name.to_owned(), power, activated));
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn the_copy_ends_when_a_noncreature_card_lands_on_top(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let graveyard = vec![creature("Giant", 4)];
    let setup = table(seats, graveyard.clone()).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, graveyard))?;
    let id = shapeshifter(&driver);
    assert_eq!(shown(&driver, id), ("Giant".to_owned(), 4, 1));
    driver.begin_cast("Lightning Bolt")?;
    let foe = driver.target_player(1);
    for _ in 0..100 {
        if driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
            && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        {
            break;
        }
        match driver.decision()?.kind {
            DecisionKind::ChooseTargets(_) => {
                driver.choose_targets(&[foe])?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    assert_eq!(shown(&driver, id), (VOLRATH.to_owned(), 0, 1));
    replays(&driver, &setup);
    Ok(())
}
