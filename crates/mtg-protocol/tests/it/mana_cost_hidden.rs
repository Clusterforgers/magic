//! Goal: a spell paid for with a Signet shows every seat the same public events, in the order the mana is made and spent (the land taps and adds, the Signet's cost is spent, the Signet taps and adds, the spell's cost is spent), and nothing of the cards that stay hidden: the rest of the caster's hand and every library.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::{ManaCost, ManaType};
use mtg_engine::{Event, GameState};
use mtg_protocol::{EventView, project_events};
use mtg_testkit::cards::{bear, forest};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::hidden_info::{assert_clean, hidden_in};

/// A spell that costs `{W}{U}`, which a Forest and a Signet pay together.
fn charm() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Charm")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_mana_cost(ManaCost::parse("{W}{U}").unwrap()),
    )
}

/// Seat 0 holds the spell and a second card, with a Forest and an Azorius Signet in play; every seat has a library.
fn table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .hand(0, [charm(), bear()])
        .battlefield(0, [ready(forest()), ready(card("Azorius Signet").unwrap())])
        .library_named(0, ["Lightning Bolt", "Giant Growth", "Counterspell"]);
    for seat in 1..seats {
        scenario = scenario.library_named(seat, ["Serra Angel", "Doom Blade", "Fireball"]);
    }
    scenario
}

/// The kinds of the mana events of a cast in the order they were logged, with the mana they name.
fn mana_events(events: &[Event]) -> Vec<String> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::PermanentTapped { .. } => Some("tap".to_owned()),
            Event::ManaAdded { mana, .. } => Some(format!("add {mana:?}")),
            Event::ManaSpent { mana, .. } => Some(format!("spend {mana:?}")),
            _ => None,
        })
        .collect()
}

#[rstest]
fn a_signet_pays_in_the_open_and_the_hidden_cards_stay_hidden(#[values(2, 3, 4)] seats: usize) {
    let scenario = table(seats);
    let before = GameState::from_setup(&scenario.clone().setup().unwrap()).unwrap();
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Charm").unwrap();
    let events = driver.engine().events().to_vec();
    let after = driver.state().clone();
    assert_eq!(
        mana_events(&events),
        [
            "tap",
            &format!("add {:?}", ManaType::Green),
            &format!("spend {:?}", [ManaType::Green]),
            "tap",
            &format!("add {:?}", ManaType::White),
            &format!("add {:?}", ManaType::Blue),
            &format!("spend {:?}", [ManaType::White, ManaType::Blue]),
        ],
        "the land first, the cost of the Signet from its mana, the Signet, then the spell's cost"
    );
    let viewers: Vec<PlayerId> = (0..seats).map(PlayerId::from_index).collect();
    let batches = project_events(&before, &events, &viewers, 0).unwrap();
    for (viewer, batch) in viewers.iter().zip(&batches) {
        let hidden: BTreeMap<ObjectId, String> = hidden_in(&after, *viewer);
        assert!(
            hidden.len() >= 3,
            "the libraries and the other hands are hidden from seat {}: {hidden:?}",
            viewer.index()
        );
        assert_clean(batch, &hidden);
        let public: Vec<&Event> = batch
            .events
            .iter()
            .filter_map(|view| match view {
                EventView::Public(event) => Some(event),
                EventView::Hidden(_) => None,
            })
            .collect();
        for event in events.iter().filter(|event| {
            matches!(
                event,
                Event::PermanentTapped { .. } | Event::ManaAdded { .. } | Event::ManaSpent { .. }
            )
        }) {
            assert!(public.contains(&event), "every seat sees {event:?}");
        }
    }
}
