//! The chess-clock tally: the wall time each seat has spent on turns of its own.
//!
//! A turn runs from its start to the start of the next turn (or the end of the game, or the moment its active player
//! leaves). Time while no seat is connected does not count. The tally is public, depends on the clock alone, lives in
//! memory with the running game and starts from zero when a game is resumed.

use std::time::Duration;

use mtg_core::ids::PlayerId;
use mtg_engine::GameState;
use mtg_engine::state::Stage;
use mtg_protocol::TableView;
use tokio::time::Instant;

/// The turn whose time is running.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Current {
    number: u32,
    seat: PlayerId,
    /// Time counted before the last pause.
    counted: Duration,
    /// When counting last (re)started; `None` while paused.
    since: Option<Instant>,
}

impl Current {
    fn elapsed(&self, now: Instant) -> Duration {
        self.counted
            + self
                .since
                .map_or(Duration::ZERO, |since| now.saturating_duration_since(since))
    }
}

/// The time of every seat's finished turns and the turn running now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TurnTimes {
    used: Vec<Duration>,
    current: Option<Current>,
    paused: bool,
}

/// Whole milliseconds, saturating.
fn millis(span: Duration) -> u64 {
    u64::try_from(span.as_millis()).unwrap_or(u64::MAX)
}

impl TurnTimes {
    /// No time used yet by any of `seats` seats.
    pub(crate) fn new(seats: usize) -> TurnTimes {
        TurnTimes {
            used: vec![Duration::ZERO; seats],
            current: None,
            paused: false,
        }
    }

    /// Notes the turn `state` is in at `now`: a new turn ends the one before and starts its own count.
    ///
    /// Call after every change of the engine, so no turn goes by unseen.
    pub(crate) fn watch(&mut self, state: &GameState, now: Instant) {
        let turn = (state.stage() == Stage::Turns)
            .then(|| {
                state
                    .active_player()
                    .map(|seat| (state.turn().number(), seat))
            })
            .flatten();
        self.enter(turn, now);
    }

    /// Enters `turn` (its number and active seat; `None` outside the turns) at `now`.
    fn enter(&mut self, turn: Option<(u32, PlayerId)>, now: Instant) {
        let same = match (&self.current, turn) {
            (Some(current), Some((number, seat))) => {
                current.number == number && current.seat == seat
            }
            (None, None) => true,
            _ => false,
        };
        if same {
            return;
        }
        if let Some(done) = self.current.take()
            && let Some(used) = self.used.get_mut(done.seat.index())
        {
            *used += done.elapsed(now);
        }
        self.current = turn.map(|(number, seat)| Current {
            number,
            seat,
            counted: Duration::ZERO,
            since: (!self.paused).then_some(now),
        });
    }

    /// Stops (`paused`) or restarts the running turn's count at `now`; nothing changes when it already is so.
    pub(crate) fn pause(&mut self, paused: bool, now: Instant) {
        if self.paused == paused {
            return;
        }
        self.paused = paused;
        if let Some(current) = &mut self.current {
            if paused {
                current.counted = current.elapsed(now);
                current.since = None;
            } else {
                current.since = Some(now);
            }
        }
    }

    /// The time `seat` has used on its own turns up to `now`, the running turn included.
    pub(crate) fn used(&self, seat: PlayerId, now: Instant) -> Duration {
        let finished = self.used.get(seat.index()).copied().unwrap_or_default();
        let running = self
            .current
            .as_ref()
            .filter(|current| current.seat == seat)
            .map_or(Duration::ZERO, |current| current.elapsed(now));
        finished + running
    }

    /// The seat whose count is running at this moment, if any.
    pub(crate) fn ticking(&self) -> Option<PlayerId> {
        self.current
            .as_ref()
            .filter(|current| current.since.is_some())
            .map(|current| current.seat)
    }

