//! The chess-clock tally: the wall time each seat has spent on turns of its own.
//!
//! A turn runs from its start to the start of the next turn (or the end of the game, or the moment its active player
//! leaves). Where each seat takes its own turn, the active player's count runs for the whole turn. Where teams share
//! their turns (CR 805), the count of a seat runs while the game waits on that seat during its team's turn: a decision
//! of a teammate charges the teammate and never the team's lead, and when several seats of the team are asked at once
//! each runs until it answers; while an opponent decides, nobody on the active team is charged. Time while no seat is
//! connected does not count. The tally is public, depends on the clock alone, lives in memory with the running game
//! and starts from zero when a game is resumed.

use std::time::Duration;

use mtg_core::ids::PlayerId;
use mtg_engine::GameState;
use mtg_engine::state::Stage;
use mtg_protocol::TableView;
use tokio::time::Instant;

/// The time of every seat's own turns, the counts running now included.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TurnTimes {
    /// Time counted so far per seat, without the count running now.
    used: Vec<Duration>,
    /// The seats whose count runs unless the tally is paused.
    charged: Vec<PlayerId>,
    /// When each running count (re)started; `None` for a seat that is not counting.
    since: Vec<Option<Instant>>,
    paused: bool,
}

/// Whole milliseconds, saturating.
fn millis(span: Duration) -> u64 {
    u64::try_from(span.as_millis()).unwrap_or(u64::MAX)
}

/// The seats to charge in `state` while the game waits on `waiting`: none outside the turns; the active player where
/// each seat takes its own turn; where teams share their turns, the waited-on seats of the active team.
fn charged(state: &GameState, waiting: &[PlayerId]) -> Vec<PlayerId> {
    if state.stage() != Stage::Turns {
        return Vec::new();
    }
    let Some(active) = state.active_player() else {
        return Vec::new();
    };
    if !state.shares_turns() {
        return vec![active];
    }
    let mut seats: Vec<PlayerId> = waiting
        .iter()
        .copied()
        .filter(|seat| state.is_active(*seat))
        .collect();
    seats.dedup();
    seats
}

impl TurnTimes {
    /// No time used yet by any of `seats` seats.
    pub(crate) fn new(seats: usize) -> TurnTimes {
        TurnTimes {
            used: vec![Duration::ZERO; seats],
            charged: Vec::new(),
            since: vec![None; seats],
            paused: false,
        }
    }

    /// Notes the turn `state` is in and the seats the game is `waiting` on at `now`: the counts of seats no longer
    /// charged stop and those of seats now charged start.
    ///
    /// Call after every change of the engine, so no turn and no decision goes by unseen.
    pub(crate) fn watch(&mut self, state: &GameState, waiting: &[PlayerId], now: Instant) {
        self.charge(charged(state, waiting), now);
    }

    /// Charges exactly `seats` from `now` on.
    fn charge(&mut self, seats: Vec<PlayerId>, now: Instant) {
        self.charged = seats;
        self.settle(now);
    }

    /// Starts the count of every charged seat that is not counting and stops the others, unless paused.
    fn settle(&mut self, now: Instant) {
        for (index, (used, since)) in self.used.iter_mut().zip(&mut self.since).enumerate() {
            let runs = !self.paused && self.charged.iter().any(|seat| seat.index() == index);
            match (*since, runs) {
                (Some(start), false) => {
                    *used += now.saturating_duration_since(start);
                    *since = None;
                }
                (None, true) => *since = Some(now),
                _ => {}
            }
        }
    }

    /// Stops (`paused`) or restarts the counts at `now`; nothing changes when it already is so.
    pub(crate) fn pause(&mut self, paused: bool, now: Instant) {
        self.paused = paused;
        self.settle(now);
    }

    /// The time `seat` has used on its own turns up to `now`, the running count included.
    pub(crate) fn used(&self, seat: PlayerId, now: Instant) -> Duration {
        let finished = self.used.get(seat.index()).copied().unwrap_or_default();
        let running = self
            .since
            .get(seat.index())
            .copied()
            .flatten()
            .map_or(Duration::ZERO, |start| now.saturating_duration_since(start));
        finished + running
    }

    /// Whether the count of `seat` is running at this moment.
    pub(crate) fn ticking(&self, seat: PlayerId) -> bool {
        self.since.get(seat.index()).is_some_and(Option::is_some)
    }

