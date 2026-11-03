//! Goal: a reflexive trigger ("when you do", CR 603.12) fires only when the step before it was carried out, goes on the stack as an ability of its own after the step has resolved with its targets chosen then, is removed without leaving the game stuck when no legal target exists, and its log replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::{Ability, Timing};
use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::effect::Effect;
use mtg_cards::refs::{TargetSpec, ZoneKind};
use mtg_engine::choose::{TargetChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::{Event, replay};
use mtg_testkit::cards::free_creature;
use mtg_testkit::trigger_cards::artifact_with;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn effects(json: &str) -> Vec<Effect> {
    serde_json::from_str(json).unwrap()
}

/// "{T}: `first`. When you do, `then`", the reflexive trigger choosing the targets `specs`.
fn rig(first: &str, specs: &str, then: &str) -> Arc<CardDefinition> {
    let mut steps = effects(first);
    steps.push(Effect::Reflexive {
        targets: serde_json::from_str::<Vec<TargetSpec>>(specs).unwrap(),
        effects: effects(then),
    });
    artifact_with(
        "Rig",
        vec![Ability::Activated {
            cost: Cost(vec![CostItem::Tap]),
            timing: Timing::Instant,
            condition: None,
            limit: None,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: steps,
        }],
    )
}

const OPPONENT: &str = r#"[{"what":"Opponent"}]"#;
const DRAIN: &str = r#"[{"LoseLife":{"who":{"Target":0},"amount":3}}]"#;
const GAIN: &str = r#"[{"GainLife":{"amount":1}}]"#;
const MAY_GAIN: &str = r#"[{"May":{"effects":[{"GainLife":{"amount":1}}]}}]"#;
const SACRIFICE: &str = r#"[{"Sacrifice":{"filter":[{"Type":"Creature"}]}}]"#;
const MAY_PAY: &str = r#"[{"May":{"effects":[{"Pay":{"cost":"{1}"}}]}}]"#;
const DISCARD: &str = r#"[{"Discard":{}}]"#;
const PAY: &str = r#"[{"Pay":{"cost":"{1}"}}]"#;
const CREATURE: &str = r#"[{"what":{"Object":[{"Type":"Creature"}]}}]"#;
const DAMAGE: &str = r#"[{"DealDamage":{"to":{"Target":0},"amount":2}}]"#;

/// Plays the activation of the Rig to the end: yes or no to a "may", the first legal target for a trigger.
fn play(driver: &mut Driver, yes: bool) -> Result<Vec<Event>, ScriptError> {
    let mut events = driver.begin_activation("Rig")?;
    for _ in 0..30 {
        let done = matches!(driver.decision()?.kind, DecisionKind::Priority(_))
            && driver.state().stack().is_empty()
            && driver.state().pending_triggers().is_empty();
        if done {
            return Ok(events);
        }
        let decision = driver.decision()?.clone();
        let answer = match &decision.kind {
            DecisionKind::YesNo(o) => Answer::YesNo(YesNoChoice(yes && !o.only_no)),
            DecisionKind::ChooseTargets(o) => Answer::ChooseTargets(TargetChoice::Targets(
                o.legal.iter().copied().take(1).collect(),
            )),
            _ => Driver::passive_answer(&decision),
        };
        events.extend(driver.answer(answer)?);
    }
    Err(ScriptError::NoSuchCard {
        name: "a finished resolution".to_owned(),
        place: "30 answers",
    })
}

fn count(events: &[Event], wanted: fn(&Event) -> bool) -> usize {
    events.iter().filter(|e| wanted(e)).count()
}

/// What happens after each kind of step before "when you do": the opponent has 20 life.
#[rstest]
#[case::always_done(GAIN, OPPONENT, DRAIN, true, true, 17, true)]
#[case::may_accepted(MAY_GAIN, OPPONENT, DRAIN, true, true, 17, true)]
#[case::may_declined(MAY_GAIN, OPPONENT, DRAIN, false, true, 20, false)]
#[case::sacrificed(SACRIFICE, OPPONENT, DRAIN, true, true, 17, true)]
#[case::nothing_to_sacrifice(SACRIFICE, OPPONENT, DRAIN, true, false, 20, false)]
#[case::nothing_to_discard(DISCARD, OPPONENT, DRAIN, true, false, 20, false)]
#[case::an_unpayable_cost_after_a_may(MAY_PAY, OPPONENT, DRAIN, true, false, 20, false)]
#[case::an_unpaid_cost(PAY, OPPONENT, DRAIN, true, false, 20, false)]
fn the_reflexive_trigger_follows_the_step_before_it(
    #[case] first: &str,
    #[case] specs: &str,
    #[case] then: &str,
    #[case] yes: bool,
    #[case] fodder: bool,
    #[case] opponent_life: i32,
    #[case] fires: bool,
) -> Result<(), ScriptError> {
    let mut mine = vec![ready(rig(first, specs, then))];
    if fodder {
        mine.push(ready(free_creature("Fodder", 1, 1)));
    }
    let scenario = || Scenario::new(2).battlefield(0, mine.clone());
    let mut driver = Driver::from_scenario(scenario())?;
    let events = play(&mut driver, yes)?;
    driver.expect_life(1, opponent_life)?;
    // The trigger is an ability of its own, put on the stack after the activation resolved.
    assert_eq!(
        count(&events, |e| matches!(e, Event::AbilityPutOnStack(_))),
        1 + usize::from(fires),
        "{events:?}"
    );
    assert_eq!(driver.state().delayed().len(), 0, "nothing is left waiting");
    let replayed = replay(&scenario().setup().unwrap(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

#[test]
fn a_target_with_no_legal_choice_removes_the_trigger_and_nothing_sticks() -> Result<(), ScriptError>
{
    let mut driver = Driver::from_scenario(
        Scenario::new(2).battlefield(0, [ready(rig(GAIN, CREATURE, DAMAGE))]),
    )?;
    let events = play(&mut driver, true)?;
    assert_eq!(
        count(&events, |e| matches!(e, Event::TriggerRemoved { .. })),
        1
    );
    assert_eq!(
        count(&events, |e| matches!(e, Event::AbilityPutOnStack(_))),
        1,
        "only the activation"
    );
    Ok(())
}

#[test]
fn the_targets_are_chosen_when_the_trigger_goes_on_the_stack() -> Result<(), ScriptError> {
    // A creature is the only legal target: damage kills it once the trigger resolves.
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .battlefield(0, [ready(rig(GAIN, CREATURE, DAMAGE))])
            .battlefield(1, [ready(free_creature("Victim", 1, 1))]),
    )?;
    let events = play(&mut driver, true)?;
    assert_eq!(
        count(&events, |e| matches!(e, Event::CreatureDied { .. })),
        1,
        "{events:?}"
    );
    Ok(())
}
