//! Dwell: the wait before the table passes for a seat that cannot act at a window it would be asked at if it could.
//!
//! A seat that can act makes the table wait for a human, so a seat passed for at once would show by its pace that it
//! holds nothing. At a default window ([`crate::windows`]) the table therefore waits a human-like time first and then
//! passes as the seat's own step, batch and save ([`crate::running::Running::fall_back`]), the way it would for a seat
//! that could act and passed. The one default window it spares is the seat's own main phase with an empty stack, where
//! what a seat holds shows anyway (a land or a spell is played in plain sight) and the pass is at once. The wait is wall
//! clock only: the log, the event order and the state hash do not depend on
//! it, and a game resumed from its log starts without one.
//!
//! [`Dwell`] is the setting (`MTG_DWELL_MS`). [`Waits`] draws the waits of one game from an RNG of its own that the
//! operating system seeds, so they depend on neither the game seed nor the state. Idle windows that follow one another
//! in one step without a seat acting between them are a round: the first waits a dwell, the others a short stagger,
//! and the whole round waits at most [`ROUND_CAP_MS`], so a table of idle seats does not add up per seat.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use mtg_engine::turn::Step;

use crate::secret::random_seed;

/// Longest dwell the setting accepts, in milliseconds.
pub const MAX_DWELL_MS: u64 = 5_000;

/// Shortest and longest wait, in milliseconds, before each pass of a round after its first.
const STAGGER_MS: (u64, u64) = (80, 250);

/// Most a round of passes waits in all, in milliseconds. The first dwell is never cut by it.
const ROUND_CAP_MS: u64 = 1_200;

/// How long the table waits before it passes for a seat that cannot act at a window of the default table
/// (`MTG_DWELL_MS`): between a shortest and a longest wait, drawn uniformly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dwell {
    min_ms: u64,
    max_ms: u64,
}

impl Dwell {
    /// No wait: an idle window is passed for at once.
    pub const OFF: Dwell = Dwell {
        min_ms: 0,
        max_ms: 0,
    };

    /// Between 300 and 900 ms, the server default.
    pub const DEFAULT: Dwell = Dwell {
        min_ms: 300,
        max_ms: 900,
    };

    /// The same wait every time (`0` is [`Dwell::OFF`]); above [`MAX_DWELL_MS`] it is lowered to it.
    pub const fn fixed(ms: u64) -> Dwell {
        Dwell::between(ms, ms)
    }

    /// A wait drawn between `min_ms` and `max_ms`, the bounds put in order and lowered to [`MAX_DWELL_MS`].
    pub const fn between(min_ms: u64, max_ms: u64) -> Dwell {
        let (low, high) = if min_ms <= max_ms {
            (min_ms, max_ms)
        } else {
            (max_ms, min_ms)
        };
        Dwell {
            min_ms: if low > MAX_DWELL_MS {
                MAX_DWELL_MS
            } else {
                low
            },
            max_ms: if high > MAX_DWELL_MS {
                MAX_DWELL_MS
            } else {
                high
            },
        }
    }

    /// Reads the text of `MTG_DWELL_MS`: `300-900` is a range, a single number a fixed wait and `0` no wait at all.
    ///
    /// `None` for anything else: a bound that is not a whole number of milliseconds, a range that runs backwards and
    /// nothing above [`MAX_DWELL_MS`].
    pub fn parse(text: &str) -> Option<Dwell> {
        let text = text.trim();
        let (min_ms, max_ms) = if let Some((low, high)) = text.split_once('-') {
            (milliseconds(low)?, milliseconds(high)?)
        } else {
            let ms = milliseconds(text)?;
            (ms, ms)
        };
        (min_ms <= max_ms && max_ms <= MAX_DWELL_MS).then_some(Dwell { min_ms, max_ms })
    }

    /// Whether idle windows are passed for at once.
    pub const fn is_off(self) -> bool {
        self.max_ms == 0
    }

