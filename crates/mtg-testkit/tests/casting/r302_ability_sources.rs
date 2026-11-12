//! Goal: a spell that counters "target activated ability from an artifact source" can target the ability of an artifact
//! and not that of a creature, and one that also says "that permanent's activated abilities can't be activated this
//! turn" counters the ability and keeps its source from activating again, at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::choose::Target;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{pid, spell};

/// A permanent of `kind` with "{0}: Target player loses 1 life."
fn pinger(kind: CardType) -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_str(
        r#"{"Activated":{"cost":"{0}","targets":[{"what":"Player"}],"effects":[{"LoseLife":{"who":{"Target":0},"amount":1}}]}}"#,
    )
    .unwrap();
    let mut card = CardDefinition::new("Pinger")
        .with_type(kind)
        .with_rules(ability);
    if kind == CardType::Creature {
        card = card.with_power_toughness(1, 1);
    }
    Arc::new(card)
}

const FROM_ARTIFACT: &str =
    r#"[{"what":{"Object":[{"Ability":"Activated"},{"AbilityFrom":[{"Type":"Artifact"}]}]}}]"#;
const ANY_ACTIVATED: &str = r#"[{"what":{"Object":[{"Ability":"Activated"}]}}]"#;
const COUNTER: &str = r#"[{"Counter":{"what":{"Target":0}}}]"#;
const LOCK_AND_COUNTER: &str = r#"[{"Choose":{"from":{"Each":[{"SourceOf":{"Target":0}}]},"as":"source"}},
    {"Rule":{"rule":{"Cant":{"who":"EachPlayer","action":{"ActivateKind":{"kind":"Every","sources":[{"Is":{"Bound":"source"}}]}}}},"duration":"UntilEndOfTurn"}},
    {"Counter":{"what":{"Target":0}}}]"#;

/// Passes and answers until the stack is empty and priority is asked.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..60 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::Priority(_))
            && driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
        {
            return Ok(());
        }
        let answer = match &decision.kind {
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck("an empty stack".to_owned()))
}

#[rstest]
#[case::from_an_artifact(CardType::Artifact, FROM_ARTIFACT, COUNTER, true, false)]
#[case::not_from_a_creature(CardType::Creature, FROM_ARTIFACT, COUNTER, false, false)]
#[case::locks_an_artifact(CardType::Artifact, ANY_ACTIVATED, LOCK_AND_COUNTER, true, true)]
#[case::locks_a_creature(CardType::Creature, ANY_ACTIVATED, LOCK_AND_COUNTER, true, true)]
fn a_counter_reads_the_source_of_the_ability(
    #[values(2, 3, 4)] seats: usize,
    #[case] kind: CardType,
    #[case] targets: &str,
    #[case] effects: &str,
    #[case] legal: bool,
    #[case] locks: bool,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(
            0,
            vec![spell("Stopper", CardType::Instant, targets, effects)],
        )
        .battlefield(0, vec![ready(pinger(kind))]);
    let mut driver = Driver::from_scenario(scenario)?;
    let life = |driver: &Driver| driver.state().player(pid(1)).unwrap().life();
    let before = life(&driver);
    driver.begin_activation("Pinger")?;
    driver.answer(Answer::ChooseTargets(
        mtg_engine::choose::TargetChoice::Targets(vec![Target::Player(pid(1))]),
    ))?;
    if matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
    }
    let ability = *driver.state().stack().last().unwrap();
    let ability = Target::Object(driver.state().object(ability).unwrap().reference());
    let cast = driver.cast_at("Stopper", &[ability]);
    assert_eq!(cast.is_ok(), legal, "{kind:?} legal {legal}");
    settle(&mut driver)?;
    let lost = before - life(&driver);
    assert_eq!(lost, if legal { 0 } else { 1 }, "{kind:?}");
    let again = driver.begin_activation("Pinger").is_ok();
    assert_eq!(again, !locks, "{kind:?} locks {locks}");
    Ok(())
}
