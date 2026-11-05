//! Goal: the cost parts Draw and Discard at random are offered exactly when the payer can pay them (the cards are in the library or the hand), pay with the events of the rules (a draw, a logged random pick followed by a discard) and never leave a game stuck on a cost that was offered.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameState};
use mtg_testkit::cards::free_creature;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use common::{names, pid};

/// An artifact with the ability "<cost>: You gain 1 life."
fn gadget(cost: &str) -> Arc<CardDefinition> {
    let ability =
        format!(r#"{{"Activated":{{"cost":[{cost}],"effects":[{{"GainLife":{{"amount":1}}}}]}}}}"#);
    Arc::new(
        CardDefinition::new("Gadget")
            .with_type(CardType::Artifact)
            .with_rules(serde_json::from_str::<Ability>(&ability).unwrap()),
    )
}

fn cards(prefix: &str, count: usize) -> Vec<Arc<CardDefinition>> {
    (0..count)
        .map(|at| free_creature(&format!("{prefix} {at}"), 1, 1))
        .collect()
}

fn zone(state: &GameState, zone: Zone) -> Vec<String> {
    names(state, zone)
}

fn offered(driver: &Driver) -> bool {
    !driver.offered_abilities("Gadget").unwrap().is_empty()
}

fn settle(driver: &mut Driver) -> Result<Vec<Event>, ScriptError> {
    let mut log = Vec::new();
    for _ in 0..50 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(log);
        }
        log.extend(driver.answer_passively()?);
    }
    Err(ScriptError::Stuck("the stack".into()))
}

#[rstest]
#[case::draw_with_cards("Draw", 1, 3, 0, true)]
#[case::draw_the_last_card("Draw", 2, 2, 0, true)]
#[case::draw_one_short("Draw", 2, 1, 0, false)]
#[case::draw_from_an_empty_library("Draw", 1, 0, 2, false)]
#[case::discard_with_cards("DiscardRandom", 1, 0, 3, true)]
#[case::discard_the_whole_hand("DiscardRandom", 2, 5, 2, true)]
#[case::discard_one_short("DiscardRandom", 3, 5, 2, false)]
#[case::discard_from_an_empty_hand("DiscardRandom", 1, 5, 0, false)]
fn a_new_cost_part_is_offered_exactly_when_it_can_be_paid(
    #[case] part: &str,
    #[case] amount: u32,
    #[case] library: usize,
    #[case] hand: usize,
    #[case] pays: bool,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(2)
        .battlefield(0, [ready(gadget(&format!(r#"{{"{part}":{amount}}}"#)))])
        .library(0, cards("Library", library))
        .hand(0, cards("Held", hand));
    let mut driver = Driver::from_scenario(scenario)?;
    assert_eq!(offered(&driver), pays, "castable agrees with payment");
    if !pays {
        assert!(
            driver.begin_activation("Gadget").is_err(),
            "an unpayable cost is refused, never half paid"
        );
        return Ok(());
    }
    let hand_before = zone(driver.state(), Zone::Hand(pid(0))).len();
    let log = driver.begin_activation("Gadget")?;
    let mut log = log;
    log.extend(settle(&mut driver)?);
    let hand_after = zone(driver.state(), Zone::Hand(pid(0))).len();
    let count = usize::try_from(amount).unwrap();
    if part == "Draw" {
        assert_eq!(hand_after, hand_before + count);
        assert_eq!(zone(driver.state(), Zone::Library(pid(0))).len(), library - count);
    } else {
        assert_eq!(hand_after, hand_before - count);
        assert_eq!(zone(driver.state(), Zone::Graveyard(pid(0))).len(), count);
        let pick = log
            .iter()
            .position(|event| matches!(event, Event::ChosenAtRandom { .. }))
            .expect("the random pick is logged");
        let first = log
            .iter()
            .position(|event| matches!(event, Event::CardDiscarded { .. }))
            .expect("the discard follows");
        assert!(pick < first, "the pick comes before the discards");
    }
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), 21);
    Ok(())
}