    /// Writes every seat's time at `now` into the players of `table`.
    pub(crate) fn stamp(&self, table: &mut TableView, now: Instant) {
        let ticking = self.ticking();
        for player in &mut table.players {
            player.turn_time_ms = millis(self.used(player.seat, now));
            player.turn_time_running = ticking == Some(player.seat);
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: each seat's figure is the sum of its own turns from start to the next turn's start, the running turn included, pauses excluded, and only the seat whose turn runs is ticking.
    use super::*;
    use rstest::rstest;

    /// A table of three seats driven by hand: `turn(n, seat)` stands for what `watch` sees.
    struct Table {
        times: TurnTimes,
        start: Instant,
    }

    impl Table {
        fn new() -> Table {
            Table {
                times: TurnTimes::new(3),
                start: Instant::now(),
            }
        }

        fn at(&self, secs: u64) -> Instant {
            self.start + Duration::from_secs(secs)
        }

        fn turn(&mut self, number: u32, seat: usize, secs: u64) {
            let now = self.at(secs);
            self.times
                .enter(Some((number, PlayerId::from_index(seat))), now);
        }

        fn end(&mut self, secs: u64) {
            let now = self.at(secs);
            self.times.enter(None, now);
        }

        fn secs(&self, seat: usize, at: u64) -> u64 {
            self.times
                .used(PlayerId::from_index(seat), self.at(at))
                .as_secs()
        }
    }

    #[rstest]
    #[case::one_turn_each(&[(1, 0, 0), (2, 1, 30), (3, 2, 50)], 80, [30, 20, 30])]
    #[case::turns_add_up(&[(1, 0, 0), (2, 1, 10), (3, 0, 25), (4, 1, 40)], 45, [25, 20, 0])]
    #[case::an_extra_turn_counts_apart(&[(1, 0, 0), (2, 0, 10), (3, 1, 30)], 31, [30, 1, 0])]
    #[case::seen_twice_is_one_turn(&[(1, 0, 0), (1, 0, 5), (2, 1, 10)], 10, [10, 0, 0])]
    fn turns_add_up_per_seat(
        #[case] turns: &[(u32, usize, u64)],
        #[case] at: u64,
        #[case] expected: [u64; 3],
    ) {
        let mut table = Table::new();
        for &(number, seat, secs) in turns {
            table.turn(number, seat, secs);
        }
        let got = [table.secs(0, at), table.secs(1, at), table.secs(2, at)];
        assert_eq!(got, expected);
    }

    #[test]
    fn the_end_of_the_game_stops_every_count() {
        let mut table = Table::new();
        table.turn(1, 0, 0);
        table.end(12);
        assert_eq!(table.secs(0, 100), 12);
        assert_eq!(table.times.ticking(), None);
    }

    #[test]
    fn paused_time_does_not_count() {
        let mut table = Table::new();
        table.turn(1, 0, 0);
        table.times.pause(true, table.at(10));
        assert_eq!(table.times.ticking(), None);
        assert_eq!(table.secs(0, 50), 10, "a paused count stands still");
        table.times.pause(true, table.at(20));
        table.times.pause(false, table.at(60));
        table.turn(2, 1, 65);
        table.times.pause(true, table.at(70));
        table.turn(3, 2, 90);
        table.times.pause(false, table.at(100));
        assert_eq!(
            [table.secs(0, 105), table.secs(1, 105), table.secs(2, 105)],
            [15, 5, 5]
        );
    }

    #[test]
    fn only_the_running_seat_ticks_and_the_stamp_carries_every_figure() {
        let mut table = Table::new();
        table.turn(1, 0, 0);
        table.turn(2, 1, 7);
        assert_eq!(table.times.ticking(), Some(PlayerId(1)));
        let engine = mtg_engine::Engine::new(mtg_testkit::fuzz_setup(3, 1)).unwrap();
        let mut view = mtg_protocol::StateView::for_player(engine.state(), PlayerId(0));
        table.times.stamp(&mut view.table, table.at(9));
        let figures: Vec<(u64, bool)> = view
            .table
            .players
            .iter()
            .map(|p| (p.turn_time_ms, p.turn_time_running))
            .collect();
        assert_eq!(figures, [(7000, false), (2000, true), (0, false)]);
    }

    #[test]
    fn a_seat_outside_the_table_has_used_nothing() {
        let table = Table::new();
        assert_eq!(table.times.used(PlayerId(9), table.at(5)), Duration::ZERO);
    }
}
