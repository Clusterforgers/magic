//! Goal: a detained permanent can't have any of its activated abilities activated, mana abilities included (CR 701.35a), until its detainer's next turn, while an undetained one can, at two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{creature, forest};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A free instant that detains the land it targets.
fn detain() -> Arc<CardDefinition> {
    let spell: Ability = serde_json::from_str(
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Land"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"duration":"UntilYourNextTurn","mods":["CantAttack","CantBlock"]}},{"Rule":{"duration":"UntilYourNextTurn","rule":{"Cant":{"action":{"ActivateKind":{"kind":"Every","sources":[]}},"who":{"Target":0}}}}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Detain")
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
#[case::detained(true, false)]
#[case::not_detained(false, true)]
fn a_detained_land_cannot_be_tapped_for_mana(
    #[values(2, 3, 4)] seats: usize,
    #[case] detained: bool,
    #[case] can_cast: bool,
) -> Result<(), ScriptError> {
    let mut hand = vec![creature("Cub", 0, &[mtg_core::color::Color::Green], 1, 1)];
    if detained {
        hand.push(detain());
    }
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, hand)
        .battlefield(0, vec![ready(forest())]);
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 30]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    if detained {
        let land = driver.target_named("Forest")?;
        driver.cast_at("Detain", &[land])?;
        settle(&mut driver)?;
    }
    assert_eq!(driver.cast("Cub").is_ok(), can_cast);
    Ok(())
}
