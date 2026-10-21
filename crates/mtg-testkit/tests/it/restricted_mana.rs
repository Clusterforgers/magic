//! Goal: mana added with a restriction ("spend this mana only to cast creature spells") is kept in the pool with what it allows until the pool empties, a payment spends it only on a spell or an ability the restriction allows, a source of restricted mana is a source of such a payment only, restricted mana is spent before mana without a restriction, and a payment that names mana the restriction forbids is refused without a trace.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::spend::{SpendFilter, SpendOn};
use mtg_core::card::CardType;
use mtg_core::mana::{ManaCost, ManaType};
use mtg_engine::Event;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::mana::Payment;
use mtg_engine::spend::Restricted;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{forest, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const SPELL: &str = "Spell";

/// A spell of these types with this cost, `x` of its symbols being {X}.
fn spell(types: &[CardType], cost: &str, x: u8) -> Arc<CardDefinition> {
    let mut definition = CardDefinition::new(SPELL)
        .with_mana_cost(ManaCost::parse(cost).unwrap())
        .with_x_in_cost(x);
    for card_type in types {
        definition = definition.with_type(*card_type);
    }
    if types.contains(&CardType::Creature) {
        definition = definition.with_power_toughness(1, 1);
    }
    Arc::new(definition)
}

/// A table where seat 0 holds the spell and has `mine` in play (cards of the corpus by name; a Forest or a Mountain is a
/// basic land of the testkit).
fn table(spell: Arc<CardDefinition>, mine: &[&str]) -> Result<Driver, ScriptError> {
    let permanents: Vec<_> = mine
        .iter()
        .map(|name| match *name {
            "Forest" => ready(forest()),
            "Mountain" => ready(mountain()),
            other => ready(card(other).unwrap()),
        })
        .collect();
    Driver::from_scenario(Scenario::new(2).hand(0, [spell]).battlefield(0, permanents))
}

fn castable(driver: &Driver) -> Result<bool, ScriptError> {
    match &driver.decision()?.kind {
        DecisionKind::Priority(options) => Ok(!options.castable.is_empty()),
        _ => Err(ScriptError::Expectation("priority was expected".into())),
    }
}

use CardType::{Artifact, Creature, Instant};

#[rstest]
#[case::a_creature_with_a_land_for_creatures(&[Creature], "{G}", 0, &["Ancient Ziggurat"], true)]
#[case::an_instant_is_not_a_creature(&[Instant], "{G}", 0, &["Ancient Ziggurat"], false)]
#[case::an_artifact_creature_is_both(&[Artifact, Creature], "{2}", 0, &["Mishra's Workshop"], true)]
#[case::an_artifact_with_a_land_for_artifacts(&[Artifact], "{2}", 0, &["Mishra's Workshop"], true)]
#[case::a_creature_is_not_an_artifact(&[Creature], "{2}", 0, &["Mishra's Workshop"], false)]
#[case::a_land_for_artifacts_beside_a_forest(&[Artifact], "{3}{G}", 0, &["Mishra's Workshop", "Forest"], true)]
#[case::a_colour_a_land_for_artifacts_cannot_make(&[Artifact], "{G}", 0, &["Mishra's Workshop"], false)]
#[case::two_colours_for_a_multicolored_spell(&[Creature], "{R}{G}", 0, &["Pillar of the Paruns", "Forest"], true)]
#[case::one_colour_is_not_multicolored(&[Creature], "{R}", 0, &["Pillar of the Paruns"], false)]
#[case::the_other_land_pays_a_one_colour_spell(&[Creature], "{R}", 0, &["Pillar of the Paruns", "Mountain"], true)]
#[case::a_cost_with_x(&[Instant], "{C}", 1, &["Rosheen Meanderer"], true)]
#[case::a_cost_without_x(&[Instant], "{C}", 0, &["Rosheen Meanderer"], false)]
fn a_source_of_restricted_mana_pays_only_for_what_it_allows(
    #[case] types: &[CardType],
    #[case] cost: &str,
    #[case] x: u8,
    #[case] mine: &[&str],
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    assert_eq!(castable(&table(spell(types, cost, x), mine)?)?, offered);
    Ok(())
}

fn artifacts() -> Vec<SpendOn> {
    vec![SpendOn::Spell(SpendFilter::of_type(Artifact))]
}

#[test]
fn restricted_mana_floats_with_what_it_allows_and_leaves_with_the_pool() -> Result<(), ScriptError>
{
    let mut driver = table(spell(&[Artifact], "{1}", 0), &["Mishra's Workshop"])?;
    let events = driver.cast(SPELL)?;
    let added: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, Event::ManaAdded { only, .. } if *only == artifacts()))
        .collect();
    assert_eq!(added.len(), 3, "{events:?}");
    let spent = events.iter().find_map(|e| match e {
        Event::ManaSpent {
            mana, restricted, ..
        } => Some((mana.clone(), restricted.clone())),
        _ => None,
    });
    let one = Restricted {
        mana: ManaType::Colorless,
        only: artifacts(),
    };
    assert_eq!(spent, Some((vec![ManaType::Colorless], vec![one.clone()])));
    let pool = driver.state().player(pid(0)).unwrap().mana().clone();
    assert_eq!(pool.restricted(), [one.clone(), one]);
    assert_eq!((pool.get(ManaType::Colorless), pool.total()), (0, 2));
    driver.pass_until(Step::BeginCombat)?;
    assert!(driver.state().player(pid(0)).unwrap().mana().is_empty());
    Ok(())
}

