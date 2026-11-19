//! Goal: `ObjectView.forecast` rides on the top stack item only, is the same for every seat and a spectator at two to four seats, is left out of the JSON elsewhere, and never moves with a card nobody may see: games that differ in one hidden card give the same views.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::ids::PlayerId;
use mtg_engine::stack_forecast::{Change, Fate, ForecastChange, Forecasted};
use mtg_protocol::{ObjectView, StateView, spectator_viewer};
use mtg_testkit::cards::{bear, burn, giant, island, mountain, study};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// Seat 0 holds `spell` and lands; seat 1 a Bear and a Giant, and `hidden` in hand and library.
fn table(seats: usize, spell: Arc<CardDefinition>, hidden: Arc<CardDefinition>) -> Driver {
    Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [spell])
            .battlefield(0, (0..4).flat_map(|_| [ready(mountain()), ready(island())]))
            .battlefield(1, [ready(bear()), ready(giant())])
            .hand(1, [hidden.clone()])
            .library(1, [hidden, island()]),
    )
    .unwrap()
}

fn views(driver: &Driver) -> Vec<Vec<ObjectView>> {
    let state = driver.state();
    (0..state.players().len())
        .map(PlayerId::from_index)
        .chain([spectator_viewer(state)])
        .map(|viewer| StateView::for_player(state, viewer).objects)
        .collect()
}

fn forecast_of(objects: &[ObjectView], id: mtg_core::ids::ObjectId) -> Vec<ForecastChange> {
    objects
        .iter()
        .find(|o| o.id == id)
        .map(|o| o.forecast.clone())
        .unwrap()
}

#[rstest]
fn the_top_item_carries_one_forecast_for_every_seat(#[values(2, 3, 4)] seats: usize) {
    let mut driver = table(seats, burn("Bolt", 3), island());
    let bear = driver.id_of("Grizzly Bears").unwrap();
    let at = driver.target_named("Grizzly Bears").unwrap();
    driver.cast_at("Bolt", &[at]).unwrap();
    let top = *driver.state().stack().last().unwrap();
    let expected = vec![
        ForecastChange {
            subject: Forecasted::Object(bear),
            change: Change::Damage { amount: 3 },
        },
        ForecastChange {
            subject: Forecasted::Object(bear),
            change: Change::Leaves { fate: Fate::Dies },
        },
    ];
    for objects in views(&driver) {
        assert_eq!(forecast_of(&objects, top), expected);
        for other in objects.iter().filter(|o| o.id != top) {
            assert!(other.forecast.is_empty(), "{} has a forecast", other.name);
            let json = serde_json::to_value(other).unwrap();
            assert!(json.get("forecast").is_none());
        }
    }
}

#[rstest]
#[case::bolt(burn("Bolt", 3), true)]
#[case::draw(study("Study"), false)]
fn a_hidden_card_moves_no_view(
    #[case] spell: Arc<CardDefinition>,
    #[case] forecast: bool,
    #[values(2, 3, 4)] seats: usize,
) {
    let cast = |hidden| {
        let mut driver = table(seats, spell.clone(), hidden);
        let name = spell.name.clone();
        if spell.name == "Bolt" {
            let at = driver.target_named("Craw Wurm").unwrap();
            driver.cast_at(&name, &[at]).unwrap();
        } else {
            driver.cast(&name).unwrap();
        }
        driver
    };
    let plain = cast(island());
    let other = cast(giant());
    let top = *plain.state().stack().last().unwrap();
    let seen = views(&plain);
    for (a, b) in seen.iter().zip(views(&other)) {
        assert_eq!(forecast_of(a, top), forecast_of(&b, top));
        assert_eq!(!forecast_of(a, top).is_empty(), forecast);
    }
}
