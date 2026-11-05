//! Goal: "any opponent may sacrifice" asks the opponents in turn order until one does and takes the creature of that one alone, a card put into a graveyard this turn is told from one that was there before, and both replay to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::{Ability, Timing};
use mtg_cards::cost::Cost;
use mtg_cards::effect::Effect;
use mtg_cards::refs::{TargetSpec, ZoneKind};
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_testkit::cards::free_creature;
use mtg_testkit::trigger_cards::artifact_with;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn effects(json: &str) -> Vec<Effect> {
    serde_json::from_str(json).unwrap()
}

/// A free ability of seat 0's Rig with these targets and effects.
fn free(targets: &str, steps: &str) -> Ability {
    Ability::Activated {
        cost: Cost(Vec::new()),
        timing: Timing::Instant,
        condition: None,
        limit: None,
        from: ZoneKind::Battlefield,
        targets: serde_json::from_str::<Vec<TargetSpec>>(targets).unwrap(),
        effects: effects(steps),
    }
}

/// "Any opponent may sacrifice a creature. If a player does, you gain 1 life", as the translator writes it.
const ANY_OPPONENT: &str = r#"[{"Pay":{"who":"Opponents","cost":[{"Sacrifice":{"filter":[{"Type":"Creature"}]}}],"paid":[{"GainLife":{"amount":1}}]}}]"#;

fn rig(abilities: Vec<Ability>) -> Arc<CardDefinition> {
    artifact_with("Rig", abilities)
}

/// Plays the first ability of the Rig to the end; the seats in `yes` say yes to a question, every other seat no.
fn play(driver: &mut Driver, yes: &[usize]) -> Result<Vec<usize>, ScriptError> {
    let mut asked = Vec::new();
    driver.begin_ability("Rig", 0)?;
    for _ in 0..30 {
        let decision = driver.decision()?.clone();
        let done =
            matches!(decision.kind, DecisionKind::Priority(_)) && driver.state().stack().is_empty();
        if done {
            return Ok(asked);
        }
        let answer = match &decision.kind {
            DecisionKind::YesNo(o) => {
                asked.push(decision.player.index());
                Answer::YesNo(YesNoChoice(
                    yes.contains(&decision.player.index()) && !o.only_no,
                ))
            }
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::NoSuchCard {
        name: "a finished resolution".to_owned(),
        place: "30 answers",
    })
}

#[rstest]
#[case::the_first_says_yes(&[1], &[1], 1, 0, 21)]
#[case::the_second_says_yes_after_the_first_no(&[2], &[1, 2], 0, 1, 21)]
#[case::both_would_but_the_first_decides(&[1, 2], &[1], 1, 0, 21)]
#[case::nobody_says_yes(&[], &[1, 2], 0, 0, 20)]
fn the_opponents_are_asked_in_turn_and_the_one_who_says_yes_sacrifices(
    #[case] yes: &[usize],
    #[case] asked: &[usize],
    #[case] lost_by_first: usize,
    #[case] lost_by_second: usize,
    #[case] life: i32,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(3)
            .battlefield(0, [ready(rig(vec![free("[]", ANY_OPPONENT)]))])
            .battlefield(1, [ready(free_creature("Wall 1", 0, 4))])
            .battlefield(2, [ready(free_creature("Wall 2", 0, 4))])
    };
    let mut driver = Driver::from_scenario(scenario())?;
    assert_eq!(play(&mut driver, yes)?, asked);
    let mut left = vec!["Rig"];
    left.extend((lost_by_first == 0).then_some("Wall 1"));
    left.extend((lost_by_second == 0).then_some("Wall 2"));
    driver.expect_zone(mtg_engine::zones::Zone::Battlefield, &left)?;
    driver.expect_life(0, life)?;
    let replayed = replay(&scenario().setup().unwrap(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

/// "Exile target creature card from a graveyard that was put there this turn", as the translator writes it.
const PUT_THERE_THIS_TURN: &str = r#"[{"what":{"Object":[{"Type":"Creature"},{"AnyOf":[{"ArrivedFrom":"Battlefield"},{"ArrivedFrom":"Hand"},{"ArrivedFrom":"Library"},{"ArrivedFrom":"Graveyard"},{"ArrivedFrom":"Stack"},{"ArrivedFrom":"Exile"},{"ArrivedFrom":"Command"}]},{"In":"Graveyard"}]}}]"#;

#[test]
fn a_creature_card_put_into_a_graveyard_this_turn_is_a_target_and_one_that_lay_there_is_not()
-> Result<(), ScriptError> {
    let rig = rig(vec![
        free("[]", r#"[{"Sacrifice":{"filter":[{"Type":"Creature"}]}}]"#),
        free(PUT_THERE_THIS_TURN, r#"[{"Exile":{"what":{"Target":0}}}]"#),
    ]);
    let scenario = Scenario::new(2)
        .battlefield(0, [ready(rig), ready(free_creature("Fresh", 1, 1))])
        .graveyard(1, [free_creature("Old", 1, 1)]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_ability("Rig", 0)?;
    while !driver.state().stack().is_empty()
        || !matches!(driver.decision()?.kind, DecisionKind::Priority(_))
    {
        let decision = driver.decision()?.clone();
        driver.answer(Driver::passive_answer(&decision))?;
    }
    driver.begin_ability("Rig", 1)?;
    let decision = driver.decision()?.clone();
    let DecisionKind::ChooseTargets(options) = &decision.kind else {
        panic!("a target was expected, got {:?}", decision.kind);
    };
    assert_eq!(options.legal.len(), 1, "only the card put there this turn");
    Ok(())
}