#[test]
fn restricted_mana_is_spent_before_mana_without_a_restriction() -> Result<(), ScriptError> {
    let mut driver = table(
        spell(&[Creature], "{G}{1}", 0),
        &["Ancient Ziggurat", "Forest", "Mountain"],
    )?;
    let events = driver.cast(SPELL)?;
    let tapped: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, Event::PermanentTapped { .. }))
        .collect();
    assert_eq!(tapped.len(), 2, "two sources pay for two mana: {events:?}");
    let spent = events
        .iter()
        .any(|e| matches!(e, Event::ManaSpent { restricted, .. } if restricted.len() == 1));
    assert!(spent, "the land for creatures is one of them: {events:?}");
    assert!(driver.state().player(pid(0)).unwrap().mana().is_empty());
    Ok(())
}

#[test]
fn a_manual_payment_names_the_restricted_mana_it_spends() -> Result<(), ScriptError> {
    let mut driver = table(spell(&[Artifact], "{2}", 0), &["Mishra's Workshop"])?;
    driver.tap_for_mana("Mishra's Workshop")?;
    driver.begin_cast(SPELL)?;
    let options = driver.pay_options()?.clone();
    assert_eq!(options.pool.restricted().len(), 3);
    assert_eq!(options.pool.get(ManaType::Colorless), 0);
    let before = driver.engine().state_hash();
    // Mana without a restriction is not what the pool holds.
    let wrong = Payment {
        pool: vec![ManaType::Colorless, ManaType::Colorless],
        ..Payment::default()
    };
    assert!(
        driver
            .answer(Answer::PayMana(PayChoice::Manual(wrong)))
            .is_err()
    );
    assert_eq!(driver.engine().state_hash(), before);
    let unit = Restricted {
        mana: ManaType::Colorless,
        only: artifacts(),
    };
    let right = Payment {
        restricted: vec![unit.clone(), unit.clone()],
        ..Payment::default()
    };
    let events = driver.answer(Answer::PayMana(PayChoice::Manual(right)))?;
    assert!(events.iter().any(|e| matches!(
        e,
        Event::ManaSpent { restricted, .. } if restricted.len() == 2
    )));
    let pool = driver.state().player(pid(0)).unwrap().mana().clone();
    assert_eq!(pool.restricted(), [unit]);
    Ok(())
}

#[test]
fn restricted_mana_is_not_offered_to_a_spell_it_does_not_allow() -> Result<(), ScriptError> {
    // Three mana for artifacts float; an instant that costs one is not castable with them.
    let mut driver = table(spell(&[Instant], "{1}", 0), &["Mishra's Workshop"])?;
    driver.tap_for_mana("Mishra's Workshop")?;
    assert!(!castable(&driver)?);
    Ok(())
}
