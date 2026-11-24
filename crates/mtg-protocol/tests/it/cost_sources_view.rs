//! Goal: the preview's modifiers of a card waiting in its owner's hand name each cost change by source (a tax of any
//! seat's permanent, a reduction of one's own), at tables of two to four seats, and nothing for a permanent or while no
//! cost changes; a player's effects that a permanent brings name it in `PlayerView.sources` for every viewer, and an
//! effect no object brings names none.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_engine::effects::Duration;
use mtg_engine::rules::{PlayerEffect, Rule};
use mtg_protocol::{ModifierView, ObjectView, PlayerEffectSourceView, StateView};
use mtg_testkit::cards::{creature, land};
use mtg_testkit::cost_cards::{helm, sphere};
use mtg_testkit::trigger_cards::creature_with;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// Seat 0 holds a Bear and controls `own`; the last seat controls `foe`.
fn table(seats: usize, own: &[Arc<CardDefinition>], foe: &[Arc<CardDefinition>]) -> Driver {
    let mut scenario = Scenario::new(seats)
        .hand(0, [creature("Bear", 1, &[Color::Green], 2, 2)])
        .battlefield(0, own.iter().cloned().map(ready).collect::<Vec<_>>())
        .battlefield(
            seats - 1,
            foe.iter().cloned().map(ready).collect::<Vec<_>>(),
        );
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| land(Color::Green)));
    }
    Driver::from_scenario(scenario).unwrap()
}

fn seen(driver: &Driver, viewer: usize, name: &str) -> Option<ObjectView> {
    StateView::for_player(driver.state(), PlayerId::from_index(viewer))
        .objects
        .into_iter()
        .find(|object| object.name == name)
}

/// The modifier lines of `view` as (effect, source, whether it points at an object).
fn lines(view: &ObjectView) -> Vec<(String, String, bool)> {
    view.modifiers
        .iter()
        .map(
            |ModifierView {
                 effect,
                 source,
                 sources,
                 ..
             }| { (effect.clone(), source.clone(), !sources.is_empty()) },
        )
        .collect()
}

#[rstest]
fn a_card_in_hand_names_each_cost_change_by_source(#[values(2, 3, 4)] seats: usize) {
    let driver = table(seats, &[helm().unwrap()], &[sphere().unwrap()]);
    let bear = seen(&driver, 0, "Bear").unwrap();
    let mut got = lines(&bear);
    got.sort();
    assert_eq!(
        got,
        vec![
            ("Costs {1} less".to_owned(), "Helm".to_owned(), true),
            ("Costs {1} more".to_owned(), "Sphere".to_owned(), true),
        ]
    );
    // The permanents that change the costs change none of their own.
    let sphere = seen(&driver, 0, "Sphere").unwrap();
    assert_eq!(sphere.modifiers, Vec::<ModifierView>::new());
}

#[test]
fn a_card_whose_cost_no_one_changes_has_no_lines() {
    let driver = table(2, &[], &[]);
    assert_eq!(
        seen(&driver, 0, "Bear").unwrap().modifiers,
        Vec::<ModifierView>::new()
    );
}

/// A creature whose rule keeps its controller's opponents from gaining life.
fn vortex() -> Arc<CardDefinition> {
    let rule: Ability =
        serde_json::from_str(r#"{"Rule":{"rule":{"CantGainLife":"Opponents"}}}"#).unwrap();
    creature_with("Vortex", vec![rule])
}

#[rstest]
fn a_permanent_is_named_as_the_source_of_a_players_effect(#[values(2, 3, 4)] seats: usize) {
    let driver = table(seats, &[vortex()], &[]);
    let id = driver.id_of("Vortex").unwrap();
    for viewer in 0..=seats {
        let view = StateView::for_player(driver.state(), PlayerId::from_index(viewer));
        for player in &view.table.players {
            let want = if player.seat == PlayerId(0) {
                Vec::new()
            } else {
                vec![PlayerEffectSourceView {
                    effect: PlayerEffect::CantGainLife,
                    source: "Vortex".to_owned(),
                    id: Some(id),
                }]
            };
            assert_eq!(
                player.sources, want,
                "viewer {viewer}, seat {:?}",
                player.seat
            );
        }
    }
}

#[test]
fn an_effect_no_object_brings_names_no_source() {
    let mut driver = table(2, &[], &[]);
    driver
        .add_rule(Rule::CantGainLife(vec![PlayerId(1)]), Duration::Forever)
        .unwrap();
    let view = StateView::for_player(driver.state(), PlayerId(0));
    let foe = &view.table.players[1];
    assert_eq!(foe.effects, vec![PlayerEffect::CantGainLife]);
    assert_eq!(foe.sources, Vec::<PlayerEffectSourceView>::new());
    let wire = serde_json::to_value(&view).unwrap();
    assert!(wire["players"][1].get("sources").is_none());
}
