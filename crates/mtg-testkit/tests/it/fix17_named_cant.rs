//! Goal: "its activated abilities can't be activated" stops the abilities of the target it names, mana abilities included, and of no other permanent, nor of the new object the target becomes once it changes zones (CR 611.2c, 400.7), at two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::mana::ManaCost;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{creature, forest, wastes};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A free instant: no player can activate the target land's abilities, as Deadlock Trap's rule reads; with `bounce`
/// the land then returns to its owner's hand.
fn lock(bounce: bool) -> Arc<CardDefinition> {
    let rule = r#"{"Rule":{"duration":"Forever","rule":{"Cant":{"action":{"ActivateKind":{"kind":"Every","sources":[{"Is":{"Target":0}}]}},"who":"EachPlayer"}}}}"#;
    let back = if bounce {
        r#",{"Move":{"what":{"Target":0},"to":"Hand"}}"#
    } else {
        ""
    };
    let spell: Ability = serde_json::from_str(&format!(
        r#"{{"Spell":{{"targets":[{{"what":{{"Object":[{{"Type":"Land"}}]}}}}],"effects":[{rule}{back}]}}}}"#
    ))
    .unwrap();
    Arc::new(
        CardDefinition::new("Lock")
            .with_type(CardType::Instant)
            .with_mana_cost(ManaCost::parse("{0}").unwrap())
            .with_rules(spell),
    )
}

fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..100 {
        if driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
            && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        {
            return Ok(());
        }
        if matches!(driver.decision()?.kind, DecisionKind::Priority(_)) {
            driver.pass()?;
        } else {
            driver.answer_passively()?;
        }
    }
    Ok(())
}

#[rstest]
#[case::forest_locked(Some("Forest"), false, false)]
#[case::other_land_locked(Some("Wastes"), false, true)]
#[case::nothing_locked(None, false, true)]
#[case::locked_land_returned(Some("Forest"), true, true)]
fn a_locked_land_cannot_be_tapped_for_mana(
    #[values(2, 3, 4)] seats: usize,
    #[case] locked: Option<&str>,
    #[case] bounce: bool,
    #[case] can_cast: bool,
) -> Result<(), ScriptError> {
    let mut hand = vec![creature("Cub", 0, &[Color::Green], 1, 1)];
    if locked.is_some() {
        hand.push(lock(bounce));
    }
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, hand)
        .battlefield(0, vec![ready(forest()), ready(wastes())]);
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![wastes(); 30]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    if let Some(name) = locked {
        let land = driver.target_named(name)?;
        driver.cast_at("Lock", &[land])?;
        settle(&mut driver)?;
    }
    if bounce {
        driver.play_land("Forest")?;
    }
    assert_eq!(driver.cast("Cub").is_ok(), can_cast);
    Ok(())
}
