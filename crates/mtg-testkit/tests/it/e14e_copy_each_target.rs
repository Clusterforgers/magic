//! Goal: "copy that spell for each other creature it could target; each copy targets a different one" makes one copy for each other legal object the selector names, never one for the original target or one outside the selector; at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::creature;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::spell;

/// "Whenever a player casts a spell that targets only a single creature, that player copies that spell for each other
/// creature `each` that spell could target."
fn mirror(each: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Mirror")
            .with_type(CardType::Artifact)
            .with_rules(
                serde_json::from_str(&format!(
                    r#"{{"Triggered":{{"when":{{"SpellCast":{{"what":["SingleTarget",{{"Targeting":[{{"Type":"Creature"}}]}}]}}}},"effects":[{{"CopySpell":{{"what":"Triggering","who":"TriggeringPlayer","each_target":{each}}}}}]}}}}"#
                ))
                .unwrap(),
            ),
    )
}

/// "Tap target creature."
fn zap() -> Arc<CardDefinition> {
    spell(
        "Zap",
        CardType::Sorcery,
        r#"[{"what":{"Object":[{"Type":"Creature"}]}}]"#,
        r#"[{"Tap":{"what":{"Target":0}}}]"#,
    )
}

#[rstest]
#[case::every_creature(r#"[{"Type":"Creature"}]"#, |seats: usize| 3 + 2 * (seats - 1))]
#[case::the_casters_creatures(r#"[{"Type":"Creature"},{"ControlledBy":"Triggering"}]"#, |_| 3)]
fn each_other_legal_target_gets_a_copy(
    #[values(2, 3, 4)] seats: usize,
    #[case] each: &str,
    #[case] tapped: fn(usize) -> usize,
) -> Result<(), ScriptError> {
    let bears = (0..3).map(|_| ready(creature("Bear", 2, &[], 2, 2)));
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, vec![zap()])
        .battlefield(0, bears.chain([ready(mirror(each))]));
    for seat in 1..seats {
        scenario =
            scenario.battlefield(seat, (0..2).map(|_| ready(creature("Ogre", 3, &[], 3, 3))));
    }
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let aim = driver.target_named("Bear")?;
    driver.cast_at("Zap", &[aim])?;
    for _ in 0..60 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        driver.answer_passively()?;
    }
    let state = driver.state();
    let count = state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            state
                .object(**id)
                .and_then(|o| o.permanent())
                .is_some_and(|p| p.tapped())
        })
        .count();
    assert_eq!(count, tapped(seats));
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
