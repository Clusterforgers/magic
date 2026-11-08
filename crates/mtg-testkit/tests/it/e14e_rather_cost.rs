//! Goal: a card cast for another cost rather than its mana cost pays that cost and no mana: life equal to its mana value through a permission (Bolas's Citadel) or through a cast during a resolution, and a card whose caster cannot pay it is not offered; at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::creature;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{on_battlefield, pid, spell};

/// Life equal to the mana value of the card cast.
const LIFE_EQUAL: &str = r#"[{"PayLife":{"Stat":{"of":"This","stat":"ManaValue"}}}]"#;

/// "You may cast creature spells from your graveyard. If you cast a spell this way, pay life equal to its mana value
/// rather than pay its mana cost."
fn citadel() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Citadel")
            .with_type(CardType::Artifact)
            .with_rules(
                serde_json::from_str(&format!(
                    r#"{{"Rule":{{"rule":{{"MayCast":{{"from":"Graveyard","what":[{{"Type":"Creature"}}],"rather":{LIFE_EQUAL}}}}}}}}}"#
                ))
                .unwrap(),
            ),
    )
}

/// "You may cast target creature card from your graveyard by paying life equal to its mana value rather than paying its
/// mana cost."
fn recall() -> Arc<CardDefinition> {
    spell(
        "Recall",
        CardType::Sorcery,
        r#"[{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"}]}}]"#,
        &format!(r#"[{{"CastFromZone":{{"what":{{"Target":0}},"rather":{LIFE_EQUAL}}}}}]"#),
    )
}

fn table(seats: usize, mana_value: u32) -> Scenario {
    Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .graveyard(0, [creature("Ogre", mana_value, &[], 3, 3)])
        .hand(0, vec![recall()])
        .battlefield(0, vec![ready(citadel())])
}

fn life(driver: &Driver) -> i32 {
    driver.state().player(pid(0)).unwrap().life()
}

/// Answers until the stack is empty, saying yes to a cast and paying automatically.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..60 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        let decision = driver.decision()?.clone();
        match decision.kind {
            DecisionKind::YesNo(_) => driver.answer(Answer::YesNo(YesNoChoice(true)))?,
            DecisionKind::PayMana(_) => driver.answer(Answer::PayMana(PayChoice::Auto))?,
            _ => driver.answer_passively()?,
        };
    }
    Err(ScriptError::Stuck("the stack".into()))
}

#[rstest]
fn a_permission_takes_life_equal_to_the_mana_value(
    #[values(2, 3, 4)] seats: usize,
    #[values(2, 5)] mana_value: u32,
) -> Result<(), ScriptError> {
    let scenario = table(seats, mana_value);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Ogre")?;
    // No mana is asked for.
    assert_eq!(driver.pay_options()?.cost.to_string(), "{0}");
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver)?;
    assert_eq!(life(&driver), 20 - i32::try_from(mana_value).unwrap());
    assert!(on_battlefield(driver.state(), "Ogre").is_some());
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

#[rstest]
fn a_cast_during_a_resolution_takes_life_equal_to_the_mana_value(
    #[values(2, 3, 4)] seats: usize,
    #[values(2, 5)] mana_value: u32,
) -> Result<(), ScriptError> {
    let scenario = table(seats, mana_value);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let ogre = driver
        .state()
        .zone(mtg_engine::zones::Zone::Graveyard(pid(0)))[0];
    let aim = mtg_engine::choose::Target::Object(driver.state().object(ogre).unwrap().reference());
    driver.cast_at("Recall", &[aim])?;
    settle(&mut driver)?;
    assert_eq!(life(&driver), 20 - i32::try_from(mana_value).unwrap());
    assert!(on_battlefield(driver.state(), "Ogre").is_some());
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

#[test]
fn a_card_whose_life_cannot_be_paid_is_not_offered() -> Result<(), ScriptError> {
    let scenario = table(2, 5).life(0, 4);
    let mut driver = Driver::from_scenario(scenario)?;
    assert!(driver.begin_cast("Ogre").is_err());
    Ok(())
}
