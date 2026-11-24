//! Goal: where a rule draws the colors of a hero's costs anew (the Delves uber boss), the view of each card in their
//! hand and command zone carries the cost they pay now beside the printed one, at tables of two to four seats; the
//! field is left out for a card whose cost stays, for the boss's own cards and for a permanent, so other games keep
//! their wire form. Every seat sees on each hero's view which color their costs stand for this round.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_engine::format::Commander;
use mtg_engine::rules::PlayerEffect;
use mtg_engine::w6b_emblems::emblem_definition;
use mtg_protocol::{ObjectView, StateView};
use mtg_testkit::cards::{creature, land};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// The boss's emblem: the costs of its opponents' spells have their colors drawn anew.
fn unmaker() -> Arc<CardDefinition> {
    let rule: Ability =
        serde_json::from_str(r#"{"Rule":{"rule":{"CostColorsDrawn":"Opponents"}}}"#).unwrap();
    Arc::new(emblem_definition("The Unmaker", &[rule]))
}

/// The heroes (every seat but the last) lead Azorius commanders and hold a white-heavy spell, with an Azorius Bear on
/// the battlefield; the boss holds a black spell and, with `rule`, the emblem.
fn driver(seats: usize, rule: bool) -> Driver {
    let boss = seats - 1;
    let mut scenario = Scenario::new(seats)
        .format(Arc::new(Commander))
        .hand(
            boss,
            [creature(
                "Boss Spell",
                1,
                &[Color::Black, Color::Black, Color::Red],
                2,
                2,
            )],
        )
        .library(boss, (0..20).map(|_| land(Color::Black)));
    if rule {
        scenario = scenario.emblems(boss, [unmaker()]);
    }
    for seat in 0..boss {
        let azorius = [Color::White, Color::Blue];
        scenario = scenario
            .commanders(
                seat,
                [creature(
                    &format!("Leader {seat}"),
                    1,
                    &[Color::White, Color::White, Color::Blue],
                    2,
                    2,
                )],
            )
            .hand(
                seat,
                [creature(
                    &format!("Spell {seat}"),
                    1,
                    &[Color::White, Color::White, Color::Blue],
                    2,
                    2,
                )],
            )
            .battlefield(
                seat,
                [ready(creature(&format!("Bear {seat}"), 0, &azorius, 2, 2))],
            )
            .library(seat, (0..20).map(|_| land(Color::White)));
    }
    Driver::from_scenario(scenario).unwrap()
}

fn seen(driver: &Driver, viewer: usize, name: &str) -> ObjectView {
    StateView::for_player(driver.state(), PlayerId::from_index(viewer))
        .objects
        .into_iter()
        .find(|o| o.name == name)
        .unwrap()
}

#[rstest]
fn a_hero_sees_the_cost_they_pay_now(#[values(2, 3, 4)] seats: usize) {
    let driver = driver(seats, true);
    for seat in 0..seats - 1 {
        for name in [format!("Spell {seat}"), format!("Leader {seat}")] {
            let view = seen(&driver, seat, &name);
            let printed = view.mana_cost.clone().unwrap();
            let now = view
                .cost_now
                .clone()
                .unwrap_or_else(|| panic!("{name}: a drawn cost"));
            // An Azorius identity has one derangement: white and blue trade places.
            assert_eq!(now.to_string(), "{1}{W}{U}{U}", "{name} was {printed}");
            assert_eq!(now.mana_value(), printed.mana_value());
        }
        let bear = seen(&driver, seat, &format!("Bear {seat}"));
        assert_eq!(bear.cost_now, None, "a permanent shows its printed cost");
    }
    let boss = seen(&driver, seats - 1, "Boss Spell");
    assert_eq!(boss.cost_now, None, "the boss's own costs stay");
}

#[rstest]
fn a_hero_reads_whose_rule_draws_their_colors(#[values(2, 3, 4)] seats: usize) {
    let driver = driver(seats, true);
    for seat in 0..seats - 1 {
        let view = seen(&driver, seat, &format!("Spell {seat}"));
        let drawn: Vec<_> = view
            .modifiers
            .iter()
            .map(|line| (line.effect.as_str(), line.source.as_str()))
            .collect();
        assert_eq!(
            drawn,
            vec![("Costs {1}{W}{U}{U} this round", "The Unmaker")]
        );
        let state = StateView::for_player(driver.state(), PlayerId::from_index(seat));
        let sources = &state.table.players[seat].sources;
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].source, "The Unmaker");
    }
    let boss = seen(&driver, seats - 1, "Boss Spell");
    assert_eq!(boss.modifiers.len(), 0, "the boss's own costs stay");
}

#[test]
fn without_the_rule_the_field_is_left_out() {
    let driver = driver(3, false);
    let view = seen(&driver, 0, "Spell 0");
    assert_eq!(view.cost_now, None);
    let json = serde_json::to_value(&view).unwrap();
    assert!(json.get("cost_now").is_none());
    let state = StateView::for_player(driver.state(), PlayerId::from_index(0));
    assert!(
        state
            .table
            .players
            .iter()
            .all(|player| player.effects.is_empty())
    );
}

#[rstest]
fn every_seat_sees_what_each_heros_colors_stand_for(#[values(2, 3, 4)] seats: usize) {
    let driver = driver(seats, true);
    let drawn = PlayerEffect::CostColorsDrawn(vec![
        (Color::White, Color::Blue),
        (Color::Blue, Color::White),
    ]);
    for viewer in 0..seats {
        let view = StateView::for_player(driver.state(), PlayerId::from_index(viewer));
        for (seat, player) in view.table.players.iter().enumerate() {
            let shown = player.effects.contains(&drawn);
            assert_eq!(shown, seat < seats - 1, "seat {seat} seen by {viewer}");
        }
    }
}
