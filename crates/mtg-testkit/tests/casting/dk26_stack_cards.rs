//! Goal: the corpus cards Tale's End and Wild Ricochet play as printed: Tale's End counters an activated ability, a
//! triggered ability or a legendary spell and may not target a spell that is not legendary; Wild Ricochet lets its
//! controller point an instant at another player and copies it, so both deal their damage there.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::{CardType, Supertype};
use mtg_core::color::Color;
use mtg_engine::choose::{Target, TargetChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, creature, island, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{names, pid};

const GAIN: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"GainLife":{"amount":3}}]}}"#;

fn trinket() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Trinket")
            .with_type(CardType::Artifact)
            .with_rules(serde_json::from_str::<Ability>(GAIN).unwrap()),
    )
}

fn legend() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::clone(&creature("Legend", 0, &[Color::Red], 2, 2))
            .with_supertype(Supertype::Legendary),
    )
}

/// Passes until the stack is empty, answering any question with yes, the first legal target or the passive answer.
fn settle(driver: &mut Driver, retarget: Option<Target>) -> Result<(), ScriptError> {
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
            DecisionKind::ChooseTargets(o) => {
                let chosen = retarget
                    .filter(|target| o.legal.contains(target))
                    .or_else(|| o.legal.first().copied());
                Answer::ChooseTargets(TargetChoice::Targets(chosen.into_iter().collect()))
            }
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Seat 0 holds `spell` and a Mountain and controls the Trinket; seat 1 holds Tale's End and two Islands.
fn tales_table(spell: Arc<CardDefinition>) -> Scenario {
    Scenario::new(2)
        .hand(0, [spell])
        .battlefield(0, [ready(mountain()), ready(trinket())])
        .hand(1, [card("Tale's End").unwrap()])
        .battlefield(1, [ready(island()), ready(island())])
}

#[rstest]
#[case::a_legendary_spell(legend(), true)]
#[case::a_spell_that_is_not_legendary(creature("Bear", 0, &[Color::Red], 2, 2), false)]
fn tales_end_counters_only_a_legendary_spell(
    #[case] spell: Arc<CardDefinition>,
    #[case] countered: bool,
) -> Result<(), ScriptError> {
    let name = spell.name.clone();
    let mut driver = Driver::from_scenario(tales_table(spell))?;
    driver.cast(&name)?;
    let on_stack = driver.state().stack()[0];
    let at = Target::Object(driver.state().object(on_stack).unwrap().reference());
    driver.pass()?;
    let cast = driver.cast_at("Tale's End", &[at]);
    assert_eq!(cast.is_ok(), countered, "{cast:?}");
    if cast.is_err() {
        driver.pass()?;
    }
    settle(&mut driver, None)?;
    let in_play = names(driver.state(), Zone::Battlefield).contains(&name);
    assert_eq!(in_play, !countered);
    Ok(())
}

#[test]
fn tales_end_counters_an_activated_ability() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(tales_table(legend()))?;
    driver.begin_activation("Trinket")?;
    settle_until_stack(&mut driver)?;
    let on_stack = driver.state().stack()[0];
    let at = Target::Object(driver.state().object(on_stack).unwrap().reference());
    driver.pass()?;
    driver.cast_at("Tale's End", &[at])?;
    settle(&mut driver, None)?;
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), 20);
    Ok(())
}

/// Answers the questions of an activation until it is on the stack.
fn settle_until_stack(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..10 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::Priority(_)) {
            return Ok(());
        }
        let answer = match &decision.kind {
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck("the activation".into()))
}

#[rstest]
#[case::pointed_back(true, 14)]
#[case::kept_on_me(false, 20)]
fn wild_ricochet_turns_the_spell_and_its_copy_on_another_player(
    #[case] back: bool,
    #[case] caster_life: i32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [burn("Bolt", 3)])
            .battlefield(0, [ready(mountain())])
            .hand(1, [card("Wild Ricochet").unwrap()])
            .battlefield(1, (0..4).map(|_| ready(mountain()))),
    )?;
    driver.begin_cast("Bolt")?;
    driver.choose_targets(&[Target::Player(pid(1))])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    let on_stack = driver.state().stack()[0];
    let at = Target::Object(driver.state().object(on_stack).unwrap().reference());
    driver.pass()?;
    driver.cast_at("Wild Ricochet", &[at])?;
    let aim = if back { pid(0) } else { pid(1) };
    settle(&mut driver, Some(Target::Player(aim)))?;
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), caster_life);
    let target_life = if back { 20 } else { 14 };
    assert_eq!(driver.state().player(pid(1)).unwrap().life(), target_life);
    Ok(())
}
