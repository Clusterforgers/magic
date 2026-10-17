//! A token-bucket rate limiter, one per connection.

use std::time::Instant;

/// Allows `burst` messages at once and `per_second` more every second.
#[derive(Debug)]
pub(crate) struct RateLimiter {
    capacity: f64,
    per_second: f64,
    tokens: f64,
    last: Instant,
}

impl RateLimiter {
    /// A full bucket at time `now`.
    pub(crate) fn new(burst: u32, per_second: u32, now: Instant) -> RateLimiter {
        let capacity = f64::from(burst.max(1));
        RateLimiter {
            capacity,
            per_second: f64::from(per_second),
            tokens: capacity,
            last: now,
        }
    }

    /// Takes one token if there is one; false means the message is over the limit.
    pub(crate) fn allow(&mut self, now: Instant) -> bool {
        let elapsed = now.saturating_duration_since(self.last).as_secs_f64();
        self.last = self.last.max(now);
        self.tokens = (self.tokens + elapsed * self.per_second).min(self.capacity);
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the bucket allows its burst, refills at the configured rate, never holds more than its capacity and tolerates a clock that stands still or steps back.
    use super::*;
    use rstest::rstest;
    use std::time::Duration;

    /// Sends `count` messages at `at_ms` and returns how many were allowed.
    fn send(limiter: &mut RateLimiter, base: Instant, at_ms: u64, count: usize) -> usize {
        let now = base + Duration::from_millis(at_ms);
        (0..count).filter(|_| limiter.allow(now)).count()
    }

    #[rstest]
    #[case(5, 2, &[(0, 8, 5), (500, 4, 1), (1_000, 4, 1), (3_000, 10, 4)])]
    #[case(1, 1, &[(0, 3, 1), (900, 1, 0), (1_100, 1, 1)])]
    #[case(10, 0, &[(0, 20, 10), (60_000, 5, 0)])]
    #[case(3, 100, &[(0, 3, 3), (15, 3, 1), (10_000, 10, 3)])]
    fn buckets_follow_the_schedule(
        #[case] burst: u32,
        #[case] per_second: u32,
        #[case] schedule: &[(u64, usize, usize)],
    ) {
        let base = Instant::now();
        let mut limiter = RateLimiter::new(burst, per_second, base);
        for &(at_ms, count, allowed) in schedule {
            assert_eq!(
                send(&mut limiter, base, at_ms, count),
                allowed,
                "{count} messages at {at_ms} ms"
            );
        }
    }

    #[test]
    fn a_clock_that_steps_back_grants_nothing() {
        let base = Instant::now() + Duration::from_secs(10);
        let mut limiter = RateLimiter::new(2, 1, base);
        assert!(limiter.allow(base));
        assert!(limiter.allow(base));
        assert!(!limiter.allow(base.checked_sub(Duration::from_secs(5)).unwrap()));
        assert!(!limiter.allow(base));
    }

    #[test]
    fn a_zero_burst_still_lets_one_message_through() {
        let base = Instant::now();
        let mut limiter = RateLimiter::new(0, 0, base);
        assert!(limiter.allow(base));
        assert!(!limiter.allow(base));
    }
}
