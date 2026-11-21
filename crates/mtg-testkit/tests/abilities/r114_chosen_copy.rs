//! Goal: an object chosen as a permanent enters (`ReplaceBy::R114ChooseObject`, CR 614.1c) and a static copy of it
//! (`Ref::R114ChosenObject`, CR 707.2, 613.2a) at 2 to 4 seats: Metamorphic Alteration makes the creature it enchants a
//! copy of the creature chosen as it entered, the copy stays once the chosen creature has died (the values are locked in
//! as it is chosen), and ends when the Aura leaves; each log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::{CardsChoice, Target};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_testkit::cards::{cost, creature};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::replays;

const ALTERATION: &str = "Metamorphic Alteration";

/// The corpus card with no mana cost.
fn free(name: &str) -> Arc<CardDefinition> {
    let mut card = (*card(name).unwrap()).clone();
    card.mana_cost = Some(cost(0, &[]));
    Arc::new(card)
}

/// A free instant: "Destroy target creature." or "Destroy target enchantment."
fn r114_destroy(name: &str, kind: &str) -> Arc<CardDefinition> {
    crate::common::spell(
        name,
        CardType::Instant,
        &format!(r#"[{{"what":{{"Object":[{{"Type":"{kind}"}}]}}}}]"#),
        r#"[{"Destroy":{"what":{"Target":0}}}]"#,
    )
}

/// Seat 0 has the Aura, Murder and Shatter in hand and a 2/2 Bear; the last seat has a 3/3 Ogre and a 5/5 Dragon.
fn table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(
            0,
            [
                free(ALTERATION),
                r114_destroy("Murder", "Creature"),
                r114_destroy("Shatter", "Enchantment"),
            ],
        )
        .battlefield(0, [ready(creature("Bear", 1, &[], 2, 2))])
        .battlefield(
            seats - 1,
            [
                ready(creature("Ogre", 2, &[], 3, 3)),
                ready(creature("Dragon", 5, &[], 5, 5)),
            ],
        );
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..4).map(|_| mtg_testkit::cards::plains()));
    }
    scenario
}

/// Plays on until the stack is empty, picking `pick` for the choice of a creature.
fn settle(driver: &mut Driver, pick: ObjectId) -> Result<(), ScriptError> {
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
                driver.answer(Answer::ChooseCards(CardsChoice(vec![pick])))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// The power and toughness of the permanent.
fn looks(driver: &Driver, id: ObjectId) -> (Option<i32>, Option<i32>) {
    let chars = characteristics(driver.state(), id).unwrap();
    (chars.power, chars.toughness)
}

#[rstest]
#[case::the_dragon("Dragon", (Some(5), Some(5)))]
#[case::the_ogre("Ogre", (Some(3), Some(3)))]
fn metamorphic_alteration_copies_the_chosen_creature_for_good(
    #[values(2, 3, 4)] seats: usize,
    #[case] chosen: &str,
    #[case] copied: (Option<i32>, Option<i32>),
) -> Result<(), ScriptError> {
    let setup = table(seats).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats))?;
    let bear = driver.id_of("Bear")?;
    let pick = driver.id_of(chosen)?;
    let aim = driver.target_named("Bear")?;
    driver.begin_cast(ALTERATION)?;
    driver.choose_targets(&[aim])?;
    settle(&mut driver, pick)?;
    assert_eq!(looks(&driver, bear), copied, "the Bear is a copy");
    // The chosen creature dies: the enchanted creature stays a copy of it.
    let target = Target::Object(driver.state().object(pick).unwrap().reference());
    driver.cast_at("Murder", &[target])?;
    settle(&mut driver, pick)?;
    assert_eq!(
        looks(&driver, bear),
        copied,
        "the copy outlives the chosen creature"
    );
    // The Aura leaves: the creature is itself again.
    let aura = driver.target_named(ALTERATION)?;
    driver.cast_at("Shatter", &[aura])?;
    settle(&mut driver, pick)?;
    assert_eq!(looks(&driver, bear), (Some(2), Some(2)));
    replays(&driver, &setup);
    Ok(())
}
