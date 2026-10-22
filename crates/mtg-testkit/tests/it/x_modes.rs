//! Goal: a modal spell or ability with X in the target filter of a mode is offered at priority only when some X it can pay leaves enough modes with a target (CR 601.2c, 700.2a), as a spell without modes is, and is still asked for its modes, X and targets when it is.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common;

use common::casting::{pay, resolve};
use common::names;
use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::PresetPermanent;
use mtg_engine::choose::{ModeChoice, NumberChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::mountain;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// "Destroy target creature with mana value X".
const DESTROY: &str = r#"{"label":"Destroy","targets":[{"what":{"Object":[{"Type":"Creature"},{"Compare":{"stat":"ManaValue","op":"Eq","to":"X"}}]}}],"effects":[{"Destroy":{"what":{"Target":0},"regenerate":false}}]}"#;
/// "Exile target artifact with mana value X".
const EXILE: &str = r#"{"label":"Exile","targets":[{"what":{"Object":[{"Type":"Artifact"},{"Compare":{"stat":"ManaValue","op":"Eq","to":"X"}}]}}],"effects":[{"Exile":{"what":{"Target":0}}}]}"#;
/// "You gain 2 life", a mode with no target.
const GAIN: &str = r#"{"label":"Gain","effects":[{"GainLife":{"amount":2}}]}"#;

/// The modal effect that chooses between `min` and `max` of these modes, each once unless `repeat`.
fn modal(min: u32, max: u32, repeat: bool, modes: &[&str]) -> String {
    format!(
        r#"{{"Modal":{{"choose":{{"min":{min},"max":{max}}},"repeat":{repeat},"modes":[{}]}}}}"#,
        modes.join(",")
    )
}

/// An instant for `{R}{X}` that chooses between `min` and `max` of these modes.
fn charm(min: u32, max: u32, repeat: bool, modes: &[&str]) -> Arc<CardDefinition> {
    let ability = format!(
        r#"{{"Spell":{{"effects":[{}]}}}}"#,
        modal(min, max, repeat, modes)
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

/// The opponent's permanents of this type, one for each mana value, named after the type and the place in the list.
fn opposing(kind: CardType, values: &[u32]) -> Vec<PresetPermanent> {
    values
        .iter()
        .enumerate()
        .map(|(at, value)| permanent(format!("{kind:?} {at}"), kind, *value))
        .collect()
}

/// The opponent's creatures and artifacts with these mana values.
fn opponent(creatures: &[u32], artifacts: &[u32]) -> Vec<PresetPermanent> {
    let mut permanents = opposing(CardType::Creature, creatures);
    permanents.extend(opposing(CardType::Artifact, artifacts));
    permanents
}

/// `count` Mountains.
fn mountains(count: usize) -> Vec<PresetPermanent> {
    (0..count).map(|_| ready(mountain())).collect()
}

/// The game in which the player holds the charm with `lands` Mountains against these creatures and artifacts, with the
/// cast begun, or `NoSuchCard` when the charm is not among the castable spells.
fn cast_against(
    charm: Arc<CardDefinition>,
    creatures: &[u32],
    artifacts: &[u32],
    lands: usize,
) -> Result<Driver, ScriptError> {
    let scenario = Scenario::new(2)
        .hand(0, [charm])
        .battlefield(0, mountains(lands))
        .battlefield(1, opponent(creatures, artifacts));
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Charm")?;
    Ok(driver)
}

/// Whether the charm is among the castable spells.
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

// Mountains give {R} each: with `lands` of them the most X of the charm is `lands - 1`.
#[rstest]
#[case::a_creature_in_reach(1, 1, &[DESTROY, EXILE], &[2], &[], 3, true)]
#[case::a_creature_out_of_reach(1, 1, &[DESTROY, EXILE], &[2], &[], 2, false)]
#[case::an_artifact_in_reach(1, 1, &[DESTROY, EXILE], &[], &[1], 2, true)]
#[case::nothing_to_target(1, 1, &[DESTROY, EXILE], &[], &[], 5, false)]
#[case::the_other_mode_is_in_reach(1, 1, &[DESTROY, EXILE], &[4], &[1], 2, true)]
#[case::a_mode_without_a_target(1, 1, &[DESTROY, EXILE, GAIN], &[], &[], 1, true)]
#[case::one_of_two_with_one_mode_in_reach(1, 2, &[DESTROY, EXILE], &[9], &[1], 2, true)]
#[case::one_of_two_with_none_in_reach(1, 2, &[DESTROY, EXILE], &[9], &[9], 4, false)]
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
    let offered = castable(charm(min, max, false, modes), creatures, artifacts, lands)?;
    assert_eq!(offered, expected);
    Ok(())
}

// A mode that may repeat is chosen as often as is wanted, so one mode with a target is enough (CR 700.2d).
#[rstest]
#[case::one_mode_in_reach(&[DESTROY, EXILE], &[2], &[], 3, true)]
#[case::the_mode_out_of_reach(&[DESTROY, EXILE], &[9], &[9], 4, false)]
#[case::a_mode_without_a_target(&[DESTROY, GAIN], &[9], &[], 1, true)]
fn a_modal_spell_that_repeats_a_mode_needs_one_mode_with_a_target(
    #[case] modes: &[&str],
    #[case] creatures: &[u32],
    #[case] artifacts: &[u32],
    #[case] lands: usize,
    #[case] expected: bool,
) -> Result<(), ScriptError> {
    let offered = castable(charm(2, 2, true, modes), creatures, artifacts, lands)?;
    assert_eq!(offered, expected);
    Ok(())
}

/// The modes asked for, by index.
fn offered_modes(driver: &Driver) -> Vec<usize> {
    let DecisionKind::ChooseMode(options) = &driver.decision().unwrap().kind else {
        panic!(
            "the modes are asked: {}",
            driver.decision().unwrap().kind.name()
        );
    };
    options.modes.iter().map(|mode| mode.index).collect()
}

#[rstest]
#[case::the_least_x_that_has_a_target(1, "Artifact 0")]
#[case::the_most_x_that_can_be_paid(3, "Artifact 2")]
fn an_offered_charm_asks_for_its_modes_then_x_then_the_targets(
    #[case] x: u32,
    #[case] exiled: &str,
) -> Result<(), ScriptError> {
    let mut driver = cast_against(charm(1, 1, false, &[DESTROY, EXILE]), &[], &[1, 2, 3], 4)?;
    driver.expect_decision(0, "choose-mode")?;
    // There is no creature to destroy at any X.
    assert_eq!(offered_modes(&driver), [1]);
    driver.answer(Answer::ChooseMode(ModeChoice::Modes(vec![1])))?;
    // Only an X with an artifact of that mana value is offered: 0 has none.
    let DecisionKind::ChooseNumber(options) = &driver.decision()?.kind else {
        panic!("X is asked: {}", driver.decision()?.kind.name());
    };
    assert_eq!((options.min, options.max), (1, Some(3)));
    driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
    let target = driver.target_named(exiled)?;
    driver.choose_targets(&[target])?;
    pay(&mut driver)?;
    assert_eq!(names(driver.state(), Zone::Stack), ["Charm"]);
    resolve(&mut driver)?;
    assert_eq!(names(driver.state(), Zone::Exile), [exiled]);
    assert_eq!(names(driver.state(), Zone::Battlefield).len(), 4 + 2);
    Ok(())
}

/// An enchantment whose ability is `ability`; it is neither a creature nor an artifact, so it is no target of the modes.
fn engine(ability: &str) -> PresetPermanent {
    ready(Arc::new(
        CardDefinition::new("Engine")
            .with_type(CardType::Enchantment)
            .with_rules(serde_json::from_str(ability).unwrap()),
    ))
}

/// A planeswalker with 3 loyalty whose `-X` is `effect`.
fn walker(effect: &str) -> PresetPermanent {
    ready(Arc::new(
        CardDefinition::new("Engine")
            .with_type(CardType::Planeswalker)
            .with_loyalty(3)
            .with_rules(
                serde_json::from_str(&format!(
                    r#"{{"Loyalty":{{"cost":"MinusX","effects":[{effect}]}}}}"#
                ))
                .unwrap(),
            ),
    ))
}

/// Whether the engine of the player offers an ability against these creatures and artifacts.
fn ability_offered(
    engine: PresetPermanent,
    lands: usize,
    creatures: &[u32],
    artifacts: &[u32],
) -> Result<bool, ScriptError> {
    let mut mine = vec![engine];
    mine.extend(mountains(lands));
    let scenario = Scenario::new(2)
        .battlefield(0, mine)
        .battlefield(1, opponent(creatures, artifacts));
    let driver = Driver::from_scenario(scenario)?;
    Ok(!driver.offered_abilities("Engine")?.is_empty())
}

// The ability costs {X}: with `lands` of them the most X is `lands`.
#[rstest]
#[case::a_creature_in_reach(1, 1, &[DESTROY, EXILE], &[3], &[], 4, true)]
#[case::a_creature_out_of_reach(1, 1, &[DESTROY, EXILE], &[5], &[], 4, false)]
#[case::an_artifact_in_reach(1, 1, &[DESTROY, EXILE], &[], &[4], 4, true)]
#[case::nothing_to_target(1, 1, &[DESTROY, EXILE], &[], &[], 4, false)]
#[case::a_mode_without_a_target(1, 1, &[DESTROY, GAIN], &[9], &[], 0, true)]
#[case::both_modes_at_one_x(2, 2, &[DESTROY, EXILE], &[2], &[2], 4, true)]
#[case::no_x_meets_both(2, 2, &[DESTROY, EXILE], &[2], &[3], 4, false)]
fn a_modal_activated_ability_with_x_in_a_mode_is_offered_only_when_some_x_has_the_targets(
    #[case] min: u32,
    #[case] max: u32,
    #[case] modes: &[&str],
    #[case] creatures: &[u32],
    #[case] artifacts: &[u32],
    #[case] lands: usize,
    #[case] expected: bool,
) -> Result<(), ScriptError> {
    let ability = format!(
        r#"{{"Activated":{{"cost":"{{X}}","effects":[{}]}}}}"#,
        modal(min, max, false, modes)
    );
    let offered = ability_offered(engine(&ability), lands, creatures, artifacts)?;
    assert_eq!(offered, expected);
    Ok(())
}

// A planeswalker with 3 loyalty counters has -X up to 3.
#[rstest]
#[case::a_creature_in_reach(&[3], &[], true)]
#[case::an_artifact_in_reach(&[], &[1], true)]
#[case::out_of_reach(&[4], &[5], false)]
#[case::nothing_to_target(&[], &[], false)]
fn a_modal_loyalty_ability_with_x_in_a_mode_is_offered_only_when_some_x_has_the_targets(
    #[case] creatures: &[u32],
    #[case] artifacts: &[u32],
    #[case] expected: bool,
) -> Result<(), ScriptError> {
    let effect = modal(1, 1, false, &[DESTROY, EXILE]);
    let offered = ability_offered(walker(&effect), 0, creatures, artifacts)?;
    assert_eq!(offered, expected);
    Ok(())
}
