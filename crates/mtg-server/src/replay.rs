//! Replays: a finished game's log cut into steps and projected for one viewer, with what that viewer may know.

use mtg_core::ids::PlayerId;
use mtg_engine::{EngineError, Event, GameSetup, apply_event, replay};
use mtg_protocol::{EventBatch, ProtocolError, project_events};
use thiserror::Error;

/// Why a log could not be replayed.
#[derive(Debug, Error)]
pub enum ReplayError {
    /// The log does not apply to the game's setup.
    #[error("the log does not replay: {0}")]
    Engine(#[from] EngineError),
    /// A step could not be projected.
    #[error("a step could not be projected: {0}")]
    Protocol(#[from] ProtocolError),
}

/// Whether `event` is bookkeeping a person would not stop the replay for: priority, an empty mana pool, the start of a
/// turn or step. A step of the replay ends only once it holds an event that is not.
fn quiet(event: &Event) -> bool {
    matches!(
        event,
        Event::PriorityGranted { .. }
            | Event::PriorityPassed { .. }
            | Event::StepBegan { .. }
            | Event::StepDone { .. }
            | Event::TurnBegan { .. }
            | Event::NothingToDo { .. }
            | Event::ManaPoolEmptied { .. }
    )
}

/// The log cut into steps: a step ends where a player is given priority after something that is not bookkeeping
/// happened, so each step shows a change and the quiet passes between two changes join the next one.
pub fn cut(events: &[Event]) -> Vec<&[Event]> {
    let mut steps = Vec::new();
    let (mut start, mut loud) = (0, false);
    for (at, event) in events.iter().enumerate() {
        if loud && matches!(event, Event::PriorityGranted { .. }) {
            steps.push(&events[start..at]);
            (start, loud) = (at, false);
        }
        loud |= !quiet(event);
    }
    if start < events.len() {
        steps.push(&events[start..]);
    }
    steps
}

/// The steps of the game of `setup` whose log is `events`, as `viewer` sees them: the first at `from_seq` 0 starts the
/// view and each later one folds into it, exactly as the batches a client of that seat (or a spectator, for a viewer
/// past the last seat) was sent.
pub fn steps(
    setup: &GameSetup,
    events: &[Event],
    viewer: PlayerId,
) -> Result<Vec<EventBatch>, ReplayError> {
    let mut state = replay(setup, &[])?;
    let mut from = 0u64;
    let mut batches = Vec::new();
    for step in cut(events) {
        let mut projected = project_events(&state, step, &[viewer], from)?;
        batches.append(&mut projected);
        for event in step {
            apply_event(&mut state, event)?;
        }
        from += step.len() as u64;
    }
    Ok(batches)
}

#[cfg(test)]
mod tests {
    //! Goal: the steps of a replay are the whole log in order with no step of bookkeeping alone, and folded one after the other they end in exactly the view the viewer has of the final state, for every seat and a spectator of 2 to 4-seat bot games; a spectator's steps name no card of a hand.
    #![allow(clippy::unwrap_used)]
    use mtg_protocol::{StateView, spectator_viewer};
    use mtg_testkit::{fuzz_setup, play_game};
    use rstest::rstest;

    use super::*;

    fn game(seats: usize, seed: u64) -> (GameSetup, Vec<Event>) {
        let setup = fuzz_setup(seats, seed);
        let outcome = play_game(setup.clone(), seed).unwrap();
        (setup, outcome.events)
    }

    #[rstest]
    fn the_steps_fold_into_the_final_view(
        #[values(2, 3, 4)] seats: usize,
        #[values(1, 2)] seed: u64,
    ) {
        let (setup, events) = game(seats, seed);
        let last = replay(&setup, &events).unwrap();
        let cuts = cut(&events);
        assert_eq!(cuts.concat(), events);
        assert!(
            cuts.iter()
                .skip(1)
                .all(|step| step.iter().any(|event| !quiet(event)))
        );
        let viewers = (0..seats)
            .map(PlayerId::from_index)
            .chain([spectator_viewer(&last)]);
        for viewer in viewers {
            let batches = steps(&setup, &events, viewer).unwrap();
            assert_eq!(batches.len(), cuts.len());
            let mut view = StateView::from_first_batch(&batches[0]).unwrap();
            for batch in &batches[1..] {
                view.apply(batch);
            }
            assert_eq!(view, StateView::for_player(&last, viewer), "{viewer:?}");
        }
    }

    #[test]
    fn a_spectator_sees_no_hand() {
        let (setup, events) = game(3, 4);
        let spectator = spectator_viewer(&replay(&setup, &[]).unwrap());
        for batch in steps(&setup, &events, spectator).unwrap() {
            assert!(
                batch
                    .objects
                    .iter()
                    .all(|object| !matches!(object.zone, mtg_engine::zones::Zone::Hand(_)))
            );
        }
    }
}
