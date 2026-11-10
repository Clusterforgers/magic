//! Goal: a mana ability that costs mana (filter lands, Signets, a Coffers) is offered at priority exactly when its cost can be paid from floating mana or the other sources, activating it pays that cost and adds its mana, it pays a spell from the mana of the same payment, and without other mana it is neither offered nor accepted.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::mana::{ManaCost, ManaType};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::mana::Payment;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{island, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// A land with one mana ability of these rules.
fn land(name: &str, rules: &str) -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_str(rules).unwrap();
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Land)
            .with_rules(ability),
    )
}

fn catacombs() -> Arc<CardDefinition> {
    land(
        "Darkwater Catacombs",
        r#"{"Activated":{"cost":[{"Mana":"{1}"},"Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Blue","Black"]}}}]}}"#,
    )
}

fn coffers() -> Arc<CardDefinition> {
    land(
        "Coffers",
        r#"{"Activated":{"cost":[{"Mana":"{2}"},"Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Black"]},"times":{"Count":[{"Subtype":"Swamp"},{"ControlledBy":"You"}]}}}]}}"#,
    )
}

/// The source under test, by name: the inline ones or a corpus card.
fn source(name: &str) -> Arc<CardDefinition> {
    match name {
        "Darkwater Catacombs" => catacombs(),
        "Coffers" => coffers(),
        other => card(other).unwrap(),
    }
}

/// Seat 0 controls `name` and `lands`, holds `hand`; turn 1, precombat main.
fn table(
    name: &str,
    lands: Vec<Arc<CardDefinition>>,
    hand: Vec<Arc<CardDefinition>>,
) -> Result<Driver, ScriptError> {
    let mut permanents = vec![ready(source(name))];
    permanents.extend(lands.into_iter().map(ready));
    Driver::from_scenario(
        Scenario::new(2)
            .at(1, 0, Step::Main1)
            .hand(0, hand)
            .battlefield(0, permanents)
            .library(0, (0..5).map(|_| island()))
            .library(1, (0..5).map(|_| island())),
    )
}

fn offered(driver: &Driver, name: &str) -> Vec<mtg_engine::mana::ManaOption> {
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        panic!("priority expected");
    };
    let id = driver.id_of(name).unwrap();
    options
        .mana_abilities
        .iter()
        .filter(|option| option.source.object == id && option.is_costly())
        .cloned()
        .collect()
}

fn floating(driver: &Driver) -> u32 {
    driver.state().player(pid(0)).unwrap().mana().total()
}

/// The lands each source needs to pay its cost: one plains or island pays the {1} or the hybrid, two the {2}.
fn payers(name: &str) -> Vec<Arc<CardDefinition>> {
    match name {
        "Coffers" => vec![swamp(), swamp()],
        "Mystic Gate" | "Azorius Signet" | "Skycloud Expanse" => vec![plains()],
        _ => vec![island()],
    }
}

#[rstest]
#[case::darkwater_catacombs("Darkwater Catacombs", 2)]
#[case::skycloud_expanse("Skycloud Expanse", 2)]
#[case::mystic_gate("Mystic Gate", 2)]
#[case::sunken_ruins("Sunken Ruins", 2)]
#[case::azorius_signet("Azorius Signet", 2)]
#[case::coffers("Coffers", 2)]
fn a_costly_mana_ability_is_paid_from_floating_mana(
    #[case] name: &str,
    #[case] made: u32,
) -> Result<(), ScriptError> {
    let lands = payers(name);
    let mut driver = table(name, lands.clone(), Vec::new())?;
    for land in &lands {
        driver.tap_for_mana(&land.name)?;
    }
    let before = floating(&driver);
    let option = offered(&driver, name)
        .into_iter()
        .next()
        .expect("offered with the cost floating");
    let paid = option.cost.mana_value();
    driver.answer(Answer::Priority(PriorityAction::ActivateMana(
        option.source,
    )))?;
    assert_eq!(floating(&driver), before - paid + made);
    assert!(
        driver
            .state()
            .object(driver.id_of(name)?)
            .unwrap()
            .permanent()
            .unwrap()
            .tapped()
    );
    Ok(())
}

