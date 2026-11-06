//! Goal: when each player chooses among what they control (`Ref::Chooser`), every player is offered only their own permanents, a player with fewer than the count keeps them all unasked, what they chose is kept together, and the log replays; at 2 to 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, free_creature, plains};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// "Each player chooses a creature they control. Destroy the rest." for {W}.
fn reckoning() -> Arc<CardDefinition> {
    let rules: Ability = serde_json::from_str(
        r#"{"Spell":{"effects":[
            {"Choose":{"who":"EachPlayer","from":{"Each":[{"Type":"Creature"},{"ControlledBy":"Chooser"}]},"as":"kept"}},
            {"Destroy":{"what":{"Each":[{"Type":"Creature"},{"Not":{"Is":{"Bound":"kept"}}}]}}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Reckoning")
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[Color::White]))
            .with_rules(rules),
    )
}

fn creatures(driver: &Driver, seat: usize) -> usize {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            let object = driver.state().object(**id).unwrap();
            object.controller() == pid(seat) && object.card().types.contains(&CardType::Creature)
        })
        .count()
}

#[rstest]
fn each_player_chooses_one_of_their_own_creatures_and_the_rest_are_destroyed(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // Seat 0 has two creatures and every other seat one, except the last seat, which has three.
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, vec![reckoning()]);
    for seat in 0..seats {
        let count = match seat {
            0 => 2,
            s if s == seats - 1 => 3,
            _ => 1,
        };
        let mut permanents: Vec<_> = (0..count)
            .map(|n| ready(free_creature(&format!("Unit {seat}-{n}"), 1, 1)))
            .collect();
        if seat == 0 {
            permanents.push(ready(plains()));
        }
        scenario = scenario.battlefield(seat, permanents);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Reckoning")?;
    let mut asked = Vec::new();
    for _ in 0..40 {
        let decision = driver.decision()?;
        match &decision.kind {
            DecisionKind::ChooseCards(options) => {
                let player = decision.player;
                assert!(
                    options
                        .from
                        .iter()
                        .all(|id| driver.state().object(*id).unwrap().controller() == player),
                    "{player:?} is offered only their own creatures"
                );
                asked.push(player);
                let first = options.from[0];
                driver.answer(Answer::ChooseCards(CardsChoice(vec![first])))?;
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            _ => {
                driver.pass()?;
            }
        }
    }
    asked.sort();
    let mut expected = vec![pid(0)];
    if seats > 1 {
        expected.push(pid(seats - 1));
    }
    assert_eq!(asked, expected, "only players with a choice are asked");
    for seat in 0..seats {
        assert_eq!(
            creatures(&driver, seat),
            1,
            "seat {seat} keeps one creature"
        );
    }
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    Ok(())
}
