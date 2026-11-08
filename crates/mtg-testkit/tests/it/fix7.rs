//! Goal: a permanent taken from its owner gives its last controller the token when Terastodon's trigger destroys it (CR 400.7: the controller as it last was, not the owner), at two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::VecDeque;
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::characteristics::controller_of;
use mtg_engine::choose::{Target, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, forest, island, plains};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;
use crate::w7d_control_scenarios::w7d_oracle;

/// Terastodon's trigger as the translator writes it, on a creature that costs {1}.
fn terastodon() -> Arc<CardDefinition> {
    let trigger: Ability = serde_json::from_str(
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"Not":{"Type":"Creature"}}]},"bounds":{"min":0,"max":3}}],"effects":[{"Destroy":{"what":{"Target":0}}},{"Together":{"effects":[{"RepeatEach":{"over":{"Target":0},"as":"each","effects":[{"If":{"condition":{"CountOf":{"subject":{"Arrived":{"of":{"Bound":"each"},"zone":"Graveyard"}},"selector":[],"op":"Ge","right":1}},"then":[{"CreateToken":{"who":{"ControllerOf":{"Bound":"each"}},"token":{"Custom":{"name":"Elephant","colors":["Green"],"types":["Creature"],"subtypes":["Elephant"],"power":3,"toughness":3}}}}]}}]}}]}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Terastodon")
            .with_type(CardType::Creature)
            .with_mana_cost(cost(1, &[]))
            .with_power_toughness(9, 9)
            .with_rules(trigger),
    )
}

fn elephants(driver: &Driver, seat: usize) -> usize {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|o| o.card().name == "Elephant" && o.controller() == pid(seat))
        .count()
}

/// Answers until the stack is empty: targets from `picks`, mana automatically, everything else passively.
fn settle(driver: &mut Driver, picks: &mut VecDeque<Target>) -> Result<(), ScriptError> {
    for _ in 0..300 {
        let decision = driver.decision()?.clone();
        let state = driver.state();
        if matches!(decision.kind, DecisionKind::Priority(_))
            && state.stack().is_empty()
            && state.resolving().is_none()
            && state.pending_triggers().is_empty()
        {
            return Ok(());
        }
        let answer = match &decision.kind {
            DecisionKind::ChooseTargets(options) if !picks.is_empty() => {
                let take = (options.max as usize).min(picks.len());
                Answer::ChooseTargets(TargetChoice::Targets(picks.drain(..take).collect()))
            }
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

#[rstest]
fn the_token_goes_to_the_last_controller_of_a_stolen_permanent(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let grab = w7d_oracle(
        "Grab",
        "{U}",
        "Sorcery",
        None,
        "Gain control of target land.",
    );
    let scenario = Scenario::new(seats)
        .hand(0, [grab, terastodon()])
        .battlefield(0, [ready(island()), ready(plains())])
        .battlefield(1, [ready(forest()), ready(plains())])
        .at(1, 0, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    let forest_id = driver.id_of("Forest")?;
    let named =
        |driver: &Driver| Target::Object(driver.state().object(forest_id).unwrap().reference());
    let mut picks: VecDeque<Target> = VecDeque::from([named(&driver)]);
    driver.begin_cast("Grab")?;
    settle(&mut driver, &mut picks)?;
    assert_eq!(
        controller_of(driver.state(), forest_id).unwrap(),
        pid(0),
        "the Forest was stolen"
    );
    picks.push_back(named(&driver));
    driver.begin_cast("Terastodon")?;
    settle(&mut driver, &mut picks)?;
    assert_eq!(
        driver.state().zone(Zone::Graveyard(pid(1))).len(),
        1,
        "the Forest is in its owner's graveyard"
    );
    assert_eq!(
        elephants(&driver, 0),
        1,
        "its last controller gets the token"
    );
    assert_eq!(elephants(&driver, 1), 0, "its owner does not");
    Ok(())
}