#[rstest]
#[case::darkwater_catacombs("Darkwater Catacombs")]
#[case::skycloud_expanse("Skycloud Expanse")]
#[case::mystic_gate("Mystic Gate")]
#[case::azorius_signet("Azorius Signet")]
#[case::coffers("Coffers")]
fn a_costly_mana_ability_taps_another_source_for_its_cost(
    #[case] name: &str,
) -> Result<(), ScriptError> {
    let mut driver = table(name, payers(name), Vec::new())?;
    let option = offered(&driver, name)
        .into_iter()
        .next()
        .expect("offered with untapped lands");
    driver.answer(Answer::Priority(PriorityAction::ActivateMana(
        option.source,
    )))?;
    assert_eq!(
        floating(&driver),
        u32::try_from(option.made().len()).unwrap()
    );
    let untapped = driver
        .state()
        .zone(mtg_engine::zones::Zone::Battlefield)
        .iter()
        .filter(|id| {
            !driver
                .state()
                .object(**id)
                .unwrap()
                .permanent()
                .unwrap()
                .tapped()
        })
        .count();
    assert_eq!(untapped, 0, "the cost tapped the other lands");
    Ok(())
}

#[rstest]
#[case::darkwater_catacombs("Darkwater Catacombs")]
#[case::skycloud_expanse("Skycloud Expanse")]
#[case::azorius_signet("Azorius Signet")]
#[case::coffers("Coffers")]
fn without_other_mana_a_costly_mana_ability_is_neither_offered_nor_accepted(
    #[case] name: &str,
) -> Result<(), ScriptError> {
    let mut driver = table(name, Vec::new(), Vec::new())?;
    assert_eq!(offered(&driver, name), Vec::new());
    let id = driver.id_of(name)?;
    for ability in 0..2 {
        let source = mtg_engine::mana::ManaSource {
            object: id,
            ability,
        };
        assert!(
            driver
                .answer(Answer::Priority(PriorityAction::ActivateMana(source)))
                .is_err()
        );
    }
    Ok(())
}

/// A two-mana spell of the colours Darkwater Catacombs adds.
fn dimir_bear() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Dimir Bear")
            .with_type(CardType::Creature)
            .with_power_toughness(2, 2)
            .with_mana_cost(ManaCost::parse("{U}{B}").unwrap()),
    )
}

#[rstest]
#[case::the_cost_floating(true)]
#[case::the_cost_from_a_land_of_the_payment(false)]
fn darkwater_catacombs_pays_a_spell_inside_the_payment(
    #[case] float_first: bool,
) -> Result<(), ScriptError> {
    let mut driver = table("Darkwater Catacombs", vec![plains()], vec![dimir_bear()])?;
    if float_first {
        driver.tap_for_mana("Plains")?;
    }
    driver.begin_cast("Dimir Bear")?;
    let options = driver.pay_options()?.clone();
    let catacombs = driver.id_of("Darkwater Catacombs")?;
    let plains = driver.id_of("Plains")?;
    let mut taps: Vec<_> = options
        .sources
        .iter()
        .filter(|o| o.source.object == catacombs)
        .map(|o| o.source)
        .take(1)
        .collect();
    assert_eq!(taps.len(), 1, "Catacombs is a source of the payment");
    let mut pool = Vec::new();
    if float_first {
        pool.push(ManaType::White);
    } else {
        let land = options
            .sources
            .iter()
            .find(|o| o.source.object == plains)
            .map(|o| o.source)
            .expect("the plains is a source");
        taps.insert(0, land);
    }
    driver.answer(Answer::PayMana(PayChoice::Manual(Payment {
        pool,
        taps,
        ..Payment::default()
    })))?;
    assert_eq!(driver.state().stack().len(), 1, "the bear is cast");
    assert_eq!(floating(&driver), 0);
    Ok(())
}