    /// The shortest and the longest wait.
    pub const fn range(self) -> (Duration, Duration) {
        (
            Duration::from_millis(self.min_ms),
            Duration::from_millis(self.max_ms),
        )
    }

    /// The bounds, in milliseconds, of the stagger before a pass after the first of a round: never longer than the
    /// longest dwell, so a short dwell does not get a longer stagger.
    const fn stagger(self) -> (u64, u64) {
        let longest = if STAGGER_MS.1 < self.max_ms {
            STAGGER_MS.1
        } else {
            self.max_ms
        };
        let shortest = if STAGGER_MS.0 < longest {
            STAGGER_MS.0
        } else {
            longest
        };
        (shortest, longest)
    }
}

/// A whole number of milliseconds written in digits only.
fn milliseconds(text: &str) -> Option<u64> {
    let text = text.trim();
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

/// `SplitMix64`: a small generator that is good enough to jitter a wait; it guards nothing.
#[derive(Debug, Clone)]
struct Jitter(u64);

impl Jitter {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `low..=high`; the span is at most 5001, so the bias of the remainder is far below anything seen.
    fn between(&mut self, low: u64, high: u64) -> u64 {
        if high <= low {
            return low;
        }
        low + self.next() % (high - low + 1)
    }
}

/// A seed for a game's jitter: from the operating system, else from the clock.
fn fresh_seed() -> u64 {
    random_seed().unwrap_or_else(|_| {
        let since_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        u64::try_from(since_epoch.as_nanos()).unwrap_or(u64::MAX)
    })
}

/// Where a round of idle windows is: the turn number and the step.
pub(crate) type Moment = (u32, Step);

/// The round in progress.
#[derive(Debug, Clone, Copy)]
struct Round {
    at: Moment,
    /// Milliseconds the round has waited so far.
    waited_ms: u64,
}

/// The waits of the idle windows of one game.
#[derive(Debug, Clone)]
pub(crate) struct Waits {
    dwell: Dwell,
    jitter: Jitter,
    round: Option<Round>,
}

impl Waits {
    /// Waits drawn from `dwell` by a generator that starts from `seed`.
    pub(crate) fn new(dwell: Dwell, seed: u64) -> Waits {
        Waits {
            dwell,
            jitter: Jitter(seed),
            round: None,
        }
    }

    /// Waits drawn from `dwell` by a generator the operating system seeds, independent of the game.
    pub(crate) fn fresh(dwell: Dwell) -> Waits {
        Waits::new(dwell, fresh_seed())
    }

    /// How long to wait before passing at an idle window at `at`.
    ///
    /// With `continues` (the window follows a pass that was itself a dwell's) and the round still at `at`, this is a
    /// stagger, cut so that the round never waits more than [`ROUND_CAP_MS`] in all. Otherwise a new round begins and
    /// this is a dwell.
    pub(crate) fn wait(&mut self, at: Moment, continues: bool) -> Duration {
        if continues && let Some(round) = self.round.as_mut().filter(|round| round.at == at) {
            let (low, high) = self.dwell.stagger();
            let left = ROUND_CAP_MS.saturating_sub(round.waited_ms);
            let ms = self.jitter.between(low, high).min(left);
            round.waited_ms += ms;
            return Duration::from_millis(ms);
        }
        let ms = self.jitter.between(self.dwell.min_ms, self.dwell.max_ms);
        self.round = Some(Round { at, waited_ms: ms });
        Duration::from_millis(ms)
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the setting reads `300-900`, a single number and `0` and nothing else (nothing above 5000); waits stay in their range and reach both ends; the first idle window of a round waits a dwell, the next ones a short stagger, and a round never waits more than its cap in all, whatever the seed.
    use super::*;
    use rstest::rstest;

    const AT: Moment = (3, Step::Main1);
    const ELSEWHERE: Moment = (3, Step::DeclareAttackers);

    fn ms(span: Duration) -> u64 {
        u64::try_from(span.as_millis()).unwrap()
    }

    #[rstest]
    #[case::the_default("300-900", Dwell::DEFAULT)]
    #[case::spaces(" 300 - 900 ", Dwell::DEFAULT)]
    #[case::a_fixed_wait("500", Dwell::fixed(500))]
    #[case::off("0", Dwell::OFF)]
    #[case::an_empty_range("0-0", Dwell::OFF)]
    #[case::from_nothing("0-250", Dwell::between(0, 250))]
    #[case::the_most_accepted("5000", Dwell::fixed(5_000))]
    #[case::the_widest_range("0-5000", Dwell::between(0, 5_000))]
    #[case::one_value_range("700-700", Dwell::fixed(700))]
    fn the_setting_reads_a_range_a_fixed_wait_and_off(#[case] text: &str, #[case] expected: Dwell) {
        assert_eq!(Dwell::parse(text), Some(expected));
    }

    #[rstest]
    #[case::nothing("")]
    #[case::blank("  ")]
    #[case::above_the_most("5001")]
    #[case::range_above_the_most("300-5001")]
    #[case::backwards("900-300")]
    #[case::negative("-5")]
    #[case::no_low_bound("-900")]
    #[case::no_high_bound("300-")]
    #[case::three_bounds("300-600-900")]
    #[case::words("fast")]
    #[case::seconds("0.5")]
    #[case::a_sign("+300")]
    #[case::unit("300ms")]
    #[case::too_large_for_a_number("99999999999999999999999")]
    fn anything_else_is_refused(#[case] text: &str) {
        assert_eq!(Dwell::parse(text), None);
    }

    #[test]
    fn constructors_put_the_bounds_in_order_and_cap_them() {
        assert_eq!(Dwell::between(900, 300), Dwell::DEFAULT);
        assert_eq!(Dwell::fixed(9_000), Dwell::fixed(MAX_DWELL_MS));
        assert_eq!(
            Dwell::between(6_000, 7_000),
            Dwell::fixed(MAX_DWELL_MS),
            "both bounds are lowered"
        );
        assert!(Dwell::OFF.is_off());
        assert!(Dwell::fixed(0).is_off());
        assert!(!Dwell::fixed(1).is_off());
        assert!(!Dwell::DEFAULT.is_off());
        assert_eq!(
            Dwell::DEFAULT.range(),
            (Duration::from_millis(300), Duration::from_millis(900))
        );
    }

    #[rstest]
    #[case(1)]
    #[case(2)]
    #[case(0xDEAD_BEEF)]
    fn a_dwell_stays_in_its_range_and_reaches_both_ends(#[case] seed: u64) {
        let mut waits = Waits::new(Dwell::between(300, 309), seed);
        let drawn: Vec<u64> = (0..400)
            .map(|turn| ms(waits.wait((turn, Step::Main1), false)))
            .collect();
        assert!(drawn.iter().all(|ms| (300..=309).contains(ms)), "{drawn:?}");
        assert_eq!(drawn.iter().min(), Some(&300));
        assert_eq!(drawn.iter().max(), Some(&309));
    }

    #[test]
    fn the_default_range_is_covered_evenly() {
        let mut waits = Waits::new(Dwell::DEFAULT, 99);
        let drawn: Vec<u64> = (0..10_000)
            .map(|turn| ms(waits.wait((turn, Step::Main1), false)))
            .collect();
        assert!(drawn.iter().all(|ms| (300..=900).contains(ms)));
        let mean = drawn.iter().sum::<u64>() / 10_000;
        assert!((590..=610).contains(&mean), "the mean is {mean}");
        for third in [(300, 499), (500, 699), (700, 900)] {
            let inside = drawn
                .iter()
                .filter(|ms| (third.0..=third.1).contains(*ms))
                .count();
            assert!(
                (3_000..=3_700).contains(&inside),
                "{inside} of 10000 are in {third:?}"
            );
        }
    }

    #[test]
    fn a_fixed_dwell_is_always_the_same_and_off_is_never_a_wait() {
        let mut fixed = Waits::new(Dwell::fixed(450), 5);
        let mut off = Waits::new(Dwell::OFF, 5);
        for turn in 0..50 {
            assert_eq!(fixed.wait((turn, AT.1), false), Duration::from_millis(450));
            assert_eq!(off.wait((turn, AT.1), false), Duration::ZERO);
        }
    }

    #[test]
    fn the_same_seed_draws_the_same_waits_and_another_seed_other_ones() {
        let draw = |seed| {
            let mut waits = Waits::new(Dwell::DEFAULT, seed);
            (0..12)
                .map(|turn| waits.wait((turn, AT.1), false))
                .collect::<Vec<_>>()
        };
        assert_eq!(draw(7), draw(7));
        assert_ne!(draw(7), draw(8));
    }

    #[rstest]
    #[case(1)]
    #[case(2)]
    #[case(3)]
    #[case(4)]
    fn a_round_waits_one_dwell_then_staggers_and_never_more_than_its_cap(#[case] seed: u64) {
        let mut waits = Waits::new(Dwell::DEFAULT, seed);
        let first = ms(waits.wait(AT, false));
        assert!((300..=900).contains(&first), "{first}");
        let mut total = first;
        for _ in 0..40 {
            let room = ROUND_CAP_MS - total;
            let wait = ms(waits.wait(AT, true));
            assert!(wait <= 250.min(room), "{wait} with {room} left");
            assert!(wait >= 80.min(room), "{wait} with {room} left");
            total += wait;
        }
        assert_eq!(total, ROUND_CAP_MS, "forty staggers use the cap up");
        assert_eq!(
            ms(waits.wait(AT, true)),
            0,
            "past the cap the passes are not delayed"
        );
    }

    #[test]
    fn the_first_stagger_of_a_round_fits_the_cap_for_the_longest_dwell() {
        // After 900 ms, 300 are left: a stagger of 80 to 250 is not cut.
        let mut waits = Waits::new(Dwell::fixed(900), 11);
        assert_eq!(waits.wait(AT, false), Duration::from_millis(900));
        let stagger = ms(waits.wait(AT, true));
        assert!((80..=250).contains(&stagger), "{stagger}");
    }

    #[rstest]
    #[case::short(100, 80, 100)]
    #[case::shorter_than_the_shortest_stagger(50, 50, 50)]
    #[case::as_long_as_the_longest_stagger(250, 80, 250)]
    #[case::longer_than_the_cap(2_000, 0, 0)]
    fn a_stagger_follows_a_short_dwell_and_the_cap(
        #[case] fixed: u64,
        #[case] low: u64,
        #[case] high: u64,
    ) {
        let mut waits = Waits::new(Dwell::fixed(fixed), 3);
        waits.wait(AT, false);
        let stagger = ms(waits.wait(AT, true));
        assert!(
            (low..=high).contains(&stagger),
            "{stagger} after a dwell of {fixed}"
        );
    }

    #[test]
    fn a_round_ends_when_the_game_moves_on_or_a_seat_acts() {
        let mut waits = Waits::new(Dwell::fixed(600), 4);
        assert_eq!(waits.wait(AT, false), Duration::from_millis(600));
        assert!(
            ms(waits.wait(AT, true)) <= 250,
            "same step, a pass followed a pass"
        );
        assert_eq!(
            waits.wait(ELSEWHERE, true),
            Duration::from_millis(600),
            "another step is a new round even right after a pass"
        );
        assert_eq!(
            waits.wait((4, Step::DeclareAttackers), true),
            Duration::from_millis(600),
            "and so is the same step of another turn"
        );
        assert!(ms(waits.wait((4, Step::DeclareAttackers), true)) <= 250);
        assert_eq!(
            waits.wait((4, Step::DeclareAttackers), false),
            Duration::from_millis(600),
            "a seat acting starts a new round in the same step"
        );
    }

    #[test]
    fn a_dwell_that_is_off_never_staggers_either() {
        let mut waits = Waits::new(Dwell::OFF, 1);
        for _ in 0..5 {
            assert_eq!(waits.wait(AT, true), Duration::ZERO);
        }
    }
}
