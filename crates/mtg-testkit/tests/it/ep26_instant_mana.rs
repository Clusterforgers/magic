//! Goal: a mana ability activated only as an instant with "Discard your hand" in its cost (Lion's Eye Diamond) is offered at priority whatever the hand holds, discards the whole hand and sacrifices the source as it adds three mana of the color picked, and is never a source of a payment: a spell it alone could pay for is not castable; the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::copying::replays;
use crate::common::pid;
use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::mana::{ManaCost, ManaType};
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const DIAMOND: &str = r#"{"Activated":{"cost":["SacrificeSelf",{"Discard":{"count":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]}}}],"effects":[{"AddMana":{"mana":{"AnyColor":3}}}],"timing":"AsInstant"}}"#;

fn diamond() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Diamond")
            .with_type(CardType::Artifact)
            .with_rules(serde_json::from_str(DIAMOND).unwrap()),
    )
}

fn sorcery(name: &str, cost: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Sorcery)
            .with_mana_cost(ManaCost::parse(cost).unwrap()),
    )
}

fn scenario(hand: usize) -> Scenario {
    let cards: Vec<_> = (0..hand)
        .map(|n| sorcery(&format!("Card {n}"), "{R}"))
        .collect();
    Scenario::new(2)
        .at(1, 0, Step::Main1)
        .hand(0, cards)
        .battlefield(0, vec![ready(diamond())])
}

fn castable(driver: &Driver) -> usize {
    match driver.decision().map(|d| &d.kind) {
        Ok(DecisionKind::Priority(options)) => options.castable.len(),
        _ => 0,
    }
}

#[rstest]
fn at_priority_it_discards_the_hand_and_adds_three(
    #[values(0, 1, 3)] hand: usize,
    #[values(ManaType::Red, ManaType::Blue)] color: ManaType,
) -> Result<(), ScriptError> {
    let setup = scenario(hand).setup().unwrap();
    let mut driver = Driver::from_scenario(scenario(hand))?;
    assert_eq!(castable(&driver), 0, "the diamond pays for no spell");
    driver.tap_for_mana_of("Diamond", color)?;
    let pool = driver.state().player(pid(0)).unwrap().mana().clone();
    assert_eq!((pool.get(color), pool.total()), (3, 3));
    assert!(driver.state().zone(Zone::Hand(pid(0))).is_empty());
    assert_eq!(
        driver.state().zone(Zone::Graveyard(pid(0))).len(),
        hand + 1,
        "the hand and the diamond"
    );
    replays(&driver, &setup);
    Ok(())
}
