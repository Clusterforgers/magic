//! Spacing between requests, so a source is never asked faster than its etiquette allows.

use std::time::Duration;

use tokio::sync::{Mutex, MutexGuard};
use tokio::time::Instant;

/// Hands out turns that start at least `gap` after the previous turn ended.
#[derive(Debug)]
pub struct Pacer {
    gap: Duration,
    last_end: Mutex<Option<Instant>>,
}

/// The right to make one request; the next turn is timed from the moment this one is dropped.
pub struct Turn<'a> {
    last_end: MutexGuard<'a, Option<Instant>>,
}

impl Pacer {
    /// A pacer that leaves `gap` between one request's end and the next one's start.
    pub fn new(gap: Duration) -> Self {
        Self {
            gap,
            last_end: Mutex::new(None),
        }
    }

    /// Waits for the gap to pass, then holds every other turn back until the returned value is dropped.
    pub async fn turn(&self) -> Turn<'_> {
        let last_end = self.last_end.lock().await;
        if let Some(end) = *last_end {
            tokio::time::sleep_until(end + self.gap).await;
        }
        Turn { last_end }
    }
}

impl Drop for Turn<'_> {
    fn drop(&mut self) {
        *self.last_end = Some(Instant::now());
    }
}

#[cfg(test)]
mod tests {
    //! Goal: turns never start sooner than the gap after the previous turn ended, however long the turns last and however many tasks ask at once, and the first turn never waits.
    use rstest::rstest;

    use super::*;

    const GAP: Duration = Duration::from_millis(100);

    #[tokio::test(start_paused = true)]
    async fn the_first_turn_starts_at_once() {
        let started = Instant::now();
        let pacer = Pacer::new(GAP);
        let _turn = pacer.turn().await;
        assert_eq!(started.elapsed(), Duration::ZERO);
    }

    #[rstest]
    #[case::instant_requests(Duration::ZERO)]
    #[case::short_requests(Duration::from_millis(30))]
    #[case::requests_longer_than_the_gap(Duration::from_millis(450))]
    #[tokio::test(start_paused = true)]
    async fn consecutive_turns_are_a_gap_apart(#[case] busy: Duration) {
        let pacer = Pacer::new(GAP);
        let mut starts = Vec::new();
        let mut ends = Vec::new();
        for _ in 0..4 {
            let turn = pacer.turn().await;
            starts.push(Instant::now());
            tokio::time::sleep(busy).await;
            ends.push(Instant::now());
            drop(turn);
        }
        for pair in 1..starts.len() {
            assert!(
                starts[pair] - ends[pair - 1] >= GAP,
                "turn {pair} began {:?} after the previous one ended",
                starts[pair] - ends[pair - 1]
            );
        }
    }

    #[tokio::test(start_paused = true)]
    async fn tasks_asking_together_are_served_one_gap_apart() {
        let pacer = std::sync::Arc::new(Pacer::new(GAP));
        let begun = Instant::now();
        let mut tasks = Vec::new();
        for _ in 0..5 {
            let pacer = pacer.clone();
            tasks.push(tokio::spawn(async move {
                let _turn = pacer.turn().await;
                Instant::now() - begun
            }));
        }
        let mut offsets = Vec::new();
        for task in tasks {
            offsets.push(task.await.unwrap());
        }
        offsets.sort();
        for pair in offsets.windows(2) {
            assert!(pair[1].checked_sub(pair[0]).unwrap() >= GAP, "{offsets:?}");
        }
    }
}
