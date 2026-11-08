//! Goal: a land that enters tapped unless its controller reveals a card of a type or controls a permanent of it asks for the reveal only without such a permanent, enters untapped with a reveal or a permanent and tapped without both, at 2, 3 and 4 seats.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;
use serde_json::json;

fn dragon() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Whelp")
            .with_type(CardType::Creature)
            .with_subtype("Dragon")
            .with_power_toughness(1, 1),
    )
}

/// "As it enters, you may reveal a Dragon card from your hand. It enters tapped unless you revealed one or you control a Dragon."
fn temple() -> Arc<CardDefinition> {
    let rule: Ability = serde_json::from_value(json!({"Replacement": {
        "when": {"Enters": {"what": ["This"]}},
        "by": {"RevealOrTapped": [{"Subtype": "Dragon"}, {"In": "Hand"}, {"OwnedBy": "You"}]},
        "condition": {"Not": {"Compare": {"left": {"Count": [{"Subtype": "Dragon"},
            {"In": "Battlefield"}, {"ControlledBy": "You"}]}, "op": "Ge", "right": 1}}}}}))
    .unwrap();
    Arc::new(
        CardDefinition::new("Temple")
            .with_type(CardType::Land)
            .with_rules(rule),
    )
}

#[rstest]
#[case::dragon_in_play_asks_nothing(true, false, None, false)]
#[case::reveals_the_dragon_in_hand(false, true, Some(true), false)]
#[case::declines_to_reveal(false, true, Some(false), true)]
#[case::no_dragon_anywhere_only_no(false, false, Some(false), true)]
fn the_reveal_is_asked_only_without_a_dragon_in_play(
    #[values(2, 3, 4)] seats: usize,
    #[case] in_play: bool,
    #[case] in_hand: bool,
    #[case] reveals: Option<bool>,
    #[case] tapped_after: bool,
) {
    let mut hand = vec![temple()];
    if in_hand {
        hand.push(dragon());
    }
    let mut scenario = Scenario::new(seats).hand(0, hand);
    if in_play {
        scenario = scenario.battlefield(0, [ready(dragon())]);
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.play_land("Temple").unwrap();
    for _ in 0..5 {
        let decision = driver.decision().unwrap().clone();
        let answer = match &decision.kind {
            DecisionKind::YesNo(_) => {
                assert!(reveals.is_some(), "no question was expected");
                Answer::YesNo(YesNoChoice(reveals.unwrap_or(false)))
            }
            DecisionKind::ChooseCards(options) => {
                Answer::ChooseCards(CardsChoice(options.from.iter().copied().take(1).collect()))
            }
            _ => break,
        };
        driver.answer(answer).unwrap();
    }
    let id = driver.id_of("Temple").unwrap();
    let tapped = driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .tapped();
    assert_eq!(tapped, tapped_after);
}
