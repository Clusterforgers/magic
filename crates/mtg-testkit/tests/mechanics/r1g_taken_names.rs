//! Goal: a name bound by a sacrifice inside a "may" is read by the steps after it whether the player accepted or not: an accepted sacrifice names the creature (its toughness is read as it left), a declined one or one with nothing to sacrifice names nothing, so "if you don't" and "if you do" both come out right, at tables of two and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_testkit::cards::free_creature;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common;

/// "You may sacrifice a creature. If you do, gain life equal to its toughness; if you don't, lose 5 life."
fn gamble() -> Arc<CardDefinition> {
    common::spell(
        "Gamble",
        CardType::Instant,
        "[]",
        r#"[{"May":{"effects":[{"Sacrifice":{"filter":[{"Type":"Creature"}],"as":"gone"}}]}},{"If":{"condition":{"CountOf":{"op":"Eq","right":0,"selector":[],"subject":{"Bound":"gone"}}},"then":[{"LoseLife":{"amount":5}}],"otherwise":[{"GainLife":{"amount":{"Stat":{"of":{"Bound":"gone"},"stat":"Toughness"}}}}]}}]"#,
    )
}

/// Casts the spell and resolves it, answering every "may" with `yes`.
fn play(driver: &mut Driver, yes: bool) -> Result<(), ScriptError> {
    driver.cast("Gamble")?;
    for _ in 0..40 {
        if driver.state().stack().is_empty() {
            return Ok(());
        }
        let decision = driver.decision()?.clone();
        let answer = match &decision.kind {
            DecisionKind::YesNo(o) => Answer::YesNo(YesNoChoice(yes && !o.only_no)),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Ok(())
}

#[rstest]
#[case::accepted_with_a_creature(true, 1, 22)]
#[case::declined_with_a_creature(false, 1, 15)]
#[case::accepted_with_nothing_to_sacrifice(true, 0, 15)]
#[case::declined_with_nothing_to_sacrifice(false, 0, 15)]
fn a_name_bound_in_a_may_is_read_after_it(
    #[case] yes: bool,
    #[case] creatures: usize,
    #[case] life: i32,
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let bears: Vec<_> = (0..creatures)
        .map(|n| ready(free_creature(&format!("Bear {n}"), 2, 2)))
        .collect();
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [gamble()])
        .battlefield(0, bears);
    let mut driver = Driver::from_scenario(scenario)?;
    play(&mut driver, yes)?;
    driver.expect_life(0, life)
}

/// "Choose one. If it is your turn, choose two instead", with the fewest and the most both depending on the game.
fn charm(condition: &str) -> Arc<CardDefinition> {
    let count = format!(r#"{{"When":{{"condition":{condition},"then":2,"otherwise":1}}}}"#);
    let mode = r#"{"effects":[{"GainLife":{"amount":1}}]}"#;
    common::spell(
        "Charm",
        CardType::Instant,
        "[]",
        &format!(
            r#"[{{"Modal":{{"choose":{{"min":{count},"max":{count}}},"modes":[{mode},{mode},{mode}]}}}}]"#
        ),
    )
}

#[rstest]
#[case::the_condition_holds(r#""YourTurn""#, 2)]
#[case::the_condition_fails(r#"{"Not":"YourTurn"}"#, 1)]
fn a_fewest_the_game_decides_is_the_fewest_asked(
    #[case] condition: &str,
    #[case] chosen: u32,
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [charm(condition)]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Charm")?;
    let DecisionKind::ChooseMode(options) = &driver.decision()?.kind else {
        panic!("the modes are asked: {}", driver.decision()?.kind.name());
    };
    assert_eq!((options.min, options.max), (chosen, chosen));
    Ok(())
}
