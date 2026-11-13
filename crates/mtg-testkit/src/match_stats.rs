//! Statistics of bot matches as chess engines test them: Elo from a score, its 95% interval, and a sequential
//! probability ratio test (SPRT) that stops a match once it is significant.

/// The Elo difference a score (0 to 1) stands for, clamped to ±800 for a sweep.
pub fn elo(score: f64) -> f64 {
    let score = score.clamp(0.009_9, 0.990_1);
    -400.0 * (1.0 / score - 1.0).log10()
}

/// The expected score of a side `elo` points stronger.
pub fn score_of(elo: f64) -> f64 {
    1.0 / (1.0 + 10f64.powf(-elo / 400.0))
}

/// The mean score per game and its variance over `wins`, `draws` and `losses`.
fn mean_variance(wins: u64, draws: u64, losses: u64) -> Option<(f64, f64, f64)> {
    let games = wins + draws + losses;
    if games == 0 {
        return None;
    }
    let n = count(games);
    let mean = (count(wins) + 0.5 * count(draws)) / n;
    let variance = (count(wins) * (1.0 - mean).powi(2)
        + count(draws) * (0.5 - mean).powi(2)
        + count(losses) * mean.powi(2))
        / n;
    Some((n, mean, variance))
}

/// The Elo difference's 95% interval over `wins`, `draws` and `losses`.
pub fn elo_interval(wins: u64, draws: u64, losses: u64) -> (f64, f64) {
    let Some((n, mean, variance)) = mean_variance(wins, draws, losses) else {
        return (-800.0, 800.0);
    };
    let spread = 1.96 * (variance / n).sqrt();
    (elo(mean - spread), elo(mean + spread))
}

/// A sequential probability ratio test of H0 (the strong side is `elo0` stronger) against H1 (`elo1` stronger), with
/// the generalized log-likelihood ratio over win, draw and loss counts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sprt {
    elo0: f64,
    elo1: f64,
    lower: f64,
    upper: f64,
}

/// Games before the test may stop: a few games give a variance too rough to trust.
const LEAST_GAMES: u64 = 16;

impl Sprt {
    /// The test of `elo0` against `elo1` with false-positive rate `alpha` and false-negative rate `beta`.
    pub fn new(elo0: f64, elo1: f64, alpha: f64, beta: f64) -> Sprt {
        Sprt {
            elo0,
            elo1,
            lower: (beta / (1.0 - alpha)).ln(),
            upper: ((1.0 - beta) / alpha).ln(),
        }
    }

    /// The log-likelihood ratio of H1 over H0 after `wins`, `draws` and `losses`.
    pub fn llr(&self, wins: u64, draws: u64, losses: u64) -> f64 {
        let Some((n, mean, variance)) = mean_variance(wins, draws, losses) else {
            return 0.0;
        };
        let (s0, s1) = (score_of(self.elo0), score_of(self.elo1));
        n * (s1 - s0) * (2.0 * mean - s0 - s1) / (2.0 * variance.max(1e-3))
    }

    /// `Some(true)` once H1 is accepted, `Some(false)` once H0 is, `None` while the match must go on.
    pub fn decide(&self, wins: u64, draws: u64, losses: u64) -> Option<bool> {
        if wins + draws + losses < LEAST_GAMES {
            return None;
        }
        let llr = self.llr(wins, draws, losses);
        if llr >= self.upper {
            Some(true)
        } else if llr <= self.lower {
            Some(false)
        } else {
            None
        }
    }
}

/// A game count as a float.
fn count(n: u64) -> f64 {
    // Exact below 2^53 games.
    #[allow(clippy::cast_precision_loss)]
    let n = n as f64;
    n
}

#[cfg(test)]
mod tests {
    //! Goal: Elo and its interval follow the score, and the SPRT accepts the hypothesis the results come from.
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case(0.5, 0.0)]
    #[case(0.64, 100.0)]
    #[case(0.76, 200.0)]
    fn elo_follows_the_score(#[case] score: f64, #[case] expected: f64) {
        assert!((elo(score) - expected).abs() < 3.0);
        assert!((score_of(elo(score)) - score).abs() < 1e-9);
    }

    #[rstest]
    #[case(30, 10, 20)]
    #[case(100, 0, 100)]
    #[case(5, 5, 1)]
    fn the_interval_holds_the_estimate(#[case] wins: u64, #[case] draws: u64, #[case] losses: u64) {
        let games = wins + draws + losses;
        let score = (count(wins) + 0.5 * count(draws)) / count(games);
        let (low, high) = elo_interval(wins, draws, losses);
        assert!(low < elo(score) && elo(score) < high);
    }

    #[rstest]
    #[case::clearly_stronger(700, 300, Some(true))]
    #[case::even(500, 500, Some(false))]
    #[case::too_few(9, 3, None)]
    fn the_test_accepts_what_the_results_show(
        #[case] wins: u64,
        #[case] losses: u64,
        #[case] expected: Option<bool>,
    ) {
        let sprt = Sprt::new(0.0, 50.0, 0.05, 0.05);
        assert_eq!(sprt.decide(wins, 0, losses), expected);
    }
}
