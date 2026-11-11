//! Goal: "~ enters under the control of an opponent of your choice" (`ReplaceBy::UnderChosenOpponent`) puts the permanent onto the battlefield under the opponent its caster picks, at two to four seats and for each opponent picked (no question when there is one), with its owner unchanged; each log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::choose::OptionChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::{Driver, Scenario};
use rstest::rstest;

use crate::common::pid;

const UNDER: &str =
    r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":"UnderChosenOpponent"}}"#;

/// An artifact for {0} that enters under the control of an opponent of its caster's choice.
fn pendant() -> Arc<CardDefinition> {
    let rule: Ability = serde_json::from_str(UNDER).unwrap();
    Arc::new(
        CardDefinition::new("Pendant")
            .with_type(CardType::Artifact)
            .with_mana_cost(mtg_core::mana::ManaCost::parse("{0}").unwrap())
            .with_rules(rule),
    )
}

#[rstest]
fn ep28_the_chosen_opponent_controls_it(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] pick: usize,
) {
    if pick + 1 >= seats {
        return;
    }
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .hand(0, [pendant()]);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    driver.cast("Pendant").unwrap();
    while !driver.state().stack().is_empty() {
        if let DecisionKind::ChooseOption(options) = &driver.decision().unwrap().kind {
            assert_eq!(options.options.len(), seats - 1, "each opponent is offered");
            driver
                .answer(Answer::ChooseOption(OptionChoice(pick)))
                .unwrap();
        } else {
            driver.pass().unwrap();
        }
    }
    let id = driver.id_of("Pendant").unwrap();
    let pendant = driver.state().object(id).unwrap();
    assert_eq!(pendant.controller(), pid(pick + 1), "the chosen opponent");
    assert_eq!(pendant.owner(), pid(0), "its owner stays");
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}
