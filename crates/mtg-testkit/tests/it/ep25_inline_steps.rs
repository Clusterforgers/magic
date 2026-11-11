//! Goal: the steps of an `Instead` replacement read the state the steps before them left (CR 614.1): "draw two cards, then discard a card" in place of damage offers the cards just drawn to the discard, whatever the hand held before, and the log replays to the live state; at 2 to 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::decision::DecisionKind;
use mtg_engine::replay;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, burn, island, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{names, pid};

/// "If a source would deal damage to you, instead draw two cards, then discard a card."
const LOOT_INSTEAD: &str = r#"{"Replacement":{"when":{"Damage":{"to":"You"}},"by":{"Instead":[{"Draw":{"count":2}},{"Discard":{}}]}}}"#;

#[rstest]
fn a_step_after_a_draw_sees_the_cards_drawn(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] held: usize,
) -> Result<(), ScriptError> {
    let rule: Ability = serde_json::from_str(LOOT_INSTEAD).unwrap();
    let rule = Arc::new(
        CardDefinition::new("Rule")
            .with_type(CardType::Enchantment)
            .with_rules(rule),
    );
    let mut hand = vec![burn("Bolt", 3)];
    hand.extend((0..held).map(|_| bear()));
    let scenario = Scenario::new(seats)
        .battlefield(0, [ready(rule), ready(mountain())])
        .library(0, (0..4).map(|_| island()))
        .hand(0, hand);
    let mut driver = Driver::from_scenario(scenario)?;
    let me = driver.target_player(0);
    driver.cast_at("Bolt", &[me])?;
    let mut discards = 0;
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::Priority(_)) && driver.state().stack().is_empty() {
            break;
        }
        if let DecisionKind::ChooseCards(options) = &decision.kind {
            discards += 1;
            // The cards drawn are offered beside what the hand held.
            assert_eq!(options.from.len(), held + 2);
        }
        driver.answer(Driver::passive_answer(&decision))?;
    }
    driver.expect_life(0, 20)?;
    assert_eq!(names(driver.state(), Zone::Hand(pid(0))).len(), held + 1);
    // One card to the graveyard beside the Bolt.
    assert_eq!(names(driver.state(), Zone::Graveyard(pid(0))).len(), 2);
    assert_eq!(discards, 1);
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
