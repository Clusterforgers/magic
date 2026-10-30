//! Goal: the log a snapshot carries is what the seat would have been sent had it been connected all along: over a
//! window of any size it equals the live batches of that seat, and no event of it names a card the seat could not see
//! when it happened; the names the window carries are real, bounded, never of a card the seat could not see, and none is
//! of an object the state after the window names itself.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::{Step, run_game, unique_setup};
use crate::hidden_info::{hidden_across, leaks};

use mtg_core::ids::PlayerId;
use mtg_engine::{GameSetup, replay};
use mtg_protocol::{
    EventView, LogName, SNAPSHOT_LOG, SNAPSHOT_NAMES, ServerMessage, StateView, project_events,
    project_log, project_window,
};
use rstest::rstest;

/// The events of every seat's live batches, in log order, and the game's setup and events.
struct Played {
    live: Vec<Vec<EventView>>,
    setup: GameSetup,
    events: Vec<mtg_engine::Event>,
    /// How many names the steps' windows carried in all.
    named: usize,
}

/// Plays a seeded bot game and checks each step's events, projected alone, for leaks and against the live batch.
fn play(players: usize, seed: u64) -> Played {
    let viewers: Vec<PlayerId> = (0..players).map(PlayerId::from_index).collect();
    let setup = unique_setup(players, seed);
    let mut live: Vec<Vec<EventView>> = viewers.iter().map(|_| Vec::new()).collect();
    let (mut checked, mut named) = (0usize, 0usize);
    let engine = run_game(setup.clone(), seed, |step: &Step| {
        let batches = project_events(step.before, step.events, &viewers, step.from_seq).unwrap();
        for ((viewer, batch), seen) in viewers.iter().zip(batches).zip(&mut live) {
            let log = project_log(step.before, step.events, *viewer).unwrap();
            assert_eq!(
                log, batch.events,
                "seed {seed} viewer {viewer:?}: log and batch differ"
            );
            let hidden = hidden_across(step.before, step.after, *viewer);
            checked += hidden.len();
            let found = leaks(&serde_json::to_value(&log).unwrap(), &hidden);
            assert!(
                found.is_empty(),
                "seed {seed} viewer {viewer:?}: the log leaks {found:?}"
            );
            let window = project_window(step.before, step.events, *viewer).unwrap();
            assert_eq!(
                window.log, log,
                "seed {seed} viewer {viewer:?}: the window changed the log"
            );
            named += window.names.len();
            let found = leaks(&serde_json::to_value(&window.names).unwrap(), &hidden);
            assert!(
                found.is_empty(),
                "seed {seed} viewer {viewer:?}: the names leak {found:?}"
            );
            seen.extend(batch.events);
        }
    });
    assert!(checked > 200, "test must not be vacuous: {checked}");
    Played {
        live,
        setup,
        events: engine.events().to_vec(),
        named,
    }
}

#[rstest]
#[case(2, 1)]
#[case(3, 3)]
#[case(4, 5)]
fn a_rebuilt_window_equals_the_live_batches_and_leaks_nothing(
    #[case] players: usize,
    #[case] seed: u64,
) {
    let Played {
        live,
        setup,
        events,
        named,
    } = play(players, seed);
    assert!(
        named > 0,
        "test must not be vacuous: no step named anything"
    );
    assert!(
        events.len() > 2 * SNAPSHOT_LOG,
        "the game must outgrow the window: {}",
        events.len()
    );
    // The whole game, the latest window, a window that starts mid-game and a single event.
    let windows = [
        0..events.len(),
        events.len() - SNAPSHOT_LOG..events.len(),
        events.len() / 3..events.len() / 3 + 97,
        events.len() - 1..events.len(),
    ];
    for window in windows {
        let before = replay(&setup, &events[..window.start]).unwrap();
        for (index, seen) in live.iter().enumerate() {
            let viewer = PlayerId::from_index(index);
            let log = project_log(&before, &events[window.clone()], viewer).unwrap();
            assert_eq!(
                log,
                seen[window.clone()],
                "seed {seed} viewer {viewer:?} window {window:?}"
            );
            let end = replay(&setup, &events[..window.end]).unwrap();
            let rebuilt = project_window(&before, &events[window.clone()], viewer).unwrap();
            assert_eq!(rebuilt.log, log);
            assert_names_are_real(&end, viewer, &rebuilt.names);
        }
    }
}

/// Checks the names a window carried against the state the window ends in: each is a card's real name, listed once in
/// id order, and none is of an object the viewer's view of that state names itself.
fn assert_names_are_real(end: &mtg_engine::GameState, viewer: PlayerId, names: &[LogName]) {
    assert!(names.len() <= SNAPSHOT_NAMES);
    assert!(names.windows(2).all(|pair| pair[0].object < pair[1].object));
    let view = StateView::for_player(end, viewer);
    for LogName { object, name } in names {
        assert!(!name.is_empty(), "{object:?} has no name");
        let shown = view.objects.iter().find(|shown| shown.id == *object);
        assert!(
            shown.is_none_or(|shown| shown.name.is_empty()),
            "{object:?} is named by the state itself"
        );
        // An ability on the stack is named after its source, so only a card must read as its own name.
        if let Some(card) = end.object(*object).filter(|card| card.ability().is_none()) {
            assert_eq!(&card.card().name, name, "{object:?}");
        }
    }
}

#[rstest]
#[case(2, 7)]
#[case(4, 9)]
fn every_event_has_a_view_for_every_seat_and_some_are_hidden(
    #[case] players: usize,
    #[case] seed: u64,
) {
    let Played { live, events, .. } = play(players, seed);
    let hidden: Vec<usize> = live
        .iter()
        .map(|log| {
            log.iter()
                .filter(|view| matches!(view, EventView::Hidden(_)))
                .count()
        })
        .collect();
    assert!(
        hidden.iter().all(|count| *count > 0),
        "hidden events {hidden:?}"
    );
    for log in &live {
        assert_eq!(log.len(), events.len(), "one view for every event");
    }
}

#[test]
fn a_snapshot_with_its_log_stays_small_next_to_the_state() {
    let Played { live, .. } = play(4, 5);
    let view_log: Vec<EventView> = live[0][live[0].len() - SNAPSHOT_LOG..].to_vec();
    let with_log = |log: Vec<EventView>, names: Vec<LogName>| {
        let setup = unique_setup(4, 5);
        let state = mtg_engine::GameState::from_setup(&setup).unwrap();
        let message = ServerMessage::Snapshot {
            seq: 0,
            state: Box::new(StateView::for_player(&state, PlayerId::from_index(0))),
            log,
            names,
        };
        serde_json::to_string(&message).unwrap().len()
    };
    let none = with_log(Vec::new(), Vec::new());
    let log_bytes = with_log(view_log, Vec::new()) - none;
    assert!(log_bytes < 40_000, "a full log costs {log_bytes} bytes");
    // The most names a window carries, each as long as a card name can be.
    let longest = "N".repeat(141);
    let names = (0..SNAPSHOT_NAMES as u64)
        .map(|id| LogName {
            object: mtg_core::ids::ObjectId(id),
            name: longest.clone(),
        })
        .collect();
    let name_bytes = with_log(Vec::new(), names) - none;
    assert!(
        name_bytes < 120_000,
        "a full name table costs {name_bytes} bytes"
    );
}
