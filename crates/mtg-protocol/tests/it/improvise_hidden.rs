//! Goal: improvise and typecycling disclose what the table sees and nothing more, at two to four seats and to a spectator. The artifacts tapped for improvise are tapped in the open for every seat, as creatures tapped for convoke are, with the spell named; no card of any hand is named. A typecycling shows every seat the card discarded and the card it reveals, and no seat but the searcher reads a card of the library that stayed there, a card that did not match is read by nobody, and no hand card is named.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameState};
use mtg_protocol::{EventBatch, EventView, project_events};
use mtg_testkit::cards::{forest, island, mountain, wastes};
use mtg_testkit::corpus::card as corpus_card;
use mtg_testkit::{Driver, Scenario, improvise_card, ready};
use rstest::rstest;
use serde_json::Value;

use crate::hidden_info::{assert_clean, hidden_across};

fn seat(index: usize) -> PlayerId {
    PlayerId::from_index(index)
}

/// Whether the text of `value` holds `name` anywhere.
fn names(value: &Value, name: &str) -> bool {
    match value {
        Value::String(text) => text == name,
        Value::Array(items) => items.iter().any(|item| names(item, name)),
        Value::Object(map) => map.values().any(|item| names(item, name)),
        _ => false,
    }
}

/// Whether any part of the batch holds the card's name.
fn batch_names(batch: &EventBatch, name: &str) -> bool {
    [
        serde_json::to_value(&batch.events).unwrap(),
        serde_json::to_value(&batch.table).unwrap(),
        serde_json::to_value(&batch.objects).unwrap(),
        serde_json::to_value(&batch.gone).unwrap(),
    ]
    .iter()
    .any(|part| names(part, name))
}

/// The id of the first card called `name` in `zone`.
fn card_in(state: &GameState, zone: Zone, name: &str) -> ObjectId {
    state
        .zone(zone)
        .iter()
        .copied()
        .find(|id| state.object(*id).unwrap().card().name == name)
        .unwrap()
}

/// Seat 0 holds `hand` (and a card that stays hidden), `library` and `battlefield`; every other seat has a hand of its own.
fn table(
    seats: usize,
    hand: &[&str],
    library: Vec<std::sync::Arc<mtg_cards::CardDefinition>>,
) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .library(0, library)
        .hand_named(0, hand.iter().copied().chain(["Counterspell"]));
    let others = [
        ["Wrath of God", "Doom Blade"],
        ["Fireball", "Giant Growth"],
        ["Opt", "Brainstorm"],
    ];
    for other in 1..seats {
        scenario = scenario
            .library(other, vec![forest(); 8])
            .hand_named(other, others[other - 1]);
    }
    scenario
}

#[rstest]
fn artifacts_tapped_for_improvise_are_shown_to_every_seat_and_no_hand_card_is(
    #[values(2, 3, 4)] seats: usize,
) {
    let artifacts = ["Trinket", "Gadget", "Gizmo"].map(|name| ready(improvise_card(name).unwrap()));
    let scenario = table(seats, &["Reverse Engineer"], vec![forest(); 8]).battlefield(
        0,
        [ready(island()), ready(island())]
            .into_iter()
            .chain(artifacts),
    );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let before = driver.state().clone();
    let logged = driver.engine().events().len();
    driver.begin_cast("Reverse Engineer").unwrap();
    driver
        .pay_helped_by(
            &["Island", "Island"],
            &[],
            &["Trinket", "Gadget", "Gizmo"],
            &[],
            0,
        )
        .unwrap();
    let after = driver.state().clone();
    let tapped: Vec<ObjectId> = ["Trinket", "Gadget", "Gizmo"]
        .iter()
        .map(|name| card_in(&after, Zone::Battlefield, name))
        .collect();
    let events = driver.engine().events()[logged..].to_vec();
    let all: Vec<PlayerId> = (0..=seats).map(seat).collect();
    let batches = project_events(&before, &events, &all, u64::try_from(logged).unwrap()).unwrap();
    let spell = card_in(&after, Zone::Stack, "Reverse Engineer");
    for (viewer, batch) in all.iter().zip(&batches) {
        for artifact in &tapped {
            assert!(
                batch.events.iter().any(|view| matches!(
                    view,
                    EventView::Public(Event::PermanentTapped { object }) if object == artifact
                )),
                "{viewer:?} sees {artifact:?} tapped for improvise"
            );
        }
        assert!(
            batch_names(batch, "Reverse Engineer"),
            "{viewer:?} reads the spell"
        );
        let mut hidden = hidden_across(&before, &after, *viewer);
        hidden.remove(&spell);
        assert_clean(batch, &hidden);
        for name in ["Counterspell", "Wrath of God", "Fireball", "Opt"] {
            assert!(!batch_names(batch, name), "{viewer:?} reads {name}");
        }
    }
}

