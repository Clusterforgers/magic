//! Goal: `Prohibited::Cast` refuses a cast for the players it names when the card matches its filter, when the caster has already cast the allowed number of such spells this turn and unless the exception holds, and a game under it replays to the state it reached.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::decision::DecisionKind;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{cost, forest};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

fn ability(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

/// A free spell of the given type that does nothing.
fn dud(name: &str, kind: CardType) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(kind)
            .with_mana_cost(cost(0, &[]))
            .with_rules(ability(r#"{"Spell":{"effects":[]}}"#)),
    )
}

/// Passes priority until the stack is empty.
fn settle(driver: &mut Driver) {
    for _ in 0..40 {
        if driver.state().stack().is_empty()
            || !matches!(driver.decision().unwrap().kind, DecisionKind::Priority(_))
        {
            return;
        }
        driver.pass().unwrap();
    }
    panic!("the stack never emptied");
}

/// Whether each cast of `order` is refused, in a game where a Warden of seat 0 holds the rule.
fn refusals(who: &str, action: &str, order: &[&str]) -> Vec<bool> {
    let rule = format!(r#"{{"Cant":{{"who":{who},"action":{{"Cast":{action}}}}}}}"#);
    let warden = Arc::new(
        CardDefinition::new("Warden")
            .with_type(CardType::Artifact)
            .with_rules(ability(&format!(r#"{{"Rule":{{"rule":{rule}}}}}"#))),
    );
    let scenario = Scenario::new(2)
        .at(2, 0, Step::Main1)
        .hand(
            0,
            vec![
                dud("Dud", CardType::Instant),
                dud("Spare", CardType::Instant),
                dud("Idol", CardType::Artifact),
            ],
        )
        .battlefield(0, vec![ready(warden)])
        .library(0, (0..10).map(|_| forest()))
        .library(1, (0..10).map(|_| forest()));
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let refused = order
        .iter()
        .map(|name| {
            let refused = driver.cast(name).is_err();
            settle(&mut driver);
            refused
        })
        .collect();
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    refused
}

#[rstest]
#[case::none_at_all("\"You\"", "{}", &["Dud", "Idol"], &[true, true])]
#[case::theirs_not_mine("\"Opponents\"", "{}", &["Dud"], &[false])]
#[case::one_each_turn("\"You\"", r#"{"more_than":1}"#, &["Dud", "Spare"], &[false, true])]
#[case::two_each_turn("\"You\"", r#"{"more_than":2}"#, &["Dud", "Spare", "Idol"], &[false, false, true])]
#[case::only_instants_count(
    "\"You\"",
    r#"{"filter":[{"Type":"Instant"}],"more_than":1}"#,
    &["Dud", "Idol", "Spare"],
    &[false, false, true]
)]
#[case::only_artifacts_are_stopped(
    "\"You\"",
    r#"{"filter":[{"Type":"Artifact"}]}"#,
    &["Dud", "Idol"],
    &[false, true]
)]
#[case::unless_it_holds("\"You\"", r#"{"unless":"YourTurn"}"#, &["Dud"], &[false])]
#[case::unless_it_fails("\"You\"", r#"{"unless":"InCombat"}"#, &["Dud"], &[true])]
fn a_cast_is_refused_when_its_prohibition_applies(
    #[case] who: &str,
    #[case] action: &str,
    #[case] order: &[&str],
    #[case] expected: &[bool],
) {
    assert_eq!(refusals(who, action, order), expected);
}
