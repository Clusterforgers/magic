//! Goal: a transformed double-faced permanent that dies triggers the abilities of the face it was in use as, not those of the front face of the card that lies in the graveyard, so the game goes on and the back face's "when this dies" resolves.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::{names, pid, spell};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::definition::Layout;
use mtg_core::card::CardType;
use mtg_engine::decision::DecisionKind;
use mtg_engine::replay;
use mtg_engine::zones::Zone;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};

/// A creature that transforms when activated into one that gains 3 life for its controller when it dies, and whose
/// front face has an ability of another kind at the place of that trigger.
fn moon_beast() -> Arc<CardDefinition> {
    let turn: Ability = serde_json::from_str(
        r#"{"Activated":{"cost":[],"effects":[{"Transform":{"what":"This"}}]}}"#,
    )
    .unwrap();
    let dies: Ability = serde_json::from_str(
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"effects":[{"GainLife":{"amount":3}}]}}"#,
    )
    .unwrap();
    let front = CardDefinition::new("Moon Beast")
        .with_type(CardType::Creature)
        .with_power_toughness(2, 2)
        .with_rules(turn.clone());
    let back = CardDefinition::new("Moon Fiend")
        .with_type(CardType::Creature)
        .with_power_toughness(4, 4)
        .with_rules(dies);
    Arc::new(
        CardDefinition::new("Moon Beast")
            .with_type(CardType::Creature)
            .with_power_toughness(2, 2)
            .with_rules(turn)
            .with_faces(Layout::Transform, vec![front, back]),
    )
}

fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..12 {
        let priority = matches!(driver.decision()?.kind, DecisionKind::Priority(_));
        if priority
            && driver.state().stack().is_empty()
            && driver.state().pending_triggers().is_empty()
        {
            return Ok(());
        }
        driver.pass()?;
    }
    Ok(())
}

#[test]
fn the_back_face_dies_trigger_runs_for_a_permanent_that_died_transformed() -> Result<(), ScriptError>
{
    let smite = spell(
        "Smite",
        CardType::Instant,
        r#"[{"what":{"Object":[{"Type":"Creature"}]}}]"#,
        r#"[{"Destroy":{"what":{"Target":0}}}]"#,
    );
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .battlefield(0, [ready(moon_beast())])
            .hand(0, [smite]),
    )?;
    driver.begin_activation("Moon Beast")?;
    settle(&mut driver)?;
    let target = driver.target_named("Moon Fiend")?;
    driver.cast_at("Smite", &[target])?;
    settle(&mut driver)?;

    assert_eq!(
        names(driver.state(), Zone::Battlefield),
        Vec::<String>::new()
    );
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), 23);
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    Ok(())
}
