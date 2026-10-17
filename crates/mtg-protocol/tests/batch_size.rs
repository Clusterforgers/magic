//! Goal: in the middle of a seeded game an event batch, table state included, costs a small fraction
//! of a snapshot of the same moment, so carrying the whole table in every batch stays cheap.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{Step, run_game};
use mtg_core::ids::PlayerId;
use mtg_protocol::{ServerMessage, StateView, project_events};
use mtg_testkit::fuzz_setup;
use rstest::rstest;

/// The size in bytes of one seat's batch message and of the snapshot it could have been sent instead.
struct Sample {
    answers: usize,
    batch: usize,
    snapshot: usize,
}

fn json_len(message: &ServerMessage) -> usize {
    serde_json::to_string(message).unwrap().len()
}

fn ratio(part: usize, whole: usize) -> f64 {
    f64::from(u32::try_from(part).unwrap()) / f64::from(u32::try_from(whole).unwrap())
}

#[rstest]
#[case::two_seats(2, 81, 0.30)]
#[case::two_more_seats(2, 82, 0.30)]
#[case::three_seats(3, 83, 0.20)]
#[case::four_seats(4, 84, 0.16)]
#[case::four_more_seats(4, 85, 0.16)]
fn mid_game_batches_are_a_fraction_of_a_snapshot(
    #[case] players: usize,
    #[case] seed: u64,
    #[case] largest_share: f64,
) {
    let viewers: Vec<PlayerId> = (0..players).map(PlayerId::from_index).collect();
    let mut samples = Vec::new();
    run_game(fuzz_setup(players, seed), seed, |step: &Step| {
        if step.answers == 0 {
            return;
        }
        let batches = project_events(step.before, step.events, &viewers, step.from_seq).unwrap();
        for (viewer, batch) in viewers.iter().zip(batches) {
            let snapshot = ServerMessage::Snapshot {
                seq: step.from_seq,
                state: Box::new(StateView::for_player(step.after, *viewer)),
            };
            samples.push(Sample {
                answers: step.answers,
                batch: json_len(&ServerMessage::Events(batch)),
                snapshot: json_len(&snapshot),
            });
        }
    });

    // The middle half of the game: past the opening, before the endgame.
    let total = samples.iter().map(|s| s.answers).max().unwrap();
    let mid: Vec<&Sample> = samples
        .iter()
        .filter(|s| s.answers >= total / 4 && s.answers < total * 3 / 4)
        .collect();
    assert!(mid.len() > 400, "too few batches to judge: {}", mid.len());
    let batch_bytes: usize = mid.iter().map(|s| s.batch).sum();
    let snapshot_bytes: usize = mid.iter().map(|s| s.snapshot).sum();
    let mut shares: Vec<f64> = mid.iter().map(|s| ratio(s.batch, s.snapshot)).collect();
    shares.sort_by(f64::total_cmp);
    let overall = ratio(batch_bytes, snapshot_bytes);
    let p90 = shares[shares.len() * 9 / 10];
    let mean_batch = batch_bytes / mid.len();
    let mean_snapshot = snapshot_bytes / mid.len();
    assert!(
        overall < largest_share && p90 < 0.4,
        "{players} seats: mean batch {mean_batch} B vs mean snapshot {mean_snapshot} B \
         ({overall:.3} of it; 90th percentile {p90:.3})"
    );
}
