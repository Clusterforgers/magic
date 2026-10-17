//! Goal: snapshots mirror the public table exactly, events follow the hiding rules event by event,
//! and a snapshot's size tracks what the viewer can see rather than what is hidden.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::collections::BTreeSet;

use common::{Step, run_game, unique_setup};
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::characteristics::characteristics;
use mtg_engine::effects::{Duration, Modification, Selector};
use mtg_engine::turn::Step as TurnStep;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameState};
use mtg_protocol::{EventView, HiddenEvent, StateView, project_events, zone_is_visible};
use mtg_testkit::cards::{bear, elf, forest, giant, mountain, plains, swamp};
use mtg_testkit::{Driver, Scenario, damaged, ready, tapped};
use rstest::rstest;
use serde_json::Value;

/// What a viewer should see of one event, stated independently of the implementation.
#[derive(Debug, PartialEq)]
enum Expect {
    Public,
    Hidden(HiddenEvent),
}

fn count(len: usize) -> u32 {
    u32::try_from(len).unwrap()
}

fn expected(before: &GameState, event: &Event, viewer: PlayerId) -> Expect {
    match event {
        Event::LibraryShuffled { player, .. } => {
            Expect::Hidden(HiddenEvent::LibraryShuffled { player: *player })
        }
        Event::MulliganTaken { player, .. } => {
            Expect::Hidden(HiddenEvent::MulliganTaken { player: *player })
        }
        Event::HandDealt { player, cards } if *player != viewer => {
            Expect::Hidden(HiddenEvent::HandDealt {
                player: *player,
                count: count(cards.len()),
            })
        }
        Event::CardsBottomed { player, cards } if *player != viewer => {
            Expect::Hidden(HiddenEvent::CardsBottomed {
                player: *player,
                count: count(cards.len()),
            })
        }
        Event::CardDrawn { player, .. } if *player != viewer => {
            Expect::Hidden(HiddenEvent::CardDrawn { player: *player })
        }
        Event::ObjectLeftGame { object } => {
            let zone = before
                .object(*object)
                .map(mtg_engine::object::GameObject::zone);
            match zone {
                Some(Zone::Library(_)) => Expect::Hidden(HiddenEvent::ObjectLeftGame),
                Some(Zone::Hand(owner)) if owner != viewer => {
                    Expect::Hidden(HiddenEvent::ObjectLeftGame)
                }
                _ => Expect::Public,
            }
        }
        _ => Expect::Public,
    }
}

#[rstest]
#[case(2, 21)]
#[case(3, 22)]
#[case(4, 23)]
fn events_are_projected_by_the_hiding_rules(#[case] players: usize, #[case] seed: u64) {
    let viewers: Vec<PlayerId> = (0..players).map(PlayerId::from_index).collect();
    let mut hidden_seen = 0usize;
    let mut public_seen = 0usize;
    let mut left_hidden = 0usize;
    run_game(unique_setup(players, seed), seed, |step: &Step| {
        let batches = project_events(step.before, step.events, &viewers, step.from_seq).unwrap();
        let mut state = step.before.clone();
        for (i, event) in step.events.iter().enumerate() {
            for (viewer, batch) in viewers.iter().zip(&batches) {
                assert_eq!(batch.from_seq, step.from_seq);
                assert_eq!(batch.events.len(), step.events.len(), "one view per event");
                match (expected(&state, event, *viewer), &batch.events[i]) {
                    (Expect::Public, EventView::Public(shown)) => {
                        assert_eq!(shown, event);
                        public_seen += 1;
                    }
                    (Expect::Hidden(want), EventView::Hidden(got)) => {
                        assert_eq!(&want, got);
                        hidden_seen += 1;
                        left_hidden += usize::from(matches!(want, HiddenEvent::ObjectLeftGame));
                    }
                    (want, got) => {
                        panic!("viewer {viewer:?} event {event:?}: wanted {want:?}, got {got:?}")
                    }
                }
            }
            mtg_engine::apply_event(&mut state, event).unwrap();
        }
    });
    assert!(
        public_seen > 300 && hidden_seen > 30,
        "test must not be vacuous"
    );
    assert!(
        players == 2 || left_hidden > 0,
        "eliminated players' hidden cards were never projected"
    );
}

fn collect_keys(value: &Value, out: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            for (key, inner) in map {
                out.insert(key.clone());
                collect_keys(inner, out);
            }
        }
        Value::Array(items) => items.iter().for_each(|v| collect_keys(v, out)),
        _ => {}
    }
}

#[rstest]
#[case(2, 31)]
#[case(4, 32)]
fn snapshots_carry_no_engine_internals(#[case] players: usize, #[case] seed: u64) {
    let engine = run_game(unique_setup(players, seed), seed, |_| {});
    let view = StateView::for_player(engine.state(), PlayerId(0));
    let mut keys = BTreeSet::new();
    collect_keys(&serde_json::to_value(&view).unwrap(), &mut keys);
    for forbidden in [
        "seed",
        "rng_index",
        "order",
        "library_order",
        "next_timestamp",
    ] {
        assert!(!keys.contains(forbidden), "snapshot exposes {forbidden}");
    }
}

