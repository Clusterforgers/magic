//! Goal: a counterspell that names the source of the ability it counters plays as printed at 2 to 4 seats: Rust and
//! Interdict target only an ability whose source has the named type, Interdict and Trickbind stop that permanent's
//! activated abilities for the turn and leave another permanent's alone, and Teferi's Response counters only an
//! opponent's ability that targets a land of its controller and destroys that ability's source.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::choose::{Target, TargetChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, forest, island, mountain, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{names, pid};

const GAIN: &str = r#"{"Activated":{"cost":"{1}","effects":[{"GainLife":{"amount":3}}]}}"#;
const WRECK: &str = r#"{"Activated":{"cost":"{1}","targets":[{"what":{"Object":[{"Type":"Land"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#;

fn with_rules(card: CardDefinition, rules: &str) -> Arc<CardDefinition> {
    Arc::new(card.with_rules(serde_json::from_str::<Ability>(rules).unwrap()))
}

/// An artifact with "{1}: You gain 3 life."
fn trinket() -> Arc<CardDefinition> {
    with_rules(
        CardDefinition::new("Trinket").with_type(CardType::Artifact),
        GAIN,
    )
}

/// A creature with "{1}: You gain 3 life."
fn healer() -> Arc<CardDefinition> {
    with_rules(
        CardDefinition::clone(&creature("Healer", 0, &[Color::Red], 1, 1)),
        GAIN,
    )
}

/// An artifact with "{1}: Destroy target land."
fn wrecker() -> Arc<CardDefinition> {
    with_rules(
        CardDefinition::new("Wrecker").with_type(CardType::Artifact),
        WRECK,
    )
}

/// Seat 0 controls the Trinket, the Healer, the Wrecker and three Mountains; seat 1 holds `spell` with two Islands and
/// two Plains and a Forest and a library to draw from.
fn table(seats: usize, spell: &str) -> Scenario {
    Scenario::new(seats)
        .battlefield(
            0,
            [
                ready(trinket()),
                ready(healer()),
                ready(wrecker()),
                ready(mountain()),
                ready(mountain()),
                ready(mountain()),
            ],
        )
        .hand(1, [card(spell).unwrap()])
        .battlefield(
            1,
            [
                ready(island()),
                ready(island()),
                ready(plains()),
                ready(plains()),
                ready(forest()),
            ],
        )
        .library(1, (0..4).map(|_| island()))
}

/// Passes until the stack is empty, answering any question with yes, the first legal target or the passive answer.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::Priority(_)) && driver.state().stack().is_empty() {
            return Ok(());
        }
        let answer = match &decision.kind {
            DecisionKind::Priority(_) => {
                driver.pass()?;
                continue;
            }
            DecisionKind::YesNo(o) => Answer::YesNo(YesNoChoice(!o.only_no)),
            DecisionKind::ChooseTargets(o) => Answer::ChooseTargets(TargetChoice::Targets(
                o.legal.first().copied().into_iter().collect(),
            )),
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Answers the questions of an activation until it is on the stack, aiming at `aim` when it asks for a target.
fn activate(driver: &mut Driver, name: &str, aim: Option<Target>) -> Result<Target, ScriptError> {
    driver.begin_activation(name)?;
    for _ in 0..10 {
        let decision = driver.decision()?.clone();
        let answer = match &decision.kind {
            DecisionKind::Priority(_) => {
                let on_stack = driver.state().stack()[0];
                return Ok(Target::Object(
                    driver.state().object(on_stack).unwrap().reference(),
                ));
            }
            DecisionKind::ChooseTargets(_) => {
                Answer::ChooseTargets(TargetChoice::Targets(aim.into_iter().collect()))
            }
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck("the activation".into()))
}

/// The answer to an ability of `source`: whether the spell could be cast at it, and seat 0's life once all resolved.
fn answer(seats: usize, spell: &str, source: &str) -> Result<(Driver, bool), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, spell))?;
    let ability = activate(&mut driver, source, None)?;
    driver.pass()?;
    let cast = driver.cast_at(spell, &[ability]);
    if cast.is_err() {
        driver.pass()?;
    }
    settle(&mut driver)?;
    Ok((driver, cast.is_ok()))
}

#[rstest]
#[case::rust_an_artifact("Rust", "Trinket", 2, true)]
#[case::rust_a_creature("Rust", "Healer", 4, false)]
#[case::interdict_an_artifact("Interdict", "Trinket", 3, true)]
#[case::interdict_a_creature("Interdict", "Healer", 2, true)]
#[case::trickbind_a_creature("Trickbind", "Healer", 4, true)]
fn the_spell_counters_an_ability_of_the_named_source(
    #[case] spell: &str,
    #[case] source: &str,
    #[case] seats: usize,
    #[case] countered: bool,
) -> Result<(), ScriptError> {
    let (driver, cast) = answer(seats, spell, source)?;
    assert_eq!(cast, countered, "{spell} at {source}");
    let life = driver.state().player(pid(0)).unwrap().life();
    assert_eq!(life, if countered { 20 } else { 23 });
    Ok(())
}

#[rstest]
#[case::interdict("Interdict", 2, 1)]
#[case::interdict_at_four("Interdict", 4, 1)]
#[case::trickbind("Trickbind", 3, 0)]
fn the_countered_source_cannot_activate_this_turn(
    #[case] spell: &str,
    #[case] seats: usize,
    #[case] drawn: usize,
) -> Result<(), ScriptError> {
    let (driver, cast) = answer(seats, spell, "Trinket")?;
    assert!(cast);
    assert!(
        driver.offered_abilities("Trinket")?.is_empty(),
        "the Trinket is stopped"
    );
    assert!(
        !driver.offered_abilities("Healer")?.is_empty(),
        "another permanent is not"
    );
    let hand = driver.state().zone(Zone::Hand(pid(1))).len();
    assert_eq!(hand, drawn);
    Ok(())
}

#[rstest]
#[case::at_my_land(2, true)]
#[case::at_my_land_at_four(4, true)]
#[case::at_its_controllers_own_land(3, false)]
fn teferis_response_destroys_the_source_of_an_ability_at_my_land(
    #[case] seats: usize,
    #[case] mine: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, "Teferi's Response"))?;
    let land = driver.id_of(if mine { "Island" } else { "Mountain" })?;
    let land = Target::Object(driver.state().object(land).unwrap().reference());
    let ability = activate(&mut driver, "Wrecker", Some(land))?;
    driver.pass()?;
    let cast = driver.cast_at("Teferi's Response", &[ability]);
    assert_eq!(cast.is_ok(), mine);
    if cast.is_err() {
        driver.pass()?;
    }
    settle(&mut driver)?;
    let board = names(driver.state(), Zone::Battlefield);
    assert_eq!(board.contains(&"Wrecker".to_owned()), !mine);
    let hand = driver.state().zone(Zone::Hand(pid(1))).len();
    assert_eq!(hand, if mine { 2 } else { 1 });
    Ok(())
}
