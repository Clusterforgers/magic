//! Dwell: the wait before the table passes for a seat that cannot act at a window it would be asked at if it could.
//!
//! A seat that can act makes the table wait for a human, so a seat passed for at once would show by its pace that it
//! holds nothing. At a default window ([`crate::windows`]) the table therefore waits a human-like time first and then
//! passes as the seat's own step, batch and save ([`crate::running::Running::fall_back`]), the way it would for a seat
//! that could act and passed. The one default window it spares is the seat's own main phase with an empty stack, where
//! what a seat holds shows anyway (a land or a spell is played in plain sight) and the pass is at once. The wait is wall
//! clock only: the log, the event order and the state hash do not depend on it, and a game resumed from its log starts
//! without one.
//!
//! [`Dwell`] is the setting (`MTG_DWELL_MS`). [`Waits`] draws the waits of one game from an RNG of its own that the
//! operating system seeds, so they depend on neither the game seed nor the state. Every idle seat waits a dwell of its
//! own, drawn independently when it receives priority, whoever passed before it and however: a table of idle seats waits
//! one dwell per seat, and how long a seat waits never tells whether the seats before it could act.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::secret::random_seed;

/// Longest dwell the setting accepts, in milliseconds.
pub const MAX_DWELL_MS: u64 = 5_000;

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

    /// Between 250 and 700 ms, the server default.
    pub const DEFAULT: Dwell = Dwell {
        min_ms: 250,
        max_ms: 700,
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

    /// Reads the text of `MTG_DWELL_MS`: `250-700` is a range, a single number a fixed wait and `0` no wait at all.
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

/// The waits of the idle windows of one game.
#[derive(Debug, Clone)]
pub(crate) struct Waits {
    dwell: Dwell,
    jitter: Jitter,
}

impl Waits {
    /// Waits drawn from `dwell` by a generator that starts from `seed`.
    pub(crate) fn new(dwell: Dwell, seed: u64) -> Waits {
        Waits {
            dwell,
            jitter: Jitter(seed),
        }
    }

    /// Waits drawn from `dwell` by a generator the operating system seeds, independent of the game.
    pub(crate) fn fresh(dwell: Dwell) -> Waits {
        Waits::new(dwell, fresh_seed())
    }

    /// How long a seat that receives priority now, and cannot act, waits before it is passed for: a dwell of its own,
    /// whatever the windows before it were.
    pub(crate) fn wait(&mut self) -> Duration {
        Duration::from_millis(self.jitter.between(self.dwell.min_ms, self.dwell.max_ms))
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the setting reads `250-700`, a single number and `0` and nothing else (nothing above 5000); every wait is a dwell of its own that stays in the range and reaches both ends, the draws are even, independent of each other and of the seed of the game, and none is ever a shorter wait because of the one before it.
    use super::*;
    use rstest::rstest;

    fn ms(span: Duration) -> u64 {
        u64::try_from(span.as_millis()).unwrap()
    }

    #[rstest]
    #[case::the_default("250-700", Dwell::DEFAULT)]
    #[case::spaces(" 250 - 700 ", Dwell::DEFAULT)]
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
        assert_eq!(Dwell::between(700, 250), Dwell::DEFAULT);
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
            (Duration::from_millis(250), Duration::from_millis(700))
        );
    }

    #[rstest]
    #[case(1)]
    #[case(2)]
    #[case(0xDEAD_BEEF)]
    fn a_dwell_stays_in_its_range_and_reaches_both_ends(#[case] seed: u64) {
        let mut waits = Waits::new(Dwell::between(300, 309), seed);
        let drawn: Vec<u64> = (0..400).map(|_| ms(waits.wait())).collect();
        assert!(drawn.iter().all(|ms| (300..=309).contains(ms)), "{drawn:?}");
        assert_eq!(drawn.iter().min(), Some(&300));
        assert_eq!(drawn.iter().max(), Some(&309));
    }

    #[test]
    fn the_default_range_is_covered_evenly() {
        let mut waits = Waits::new(Dwell::DEFAULT, 99);
        let drawn: Vec<u64> = (0..10_000).map(|_| ms(waits.wait())).collect();
        assert!(drawn.iter().all(|ms| (250..=700).contains(ms)));
        let mean = drawn.iter().sum::<u64>() / 10_000;
        assert!((465..=485).contains(&mean), "the mean is {mean}");
        for third in [(250, 399), (400, 549), (550, 700)] {
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
    fn a_wait_is_independent_of_the_one_before_it() {
        // The same value twice in a row happens about once in 451 draws; a wait that followed the one before it
        // (a shorter one behind a dwell, say) would repeat or fall short far more often.
        let mut waits = Waits::new(Dwell::DEFAULT, 7);
        let drawn: Vec<u64> = (0..20_000).map(|_| ms(waits.wait())).collect();
        let repeats = drawn.windows(2).filter(|pair| pair[0] == pair[1]).count();
        assert!(repeats < 100, "{repeats} repeats in 20000 draws");
        let shorter = drawn.windows(2).filter(|pair| pair[1] < pair[0]).count();
        assert!(
            (9_000..=11_000).contains(&shorter),
            "a wait is shorter than the one before it about half the time: {shorter}"
        );
    }

    #[test]
    fn a_fixed_dwell_is_always_the_same_and_off_is_never_a_wait() {
        let mut fixed = Waits::new(Dwell::fixed(450), 5);
        let mut off = Waits::new(Dwell::OFF, 5);
        for _ in 0..50 {
            assert_eq!(fixed.wait(), Duration::from_millis(450));
            assert_eq!(off.wait(), Duration::ZERO);
        }
    }

    #[test]
    fn the_same_seed_draws_the_same_waits_and_another_seed_other_ones() {
        let draw = |seed| {
            let mut waits = Waits::new(Dwell::DEFAULT, seed);
            (0..12).map(|_| waits.wait()).collect::<Vec<_>>()
        };
        assert_eq!(draw(7), draw(7));
        assert_ne!(draw(7), draw(8));
    }
}
