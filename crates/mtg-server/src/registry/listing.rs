//! What the game browser may see of the registry: public games that are still in their lobby.

use std::net::IpAddr;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use mtg_core::house::HouseRules;

use super::Registry;
use crate::game_id::GameId;
use crate::handle::GameHandle;
use crate::recipe::FormatKind;
use crate::records::{SeatState, Visibility};
use crate::throttle::Blocked;

/// Most games [`Registry::lobbies`] returns: the newest.
pub const MAX_LISTED_LOBBIES: usize = 50;

/// A public game in its lobby, as the game browser lists it. There is no token, join code or seat
/// link in it: the id is all a visitor needs to ask to join, and only a public game lets them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedGame {
    /// The game.
    pub game: GameId,
    /// What the host, who holds seat 0, is called.
    pub host: String,
    /// The rules it is played under.
    pub format: FormatKind,
    /// What the creator set in place of the format's values.
    pub house_rules: HouseRules,
    /// Seats somebody holds, the host's included.
    pub filled: usize,
    /// Seats in all; a lobby with `filled == total` is full and waits for everybody to be ready.
    pub total: usize,
    /// When the game was created, as `YYYY-MM-DDTHH:MM:SSZ` (UTC).
    pub created_at: String,
    /// Time since the game was created.
    pub age: Duration,
}

impl Registry {
    /// Counts a request for the lobby list from client `address` at time `now`. An address that
    /// has asked `AttemptLimits::lobby_requests_per_ip` times inside `lobby_window` is turned away
    /// with [`Blocked`] until the window, which began at its first request, is over. Every request
    /// counts, answered or not.
    pub fn admit_listing(&self, address: IpAddr, now: Instant) -> Result<(), Blocked> {
        self.shared.throttle.begin_listing(now, address)
    }

    /// Counts a request for the precon list or one precon from client `address` at time `now`
    /// against `AttemptLimits::precon_requests_per_ip` inside `lobby_window`; every request counts.
    pub fn admit_precons(&self, address: IpAddr, now: Instant) -> Result<(), Blocked> {
        self.shared.throttle.begin_precons(now, address)
    }

    /// The public games that are still in their lobby, newest first, at most
    /// [`MAX_LISTED_LOBBIES`]. Invite games, started games and games whose actor is gone are
    /// never in it.
    pub async fn lobbies(&self) -> Vec<ListedGame> {
        let (now, wall) = (Instant::now(), SystemTime::now());
        let mut candidates: Vec<(Instant, GameHandle, ListedGame)> = self
            .games()
            .iter()
            .filter(|(_, entry)| entry.visibility == Visibility::Public)
            .map(|(id, entry)| {
                let age = now.saturating_duration_since(entry.created);
                let listed = ListedGame {
                    game: *id,
                    host: entry
                        .seats
                        .first()
                        .map(|s| s.name.clone())
                        .unwrap_or_default(),
                    format: entry.format,
                    house_rules: entry.house_rules,
                    filled: entry
                        .seats
                        .iter()
                        .filter(|slot| slot.state != SeatState::Open)
                        .count(),
                    total: entry.seats.len(),
                    created_at: utc_stamp(wall.checked_sub(age).unwrap_or(UNIX_EPOCH)),
                    age,
                };
                (entry.created, entry.handle.clone(), listed)
            })
            .collect();
        // Newest first; two games made in the same instant keep a fixed order by id.
        candidates.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.2.game.cmp(&a.2.game)));

        let mut listed = Vec::new();
        for (_, handle, game) in candidates {
            if listed.len() == MAX_LISTED_LOBBIES {
                break;
            }
            if matches!(handle.status().await, Ok(status) if !status.started) {
                listed.push(game);
            }
        }
        listed
    }
}

/// `time` as `YYYY-MM-DDTHH:MM:SSZ` (UTC), the layout of the other timestamps on the wire.
fn utc_stamp(time: SystemTime) -> String {
    let seconds = time.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let (days, rest) = (seconds / 86_400, seconds % 86_400);
    let (year, month, day) = civil_from_days(i64::try_from(days).unwrap_or(0));
    let (hour, minute, second) = (rest / 3_600, rest % 3_600 / 60, rest % 60);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// The calendar date (year, month, day) of a day counted from 1970-01-01, in the proleptic
/// Gregorian calendar.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_from_march = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_from_march + 2) / 5 + 1;
    let month = if month_from_march < 10 {
        month_from_march + 3
    } else {
        month_from_march - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (
        year,
        u32::try_from(month).unwrap_or(1),
        u32::try_from(day).unwrap_or(1),
    )
}

#[cfg(test)]
mod tests {
    //! Goal: the creation time of a listed game is written as the UTC timestamp the rest of the wire uses, for every kind of day (the epoch, leap days, century years, the end of the supported range) and a clock before the epoch counts as the epoch.
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::the_epoch(0, "1970-01-01T00:00:00Z")]
    #[case::a_leap_day(951_782_400, "2000-02-29T00:00:00Z")]
    #[case::the_billionth_second(1_000_000_000, "2001-09-09T01:46:40Z")]
    #[case::late_2023(1_700_000_000, "2023-11-14T22:13:20Z")]
    #[case::a_century_year_that_is_not_a_leap_year(4_102_444_800, "2100-01-01T00:00:00Z")]
    #[case::the_start_of_a_month(1_782_864_000, "2026-07-01T00:00:00Z")]
    #[case::an_ordinary_time(1_791_990_123, "2026-10-14T15:02:03Z")]
    #[case::the_end_of_four_digit_years(253_402_300_799, "9999-12-31T23:59:59Z")]
    fn seconds_since_the_epoch_become_a_utc_stamp(#[case] seconds: u64, #[case] expected: &str) {
        let time = UNIX_EPOCH + Duration::from_secs(seconds);
        assert_eq!(utc_stamp(time), expected);
    }

    #[test]
    fn a_clock_before_the_epoch_counts_as_the_epoch() {
        let before = UNIX_EPOCH - Duration::from_secs(10);
        assert_eq!(utc_stamp(before), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn every_day_of_a_leap_cycle_has_a_valid_date() {
        // 400 years of days: months and days stay in range and each date follows the last.
        let mut previous = civil_from_days(-1);
        for day in 0..146_097 {
            let (year, month, date) = civil_from_days(day);
            assert!((1..=12).contains(&month) && (1..=31).contains(&date));
            let next = (year, month, date) > previous;
            assert!(next, "{previous:?} then {:?}", (year, month, date));
            previous = (year, month, date);
        }
        assert_eq!(civil_from_days(146_097), (2370, 1, 1));
    }
}
