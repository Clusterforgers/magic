//! Previews: a seat asks what combat damage would do if it gave an answer, and is told without the game moving.
//!
//! A preview is advice, so it never fails loudly. Every `Preview` gets exactly one `Forecast` to the seat that sent
//! it, with no forecast when the decision is not that seat's pending one (stale, foreign, answered, or not yet
//! started), when the engine would not take the answer or has no combat outcome for it, or when the seat is
//! previewing faster than [`PREVIEWS_PER_SECOND`] allows. Nothing is logged, stored or sent to another seat, and a
//! preview never touches the engine, so a flood of them can slow nobody's real answer beyond the work they cost,
//! which the per-seat bucket caps.

use std::time::Instant;

use mtg_core::ids::PlayerId;
use mtg_engine::Answer;
use mtg_protocol::{DecisionId, ServerMessage};

use crate::actor::{GameActor, Phase};
use crate::limit::RateLimiter;
use crate::store::EventStore;

/// Previews a seat may send back to back.
pub const PREVIEW_BURST: u32 = 20;
/// Previews per second a seat may keep sending once its burst is spent.
pub const PREVIEWS_PER_SECOND: u32 = 10;

/// One token bucket per seat. It belongs to the game, not the connection, so reconnecting does not refill it.
#[derive(Debug)]
pub(crate) struct PreviewLimits {
    buckets: Vec<RateLimiter>,
}

impl PreviewLimits {
    /// Full buckets at time `now` for `seats` seats.
    pub(crate) fn new(seats: usize, now: Instant) -> PreviewLimits {
        PreviewLimits {
            buckets: (0..seats)
                .map(|_| RateLimiter::new(PREVIEW_BURST, PREVIEWS_PER_SECOND, now))
                .collect(),
        }
    }

    /// Takes a token from `seat`'s bucket; false when it is empty, or when the seat does not exist.
    pub(crate) fn allow(&mut self, seat: PlayerId, now: Instant) -> bool {
        self.buckets
            .get_mut(seat.index())
            .is_some_and(|bucket| bucket.allow(now))
    }
}

impl<S: EventStore> GameActor<S> {
    /// Answers `seat`'s preview of `answer` to `decision` with one `Forecast`.
    pub(crate) fn preview(&mut self, seat: PlayerId, decision: DecisionId, answer: &Answer) {
        let forecast = if self.previews.allow(seat, Instant::now()) {
            match &self.phase {
                Phase::Running(running) => running.preview(seat, decision, answer),
                Phase::Lobby { .. } => None,
            }
        } else {
            None
        };
        self.seats
            .send(seat, ServerMessage::Forecast { decision, forecast });
    }
}

#[cfg(test)]
mod tests {
    //! Goal: each seat has its own bucket that allows a burst of previews, refills at a steady rate, and refuses seats that do not exist.
    use super::*;
    use std::time::Duration;

    fn spend(limits: &mut PreviewLimits, seat: usize, at: Instant, count: usize) -> usize {
        let seat = PlayerId::from_index(seat);
        (0..count).filter(|_| limits.allow(seat, at)).count()
    }

    #[test]
    fn a_seat_gets_its_burst_and_then_the_steady_rate() {
        let base = Instant::now();
        let mut limits = PreviewLimits::new(2, base);
        assert_eq!(spend(&mut limits, 0, base, 50), PREVIEW_BURST as usize);
        let second = base + Duration::from_secs(1);
        assert_eq!(
            spend(&mut limits, 0, second, 50),
            PREVIEWS_PER_SECOND as usize
        );
        let later = base + Duration::from_secs(60);
        assert_eq!(spend(&mut limits, 0, later, 50), PREVIEW_BURST as usize);
    }

    #[test]
    fn seats_do_not_share_a_bucket() {
        let base = Instant::now();
        let mut limits = PreviewLimits::new(3, base);
        assert_eq!(spend(&mut limits, 0, base, 100), PREVIEW_BURST as usize);
        assert_eq!(spend(&mut limits, 1, base, 5), 5);
        assert_eq!(spend(&mut limits, 2, base, 100), PREVIEW_BURST as usize);
    }

    #[test]
    fn a_seat_that_is_not_there_gets_nothing() {
        let base = Instant::now();
        let mut limits = PreviewLimits::new(2, base);
        assert!(!limits.allow(PlayerId::from_index(2), base));
        assert!(!limits.allow(PlayerId::from_index(usize::MAX >> 1), base));
    }
}
