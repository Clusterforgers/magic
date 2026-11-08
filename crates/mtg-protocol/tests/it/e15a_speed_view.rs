//! Goal: a player's speed is public: every seat and a spectator see the speed start your engines! gives, the field is left out while a player has none, and the event that sets it reaches every viewer whole, at 2, 3 and 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::events::Event;
use mtg_engine::state::GameState;
use mtg_protocol::{EventView, StateView, project_events, spectator_viewer};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

fn seat(index: usize) -> PlayerId {
    PlayerId::from_index(index)
}

/// A creature with start your engines!.
fn racer() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Racer")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(Ability::Keyword(Keyword::StartYourEngines)),
    )
}

fn scenario(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats).battlefield(0, [ready(racer())]);
    for n in 0..seats {
        scenario = scenario.library(n, (0..3).map(|_| racer()));
    }
    scenario
}

#[rstest]
fn the_speed_is_told_to_every_seat_and_a_spectator(#[values(2, 3, 4)] seats: usize) {
    let before = GameState::from_setup(&scenario(seats).setup().unwrap()).unwrap();
    let driver = Driver::from_scenario(scenario(seats)).unwrap();
    let state = driver.state();
    assert_eq!(state.speed(seat(0)), 1);
    let views: Vec<StateView> = (0..seats)
        .map(|n| StateView::for_player(state, seat(n)))
        .chain([StateView::for_spectator(state)])
        .collect();
    for (viewer, view) in views.iter().enumerate() {
        let speeds: Vec<u8> = view.table.players.iter().map(|p| p.speed).collect();
        let mut wanted = vec![0; seats];
        wanted[0] = 1;
        assert_eq!(speeds, wanted, "viewer {viewer}");
        let text = serde_json::to_string(view).unwrap();
        assert_eq!(text.matches("\"speed\":").count(), 1, "viewer {viewer}");
    }
    let early = StateView::for_spectator(&before);
    assert!(!serde_json::to_string(&early).unwrap().contains("\"speed\""));
    let viewers: Vec<PlayerId> = (0..seats)
        .map(seat)
        .chain([spectator_viewer(&before)])
        .collect();
    let changed = Event::SpeedChanged {
        player: seat(0),
        speed: 1,
    };
    for (viewer, batch) in viewers
        .iter()
        .zip(project_events(&before, driver.engine().events(), &viewers, 0).unwrap())
    {
        assert!(
            batch.events.contains(&EventView::Public(changed.clone())),
            "{viewer:?}"
        );
        assert_eq!(batch.table.players[0].speed, 1);
    }
}
