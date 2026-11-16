//! Goal: Desertion counters any spell; the card of a countered artifact or creature spell enters the battlefield under
//! Desertion's controller instead of going to its owner's graveyard (`ReplaceBy::BattlefieldUnderYours`), while any
//! other countered card goes to the graveyard, at every table size.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{names, pid};

/// A free spell of this type named `name`.
fn spell(name: &str, ty: CardType) -> Arc<CardDefinition> {
    match ty {
        CardType::Creature => free_creature(name, 2, 2),
        other => Arc::new(
            CardDefinition::new(name)
                .with_type(other)
                .with_mana_cost(mtg_testkit::cards::cost(0, &[])),
        ),
    }
}

#[rstest]
fn desertion_steals_artifact_and_creature_spells_only(
    #[values(
        CardType::Creature,
        CardType::Artifact,
        CardType::Enchantment,
        CardType::Sorcery
    )]
    ty: CardType,
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let islands: Vec<_> = (0..5).map(|_| ready(island())).collect();
    let mut scenario = Scenario::new(seats)
        .at(2, 1, Step::Main1)
        .hand(0, [card("Desertion").unwrap()])
        .battlefield(0, islands)
        .hand(1, [spell("Prize", ty)])
        .battlefield(1, [ready(forest())]);
    for seat in 2..seats {
        scenario = scenario.battlefield(seat, [ready(free_creature("Bystander", 1, 1))]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Prize")?;
    // Seat 1 passes with the Prize on the stack; seat 0 answers with Desertion.
    for _ in 0..10 {
        if driver.decision()?.player == pid(0) {
            break;
        }
        driver.pass()?;
    }
    let prize = driver
        .state()
        .stack()
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .find(|object| object.card().name == "Prize")
        .map(|object| mtg_engine::choose::Target::Object(object.reference()))
        .expect("the Prize is on the stack");
    driver.cast_at("Desertion", &[prize])?;
    for _ in 0..50 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        driver.answer_passively()?;
    }
    let stolen = matches!(ty, CardType::Creature | CardType::Artifact);
    let on_field = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| {
            driver
                .state()
                .object(*id)
                .is_some_and(|o| o.card().name == "Prize")
        });
    assert_eq!(on_field.is_some(), stolen);
    if let Some(prize) = on_field {
        assert_eq!(driver.state().object(prize).unwrap().controller(), pid(0));
        assert_eq!(driver.state().object(prize).unwrap().owner(), pid(1));
    }
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(1))).contains(&"Prize".to_owned()),
        !stolen
    );
    Ok(())
}
