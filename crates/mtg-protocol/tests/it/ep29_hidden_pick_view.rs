//! Goal: a card picked at random from a hand ("each opponent reveals a card at random from their hand") is played by the engine and told by the log only to the seats that see it: the owner of the hand gets the pick as logged, every other seat a hidden form that says how many were picked, and the reveal that follows names the card to all; at tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::turn::Step;
use mtg_engine::{Event, GameState};
use mtg_protocol::{EventView, HiddenEvent, project_events};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

const SPY: &str = r#"{"Activated":{"cost":[],"effects":[{"ChooseAtRandom":{"from":{"Each":[{"In":"Hand"},{"OwnedBy":"Opponents"}]},"as":"c"}},{"Reveal":{"what":{"Bound":"c"}}}]}}"#;

fn card(name: &str, rules: Option<&str>) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new(name).with_type(CardType::Artifact);
    if let Some(rules) = rules {
        card = card.with_rules(serde_json::from_str::<Ability>(rules).unwrap());
    }
    Arc::new(card)
}

#[rstest]
fn a_card_picked_from_a_hand_is_hidden_from_the_others(#[values(2, 3, 4)] seats: usize) {
    let scenario = Scenario::new(seats)
        .seed(7)
        .at(1, 0, Step::Main1)
        .battlefield(0, vec![ready(card("Spy", Some(SPY)))])
        .hand(1, [card("Secret", None), card("Other", None)]);
    let before = GameState::from_setup(&scenario.clone().setup().unwrap()).unwrap();
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_activation("Spy").unwrap();
    for _ in 0..seats * 3 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        driver.answer_passively().unwrap();
    }
    let events = driver.engine().events().to_vec();
    let picked: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            Event::ChosenAtRandom { cards } => Some(cards.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(picked.len(), 1, "one pick in the log");
    assert_eq!(picked[0].len(), 1, "one card picked");
    let viewers: Vec<PlayerId> = (0..seats).map(PlayerId::from_index).collect();
    let batches = project_events(&before, &events, &viewers, 0).unwrap();
    for (seat, batch) in batches.iter().enumerate() {
        let public = batch
            .events
            .iter()
            .filter(|event| matches!(event, EventView::Public(Event::ChosenAtRandom { .. })))
            .count();
        let hidden = batch
            .events
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    EventView::Hidden(HiddenEvent::PickedAtRandom { count: 1 })
                )
            })
            .count();
        let owner = seat == 1;
        assert_eq!(
            (public, hidden),
            if owner { (1, 0) } else { (0, 1) },
            "seat {seat}"
        );
        let revealed = batch
            .events
            .iter()
            .any(|event| matches!(event, EventView::Public(Event::Revealed(_))));
        assert!(revealed, "seat {seat} sees the reveal");
    }
}
