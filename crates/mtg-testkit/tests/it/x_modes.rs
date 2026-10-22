//! Goal: a modal spell with X in the target filter of a mode is offered at priority only when some X the player can pay leaves enough modes with a target (CR 601.2c, 700.2a), as a spell without modes is, and is still asked for its modes when it is.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common;

use common::names;
use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::PresetPermanent;
use mtg_testkit::cards::mountain;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// "Destroy target creature with mana value X".
const DESTROY: &str = r#"{"label":"Destroy","targets":[{"what":{"Object":[{"Type":"Creature"},{"Compare":{"stat":"ManaValue","op":"Eq","to":"X"}}]}}],"effects":[{"Destroy":{"what":{"Target":0},"regenerate":false}}]}"#;
/// "Exile target artifact with mana value X".
const EXILE: &str = r#"{"label":"Exile","targets":[{"what":{"Object":[{"Type":"Artifact"},{"Compare":{"stat":"ManaValue","op":"Eq","to":"X"}}]}}],"effects":[{"Exile":{"what":{"Target":0}}}]}"#;
/// "You gain 2 life", a mode with no target.
const GAIN: &str = r#"{"label":"Gain","effects":[{"GainLife":{"amount":2}}]}"#;

/// An instant for `{R}{X}` that chooses between `min` and `max` of these modes.
fn charm(min: u32, max: u32, modes: &[&str]) -> Arc<CardDefinition> {
    let ability = format!(
        r#"{{"Spell":{{"effects":[{{"Modal":{{"choose":{{"min":{min},"max":{max}}},"modes":[{}]}}}}]}}}}"#,
        modes.join(",")
    );
    Arc::new(
        CardDefinition::new("Charm")
            .with_type(CardType::Instant)
            .with_mana_cost(ManaCost::parse("{R}").unwrap())
            .with_x_in_cost(1)
            .with_rules(serde_json::from_str(&ability).unwrap()),
    )
}

/// A permanent of this type that costs `value` generic mana.
fn permanent(name: String, kind: CardType, value: u32) -> PresetPermanent {
    let card = CardDefinition::new(name)
        .with_type(kind)
        .with_mana_cost(ManaCost::parse(&format!("{{{value}}}")).unwrap());
    ready(Arc::new(if kind == CardType::Creature {
        card.with_power_toughness(2, 2)
    } else {
        card
    }))
}

/// The opponent's permanents of this type, one for each mana value.
fn opposing(kind: CardType, values: &[u32]) -> Vec<PresetPermanent> {
    values
        .iter()
        .enumerate()
        .map(|(at, value)| permanent(format!("{kind:?} {at}"), kind, *value))
        .collect()
}

/// The game in which the player holds the charm with `lands` Mountains against these creatures and artifacts, with the
/// cast begun, or `NoSuchCard` when the charm is not among the castable spells.
fn cast_against(
    charm: Arc<CardDefinition>,
    creatures: &[u32],
    artifacts: &[u32],
    lands: usize,
) -> Result<Driver, ScriptError> {
    let mut permanents = opposing(CardType::Creature, creatures);
    permanents.extend(opposing(CardType::Artifact, artifacts));
    let scenario = Scenario::new(2)
        .hand(0, [charm])
        .battlefield(0, (0..lands).map(|_| ready(mountain())).collect::<Vec<_>>())
        .battlefield(1, permanents);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Charm")?;
    Ok(driver)
}

fn castable(
    charm: Arc<CardDefinition>,
    creatures: &[u32],
    artifacts: &[u32],
    lands: usize,
) -> Result<bool, ScriptError> {
    match cast_against(charm, creatures, artifacts, lands) {
        Ok(_) => Ok(true),
        Err(ScriptError::NoSuchCard { .. }) => Ok(false),
        Err(other) => Err(other),
    }
}

#[rstest]
#[case::a_creature_in_reach(1, 1, &[DESTROY, EXILE], &[2], &[], 3, true)]
#[case::a_creature_out_of_reach(1, 1, &[DESTROY, EXILE], &[2], &[], 2, false)]
#[case::an_artifact_in_reach(1, 1, &[DESTROY, EXILE], &[], &[1], 2, true)]
#[case::nothing_to_target(1, 1, &[DESTROY, EXILE], &[], &[], 5, false)]
#[case::the_other_mode_is_in_reach(1, 1, &[DESTROY, EXILE], &[4], &[1], 2, true)]
#[case::a_mode_without_a_target(1, 1, &[DESTROY, EXILE, GAIN], &[], &[], 1, true)]
#[case::both_modes_at_one_x(2, 2, &[DESTROY, EXILE], &[1], &[1], 2, true)]
#[case::no_x_meets_both(2, 2, &[DESTROY, EXILE], &[1], &[2], 4, false)]
#[case::one_mode_has_no_target(2, 2, &[DESTROY, EXILE], &[1], &[], 3, false)]
#[case::the_mode_without_a_target_completes_two(2, 2, &[DESTROY, GAIN], &[1], &[], 2, true)]
#[case::two_modes_but_one_out_of_reach(2, 2, &[DESTROY, GAIN], &[3], &[], 2, false)]
#[case::up_to_two_may_choose_none(0, 2, &[DESTROY, EXILE], &[], &[], 1, true)]
fn a_modal_spell_with_x_in_a_mode_is_offered_only_when_some_x_has_the_targets(
    #[case] min: u32,
    #[case] max: u32,
    #[case] modes: &[&str],
    #[case] creatures: &[u32],
    #[case] artifacts: &[u32],
    #[case] lands: usize,
    #[case] expected: bool,
) -> Result<(), ScriptError> {
    let offered = castable(charm(min, max, modes), creatures, artifacts, lands)?;
    assert_eq!(offered, expected);
    Ok(())
}

#[test]
fn an_offered_charm_asks_for_its_modes() -> Result<(), ScriptError> {
    let driver = cast_against(charm(1, 1, &[DESTROY, EXILE]), &[2], &[1], 3)?;
    driver.expect_decision(0, "choose-mode")?;
    assert_eq!(names(driver.state().zone(mtg_engine::zones::Zone::Stack)), ["Charm"]);
    Ok(())
}
