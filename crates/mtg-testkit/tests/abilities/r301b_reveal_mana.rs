//! Goal: "{T}: Reveal any number of artifact cards in your hand. Add {C}{C} for each card revealed this way." offers only
//! the hand's cards of the kind, adds two mana for each card revealed (none for none) and leaves the cards in hand, at
//! tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, island};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Metalworker in the large.
fn metalworker() -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_value(serde_json::json!({"Activated": {
        "cost": ["Tap"],
        "effects": [
            {"Choose": {
                "from": {"Each": [{"In": "Hand"}, {"OwnedBy": "You"}, {"Type": "Artifact"}]},
                "count": {"min": 0, "max": null},
                "as": "revealed"}},
            {"Reveal": {"what": {"Bound": "revealed"}}},
            {"AddMana": {
                "mana": {"Fixed": ["Colorless", "Colorless"]},
                "times": {"CountOf": {"of": {"Bound": "revealed"}, "filter": []}}}}]}}))
    .unwrap();
    Arc::new(
        CardDefinition::new("Metalworker")
            .with_type(CardType::Artifact)
            .with_type(CardType::Creature)
            .with_power_toughness(1, 2)
            .with_rules(ability),
    )
}

fn trinket(name: &str) -> Arc<CardDefinition> {
    Arc::new(CardDefinition::new(name).with_type(CardType::Artifact))
}

#[rstest]
fn each_revealed_artifact_adds_two_mana(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] revealed: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, [ready(metalworker())])
        .hand(
            0,
            [
                trinket("Trinket A"),
                trinket("Trinket B"),
                free_creature("Bear", 2, 2),
            ],
        );
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..20).map(|_| island()));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_activation("Metalworker")?;
    let decision = driver.decision()?;
    let DecisionKind::ChooseCards(options) = &decision.kind else {
        panic!("a choice of cards, not {:?}", decision.kind);
    };
    let name = |id| {
        driver
            .state()
            .object(id)
            .map(|object| object.card().name.clone())
            .unwrap_or_default()
    };
    let mut offered: Vec<String> = options.from.iter().map(|id| name(*id)).collect();
    offered.sort_unstable();
    assert_eq!(
        offered,
        ["Trinket A", "Trinket B"],
        "only the artifact cards are offered"
    );
    let trinkets = options.from.clone();
    assert_eq!(options.min, 0);
    driver.answer(Answer::ChooseCards(CardsChoice(
        trinkets[..revealed].to_vec(),
    )))?;
    for _ in 0..20 {
        if !driver.state().stack().is_empty() {
            driver.answer_passively()?;
        }
    }
    let pool = driver.state().player(PlayerId(0)).unwrap().mana().total();
    assert_eq!(pool, 2 * u32::try_from(revealed).unwrap());
    for trinket in trinkets {
        assert_eq!(
            driver
                .state()
                .object(trinket)
                .map(mtg_engine::object::GameObject::zone),
            Some(Zone::Hand(PlayerId(0)))
        );
    }
    Ok(())
}