    /// Writes every seat's time at `now` into the players of `table`.
    pub(crate) fn stamp(&self, table: &mut TableView, now: Instant) {
        for player in &mut table.players {
            player.turn_time_ms = millis(self.used(player.seat, now));
            player.turn_time_running = self.ticking(player.seat);
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: each seat's figure is the sum of the time it was charged, pauses excluded, and only the charged seats tick: the active player for the whole of its turn where each seat takes its own turn, and where teams share their turns the seats of the active team the game waits on (a teammate's decision never charges the lead, several asked at once all run until they answer, an opponent's decision charges nobody).
    use super::*;
    use mtg_core::house::HouseRules;
    use mtg_core::teams::{TeamMode, Teams};
    use mtg_testkit::Scenario;
    use rstest::rstest;

    /// A table of three seats driven by hand: `charge(seats, secs)` stands for what `watch` decides.
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

        fn charge(&mut self, seats: &[usize], secs: u64) {
            let now = self.at(secs);
            let seats = seats.iter().map(|n| PlayerId::from_index(*n)).collect();
            self.times.charge(seats, now);
        }

        fn secs(&self, seat: usize, at: u64) -> u64 {
            self.times
                .used(PlayerId::from_index(seat), self.at(at))
                .as_secs()
        }

        fn ticking(&self) -> Vec<usize> {
            (0..3)
                .filter(|n| self.times.ticking(PlayerId::from_index(*n)))
                .collect()
        }
    }

    #[rstest]
    #[case::one_turn_each(&[(0, 0), (1, 30), (2, 50)], 80, [30, 20, 30])]
    #[case::turns_add_up(&[(0, 0), (1, 10), (0, 25), (1, 40)], 45, [25, 20, 0])]
    #[case::an_extra_turn_counts_on(&[(0, 0), (0, 10), (1, 30)], 31, [30, 1, 0])]
    #[case::seen_twice_is_one_turn(&[(0, 0), (0, 5), (1, 10)], 10, [10, 0, 0])]
    fn turns_add_up_per_seat(
        #[case] turns: &[(usize, u64)],
        #[case] at: u64,
        #[case] expected: [u64; 3],
    ) {
        let mut table = Table::new();
        for &(seat, secs) in turns {
            table.charge(&[seat], secs);
        }
        let got = [table.secs(0, at), table.secs(1, at), table.secs(2, at)];
        assert_eq!(got, expected);
    }

    #[test]
    fn the_end_of_the_game_stops_every_count() {
        let mut table = Table::new();
        table.charge(&[0], 0);
        table.charge(&[], 12);
        assert_eq!(table.secs(0, 100), 12);
        assert_eq!(table.ticking(), Vec::<usize>::new());
    }

    #[test]
    fn paused_time_does_not_count() {
        let mut table = Table::new();
        table.charge(&[0], 0);
        table.times.pause(true, table.at(10));
        assert_eq!(table.ticking(), Vec::<usize>::new());
        assert_eq!(table.secs(0, 50), 10, "a paused count stands still");
        table.times.pause(true, table.at(20));
        table.times.pause(false, table.at(60));
        table.charge(&[1], 65);
        table.times.pause(true, table.at(70));
        table.charge(&[2], 90);
        table.times.pause(false, table.at(100));
        assert_eq!(
            [table.secs(0, 105), table.secs(1, 105), table.secs(2, 105)],
            [15, 5, 5]
        );
    }

    #[test]
    fn seats_asked_together_run_until_each_answers() {
        let mut table = Table::new();
        table.charge(&[0, 1], 0);
        assert_eq!(table.ticking(), [0, 1]);
        table.charge(&[1], 4);
        assert_eq!(table.ticking(), [1], "seat 0 answered and stopped at once");
        table.charge(&[], 9);
        assert_eq!(
            [table.secs(0, 20), table.secs(1, 20), table.secs(2, 20)],
            [4, 9, 0]
        );
    }

    #[test]
    fn a_pause_holds_every_asked_seat() {
        let mut table = Table::new();
        table.charge(&[0, 2], 0);
        table.times.pause(true, table.at(3));
        table.times.pause(false, table.at(10));
        table.charge(&[2], 12);
        assert_eq!(
            [table.secs(0, 30), table.secs(1, 30), table.secs(2, 30)],
            [5, 0, 23]
        );
    }

    #[test]
    fn only_the_running_seat_ticks_and_the_stamp_carries_every_figure() {
        let mut table = Table::new();
        table.charge(&[0], 0);
        table.charge(&[1], 7);
        assert_eq!(table.ticking(), [1]);
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

    /// A game at turn 1 with `active` playing, at a table of `teams` (each seat its own when empty).
    fn state_of(mode: Option<TeamMode>, teams: &[u32], seats: usize, active: usize) -> GameState {
        let house = HouseRules {
            teams: mode.map(|mode| Teams {
                mode,
                seats: teams.to_vec(),
            }),
            ..HouseRules::default()
        };
        let engine = Scenario::new(seats)
            .house_rules(house)
            .at(1, active, mtg_engine::turn::Step::Main1)
            .build()
            .unwrap();
        engine.state().clone()
    }

    /// The seats charged, as numbers.
    fn who(state: &GameState, waiting: &[usize]) -> Vec<usize> {
        let waiting: Vec<PlayerId> = waiting.iter().map(|n| PlayerId::from_index(*n)).collect();
        charged(state, &waiting)
            .iter()
            .map(|seat| seat.index())
            .collect()
    }

    const GIANT: Option<TeamMode> = Some(TeamMode::TwoHeadedGiant);
    const SHARED: Option<TeamMode> = Some(TeamMode::SharedTurns);

    #[rstest]
    #[case::free_for_all_the_active_player(None, &[], 3, 1, &[1], &[1])]
    #[case::free_for_all_an_opponent_decides(None, &[], 3, 1, &[2], &[1])]
    #[case::free_for_all_nobody_asked(None, &[], 3, 1, &[], &[1])]
    #[case::teams_apart_the_active_player(Some(TeamMode::Teams), &[0, 1, 0, 1], 4, 0, &[0], &[0])]
    #[case::teams_apart_a_teammate_decides(Some(TeamMode::Teams), &[0, 1, 0, 1], 4, 0, &[2], &[0])]
    #[case::giant_the_lead(GIANT, &[0, 0, 1, 1], 4, 0, &[0], &[0])]
    #[case::giant_the_second_head(GIANT, &[0, 0, 1, 1], 4, 0, &[1], &[1])]
    #[case::giant_an_opponent(GIANT, &[0, 0, 1, 1], 4, 0, &[2], &[])]
    #[case::giant_both_heads_asked(GIANT, &[0, 0, 1, 1], 4, 0, &[0, 1], &[0, 1])]
    #[case::giant_both_heads_and_an_opponent(GIANT, &[0, 0, 1, 1], 4, 0, &[1, 3], &[1])]
    #[case::giant_the_second_team_second_head(GIANT, &[0, 0, 1, 1], 4, 2, &[3], &[3])]
    #[case::giant_seated_apart_the_second_head(GIANT, &[0, 1, 0, 1], 4, 0, &[2], &[2])]
    #[case::giant_nobody_asked(GIANT, &[0, 0, 1, 1], 4, 0, &[], &[])]
    #[case::shared_the_second_team_member(SHARED, &[0, 0, 1, 1], 4, 0, &[1], &[1])]
    #[case::shared_an_opponent(SHARED, &[0, 0, 1, 1], 4, 0, &[3], &[])]
    #[case::party_the_third_hero(SHARED, &[0, 0, 0, 1], 4, 0, &[2], &[2])]
    #[case::party_every_hero_asked(SHARED, &[0, 0, 0, 1], 4, 0, &[0, 1, 2], &[0, 1, 2])]
    #[case::party_the_boss_decides(SHARED, &[0, 0, 0, 1], 4, 0, &[3], &[])]
    #[case::party_the_boss_turn_its_minion(SHARED, &[0, 0, 0, 1, 1], 5, 3, &[4], &[4])]
    fn the_charged_seats_follow_who_the_game_waits_on(
        #[case] mode: Option<TeamMode>,
        #[case] teams: &[u32],
        #[case] seats: usize,
        #[case] active: usize,
        #[case] waiting: &[usize],
        #[case] expected: &[usize],
    ) {
        let state = state_of(mode, teams, seats, active);
        assert_eq!(who(&state, waiting), expected);
    }
}
