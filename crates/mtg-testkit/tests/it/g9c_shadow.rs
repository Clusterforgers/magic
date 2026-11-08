//! Goal: a rule lets a creature block creatures with shadow (CR 702.28) as though it had shadow, or as though they had
//! none, which a creature with shadow then can't block; without the rule shadow stops the block.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::combat::to_blockers;
use common::name_of;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_testkit::cards::free_creature;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn creature(name: &str, shadow: bool) -> Arc<CardDefinition> {
    let card = CardDefinition::new(name)
        .with_type(CardType::Creature)
        .with_power_toughness(2, 3);
    Arc::new(if shadow {
        card.with_rules(Ability::Keyword(Keyword::Shadow))
    } else {
        card
    })
}

fn rule_card(quirk: &str) -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_str(&format!(
        r#"{{"Rule":{{"rule":{{"Combat":{{"who":{{"Each":[{{"Named":"B"}}]}},"quirk":"{quirk}"}}}}}}}}"#
    ))
    .unwrap();
    Arc::new(
        CardDefinition::new("Rulebook")
            .with_type(CardType::Enchantment)
            .with_rules(ability),
    )
}

/// The blockers offered against the shadow attacker "A" by a defender with the creature "B" and the rule.
fn offered(blocker_shadow: bool, quirk: Option<&str>) -> Result<Vec<String>, ScriptError> {
    let mut theirs = vec![ready(creature("B", blocker_shadow))];
    theirs.extend(quirk.map(|quirk| ready(rule_card(quirk))));
    let mut scenario = Scenario::new(2)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, [ready(creature("A", true))])
        .battlefield(1, theirs);
    for seat in 0..2 {
        scenario = scenario.library(seat, (0..10).map(|_| free_creature("Filler", 1, 1)));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.attack(&[("A", 1)])?;
    to_blockers(&mut driver, 1)?;
    let Ok(decision) = driver.decision() else {
        return Ok(Vec::new());
    };
    let DecisionKind::DeclareBlockers(options) = &decision.kind else {
        return Ok(Vec::new());
    };
    Ok(options
        .blockers
        .iter()
        .map(|blocker| name_of(driver.state(), blocker.object))
        .collect())
}

#[rstest]
#[case::shadow_stops_the_block(false, None, &[])]
#[case::as_though_it_had_shadow(false, Some("BlockAsThoughShadow"), &["B"])]
#[case::as_though_they_had_none(false, Some("BlockIgnoringShadow"), &["B"])]
#[case::a_shadow_blocker_blocks_shadow(true, None, &["B"])]
#[case::a_shadow_blocker_as_though_it_had_shadow(true, Some("BlockAsThoughShadow"), &["B"])]
#[case::a_shadow_blocker_blocks_no_creature_without_shadow(true, Some("BlockIgnoringShadow"), &[])]
fn a_rule_lets_a_creature_block_shadow(
    #[case] blocker_shadow: bool,
    #[case] quirk: Option<&str>,
    #[case] expected: &[&str],
) -> Result<(), ScriptError> {
    assert_eq!(offered(blocker_shadow, quirk)?, expected);
    Ok(())
}
