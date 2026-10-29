//! The status line of a run: cards done and failed, the verdicts, requests, tokens and the pace with an estimate of the time left.

use std::time::Duration;

use super::budget::Budget;
use super::outcome::Tally;

/// One line: cards done, failed and in total, the cards by checker verdict, requests, tokens, tokens per second, cards
/// per minute and the time left at that pace. `total` counts the cards that need a request (those `--resume` skips are
/// not in it); a card the budget kept from running is neither done nor left.
pub fn status_line(tally: &Tally, budget: &Budget, total: usize, elapsed: Duration) -> String {
    let done = tally.total().saturating_sub(tally.skipped + tally.not_run);
    let not_run = if tally.not_run == 0 {
        String::new()
    } else {
        format!(", {} not run", tally.not_run)
    };
    let usage = budget.usage();
    let millis = elapsed.as_millis().max(1);
    let tokens_per_second = tenths(u128::from(usage.output_tokens) * 10_000 / millis);
    let cards_per_minute = tenths(done_as_u128(done) * 600_000 / millis);
    let eta = if done == 0 {
        "unknown".to_owned()
    } else {
        let left = done_as_u128(total.saturating_sub(done + tally.not_run));
        clock(left * millis / done_as_u128(done) / 1000)
    };
    format!(
        "status: {done}/{total} cards done, {} failed{not_run}; Verified {}, Partial {}, Unchecked {}, Mismatch {}, Invalid {}, Empty {}; \
         {} requests; tokens in {} (cached {}), out {}; {tokens_per_second} tok/s, {cards_per_minute} cards/min; ETA {eta}",
        tally.failed,
        tally.verified,
        tally.partial,
        tally.unchecked,
        tally.mismatch,
        tally.invalid,
        tally.empty,
        budget.requests(),
        usage.input_tokens,
        usage.cache_read_tokens,
        usage.output_tokens,
    )
}

fn done_as_u128(count: usize) -> u128 {
    u128::try_from(count).unwrap_or(u128::MAX)
}

/// A number given in tenths, such as 523, as `52.3`.
fn tenths(value: u128) -> String {
    format!("{}.{}", value / 10, value % 10)
}

/// A span of seconds as `45s`, `3m 05s` or `2h 07m`.
fn clock(seconds: u128) -> String {
    match seconds {
        0..60 => format!("{seconds}s"),
        60..3600 => format!("{}m {:02}s", seconds / 60, seconds % 60),
        _ => format!("{}h {:02}m", seconds / 3600, seconds % 3600 / 60),
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the status line shows the counts, the tokens and the pace, with an estimate of the time left that is
    //! unknown before a card is done and zero at the end.
    use rstest::rstest;

    use super::*;
    use crate::propose::model::Usage;

    fn budget(requests: u32, usage: Usage) -> Budget {
        let mut budget = Budget::new(1000, u64::MAX);
        for _ in 0..requests {
            let request = crate::propose::model::Request {
                model: String::new(),
                system: String::new(),
                messages: Vec::new(),
                max_tokens: 1,
            };
            assert!(budget.admit(&request, false).is_ok());
        }
        budget.record(usage);
        budget
    }

    #[rstest]
    #[case::seconds(45, "45s")]
    #[case::minutes(185, "3m 05s")]
    #[case::hours(7_620, "2h 07m")]
    fn a_span_reads_as_the_largest_units(#[case] seconds: u128, #[case] text: &str) {
        assert_eq!(clock(seconds), text);
    }

    #[test]
    fn the_line_has_the_counts_the_tokens_and_the_pace() {
        let tally = Tally {
            verified: 3,
            partial: 1,
            unchecked: 4,
            mismatch: 1,
            invalid: 2,
            failed: 1,
            ..Tally::default()
        };
        let usage = Usage {
            input_tokens: 340_000,
            cache_read_tokens: 320_000,
            output_tokens: 6_000,
        };
        // 12 of 60 cards in 10 minutes: 1.2 cards a minute, 40 minutes left; 6000 tokens in 600 s.
        let line = status_line(&tally, &budget(17, usage), 60, Duration::from_secs(600));
        assert_eq!(
            line,
            "status: 12/60 cards done, 1 failed; Verified 3, Partial 1, Unchecked 4, Mismatch 1, Invalid 2, Empty 0; \
             17 requests; tokens in 340000 (cached 320000), out 6000; 10.0 tok/s, 1.2 cards/min; ETA 40m 00s"
        );
    }

    #[test]
    fn cards_the_budget_kept_from_running_are_counted_apart() {
        let tally = Tally {
            verified: 5,
            not_run: 7,
            ..Tally::default()
        };
        let line = status_line(
            &tally,
            &budget(5, Usage::default()),
            12,
            Duration::from_secs(60),
        );
        assert!(
            line.contains("5/12 cards done, 0 failed, 7 not run;"),
            "{line}"
        );
        assert!(line.ends_with("ETA 0s"), "{line}");
    }

    #[rstest]
    #[case::nothing_done_yet(0, 0, "ETA unknown")]
    #[case::half_done(5, 10, "ETA 1m 00s")]
    #[case::all_done(10, 10, "ETA 0s")]
    fn the_time_left_follows_the_pace(
        #[case] verified: usize,
        #[case] total: usize,
        #[case] eta: &str,
    ) {
        let tally = Tally {
            verified,
            skipped: 7,
            ..Tally::default()
        };
        // Skipped cards are not counted as done: 5 cards in 60 s.
        let line = status_line(
            &tally,
            &budget(0, Usage::default()),
            total,
            Duration::from_secs(60),
        );
        assert!(line.ends_with(eta), "{line}");
    }
}