/// Passes priority until the stack is empty and answers the search with the card called `pick`.
fn resolve(driver: &mut Driver, pick: &str) {
    for _ in 0..50 {
        if let DecisionKind::ChooseCards(options) = &driver.decision().unwrap().kind {
            let found: Vec<ObjectId> = options
                .from
                .iter()
                .copied()
                .filter(|id| driver.state().object(*id).unwrap().card().name == pick)
                .take(1)
                .collect();
            driver
                .answer(Answer::ChooseCards(CardsChoice(found)))
                .unwrap();
        } else if driver.state().stack().is_empty() {
            return;
        } else {
            driver.pass().unwrap();
        }
    }
    panic!("the stack did not empty");
}

#[rstest]
fn a_typecycling_shows_the_discard_and_the_card_revealed_and_reads_no_other_card(
    #[values(2, 3, 4)] seats: usize,
) {
    // "Sol Ring", "Opt" and the Mountain are no basic Forest or Island: the Mountain matches basic landcycling, the others do not.
    let library = vec![
        forest(),
        island(),
        mountain(),
        corpus_card("Sol Ring").unwrap(),
        corpus_card("Serra Angel").unwrap(),
    ];
    let scenario = table(seats, &["Ash Barrens"], library).battlefield(0, [ready(wastes())]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let before = driver.state().clone();
    let logged = driver.engine().events().len();
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        panic!("priority was expected");
    };
    let ability = options.abilities.first().copied().unwrap();
    driver
        .answer(Answer::Priority(PriorityAction::Activate(ability)))
        .unwrap();
    driver.pay_tapping(&["Wastes"], 0).unwrap();
    resolve(&mut driver, "Island");
    let after = driver.state().clone();
    let barrens = card_in(&after, Zone::Graveyard(seat(0)), "Ash Barrens");
    let found = card_in(&after, Zone::Hand(seat(0)), "Island");
    let events = driver.engine().events()[logged..].to_vec();
    let all: Vec<PlayerId> = (0..=seats).map(seat).collect();
    let batches = project_events(&before, &events, &all, u64::try_from(logged).unwrap()).unwrap();
    for (viewer, batch) in all.iter().zip(&batches) {
        assert!(
            batch.events.iter().any(|view| matches!(
                view,
                EventView::Public(Event::Cycled { object, .. }) if *object == barrens
            )),
            "{viewer:?} sees the card cycled"
        );
        assert!(
            batch_names(batch, "Ash Barrens"),
            "{viewer:?} reads the card cycled"
        );
        assert!(
            batch.events.iter().any(|view| matches!(
                view,
                EventView::Public(Event::Revealed(revealed)) if revealed.cards.contains(&found)
            )),
            "{viewer:?} sees the card found revealed"
        );
        // The card goes on to the hand within the resolution, so a seat but its owner loses sight of it, as with any
        // other card revealed on its way to a hand.
        let in_view = batch.objects.iter().any(|object| object.id == found);
        assert_eq!(in_view, *viewer == seat(0), "{viewer:?} and the card found");
        let mut hidden = hidden_across(&before, &after, *viewer);
        hidden.remove(&barrens);
        hidden.remove(&found);
        if *viewer == seat(0) {
            // The searcher alone is shown the basic lands that match.
            for name in ["Forest", "Island", "Mountain"] {
                for id in before.zone(Zone::Library(seat(0))) {
                    if before.object(*id).unwrap().card().name == name {
                        hidden.remove(id);
                    }
                }
            }
        }
        assert_clean(batch, &hidden);
        for name in [
            "Counterspell",
            "Wrath of God",
            "Fireball",
            "Opt",
            "Sol Ring",
            "Serra Angel",
        ] {
            assert!(!batch_names(batch, name), "{viewer:?} reads {name}");
        }
        if *viewer != seat(0) {
            // The land that matched and stayed in the library is read by the searcher alone.
            assert!(!batch_names(batch, "Forest"), "{viewer:?} reads the Forest");
        }
    }
}