fn table() -> Driver {
    let scenario = Scenario::new(2)
        .hand(0, [bear(), elf()])
        .hand(1, [giant(), swamp(), mountain()])
        .battlefield(
            0,
            [
                ready(forest()),
                ready(forest()),
                tapped(plains()),
                damaged(giant(), 1),
            ],
        )
        .battlefield(1, [ready(swamp())])
        .graveyard(1, [giant()])
        .life(1, 13)
        .at(1, 0, TurnStep::Main1);
    Driver::from_scenario(scenario).unwrap()
}

#[test]
fn snapshot_mirrors_the_public_table_and_derives_characteristics() {
    let mut driver = table();
    let buffed_id = driver.id_of("Craw Wurm").unwrap();
    driver
        .add_effect(
            Selector::Object(buffed_id),
            Modification::ModifyPowerToughness {
                power: 2,
                toughness: 2,
            },
            Duration::UntilEndOfTurn,
        )
        .unwrap();
    driver.cast("Grizzly Bears").unwrap();
    let state = driver.state();

    for viewer in [PlayerId(0), PlayerId(1)] {
        let view = StateView::for_player(state, viewer);
        assert_eq!(view.viewer, viewer);
        assert_eq!(view.turn.active, PlayerId(0));
        assert_eq!(view.turn.step, TurnStep::Main1);
        assert_eq!(view.players[1].life, 13);
        assert_eq!(
            view.battlefield.cards.as_deref(),
            Some(state.zone(Zone::Battlefield))
        );
        assert_eq!(view.stack.cards.as_deref(), Some(state.stack()));
        assert_eq!(view.stack.count, 1, "the cast bear is on the stack");
        assert_eq!(
            view.players[1].graveyard.cards.as_deref(),
            Some(state.zone(Zone::Graveyard(PlayerId(1)))),
            "graveyards are public"
        );
        // Hands: owner sees cards, everyone sees sizes.
        assert_eq!(view.players[0].hand.count, 1);
        assert_eq!(view.players[1].hand.count, 3);
        assert_eq!(view.players[0].hand.cards.is_some(), viewer == PlayerId(0));
        assert_eq!(view.players[1].hand.cards.is_some(), viewer == PlayerId(1));

        let shown: BTreeSet<ObjectId> = view.objects.iter().map(|o| o.id).collect();
        for object in state.objects() {
            let visible = zone_is_visible(object.zone(), viewer);
            assert_eq!(
                shown.contains(&object.id()),
                visible,
                "{:?}",
                object.card().name
            );
        }
        for object in &view.objects {
            let chars = characteristics(state, object.id).unwrap();
            assert_eq!(object.power, chars.power);
            assert_eq!(object.toughness, chars.toughness);
            assert_eq!(
                object.types.contains(&CardType::Creature),
                chars.is_creature()
            );
            assert_eq!(object.permanent.is_some(), object.zone == Zone::Battlefield);
        }
    }

    let view = StateView::for_player(state, PlayerId(1));
    let buffed = view.objects.iter().find(|o| o.id == buffed_id).unwrap();
    assert_eq!((buffed.power, buffed.toughness), (Some(6), Some(6)));
    let permanent = buffed.permanent.as_ref().unwrap();
    assert_eq!(permanent.damage, 1);
    assert!(!permanent.tapped);
    let tapped_land = view.objects.iter().find(|o| o.name == "Plains").unwrap();
    assert!(tapped_land.permanent.as_ref().unwrap().tapped);
}

#[test]
fn hidden_cards_do_not_change_a_snapshots_size() {
    let build = |extra_library: usize, extra_hand: usize| {
        let driver = Driver::from_scenario(
            Scenario::new(2)
                .battlefield(0, [ready(forest()), ready(elf())])
                .library(1, vec![forest(); 20 + extra_library])
                .hand(1, vec![giant(); 3 + extra_hand]),
        )
        .unwrap();
        let view = StateView::for_player(driver.state(), PlayerId(0));
        serde_json::to_string(&view).unwrap().len()
    };
    let small = build(0, 0);
    let large = build(300, 40);
    // Counts gain a few digits; no card is added.
    assert!(large.abs_diff(small) < 16, "{small} vs {large}");
}

#[test]
fn four_player_board_snapshot_stays_small() {
    let mut scenario = Scenario::new(4).at(5, 0, TurnStep::Main1);
    for seat in 0..4 {
        let permanents = (0..16)
            .map(|_| ready(forest()))
            .chain((0..4).map(|_| tapped(mountain())))
            .chain((0..12).map(|i| {
                if i % 2 == 0 {
                    ready(bear())
                } else {
                    damaged(giant(), 2)
                }
            }));
        scenario = scenario
            .battlefield(seat, permanents)
            .hand(seat, vec![elf(); 7])
            .graveyard(seat, vec![bear(); 10])
            .library(seat, vec![forest(); 40]);
    }
    let driver = Driver::from_scenario(scenario).unwrap();
    let objects = driver.state().object_count();
    assert_eq!(objects, 4 * (32 + 7 + 10 + 40));

    for viewer in (0..4).map(PlayerId::from_index) {
        let view = StateView::for_player(driver.state(), viewer);
        // Visible: 4 boards of 32, 4 graveyards of 10, and the viewer's own hand of 7.
        assert_eq!(view.objects.len(), 4 * 32 + 4 * 10 + 7);
        let bytes = serde_json::to_string(&view).unwrap().len();
        let per_object = bytes / view.objects.len();
        assert!(bytes < 64 * 1024, "snapshot is {bytes} bytes");
        assert!(per_object < 400, "{per_object} bytes per object");
    }
}
