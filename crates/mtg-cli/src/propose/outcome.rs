//! What became of each card and the summary table of a run.

use std::fmt::Write as _;
use std::path::PathBuf;

use mtg_cards::proposal::{Report, Verdict};
use mtg_cards::support::Support;

use super::budget::{Budget, Stop};

/// What became of one card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Accepted and the engine plays all of it.
    Verified,
    /// Accepted with some clauses left to the table.
    Partial,
    /// Valid, but a construct has no writer, so it cannot be read back.
    Unchecked,
    /// An ability is written back as other text than the card's, or has no sentence.
    Mismatch,
    /// The JSON does not parse or fails a check of a rules file.
    Invalid,
    /// Accepted, but nothing is modelled: no gain.
    Empty,
    /// The API gave no answer.
    Failed(String),
    /// The budget ended the run before the card.
    NotRun,
}

impl Outcome {
    /// The outcome of a checker report.
    pub fn of(report: &Report) -> Outcome {
        match report.verdict() {
            Verdict::Invalid => Outcome::Invalid,
            Verdict::Mismatch => Outcome::Mismatch,
            Verdict::Unchecked => Outcome::Unchecked,
            Verdict::Accepted => match report.support.as_ref().map(|record| record.support) {
                Some(Support::Verified) => Outcome::Verified,
                Some(Support::Partial) => Outcome::Partial,
                _ => Outcome::Empty,
            },
        }
    }

    /// Whether the proposal is kept as an accepted file.
    pub fn is_kept(&self) -> bool {
        matches!(self, Outcome::Verified | Outcome::Partial)
    }

    /// Whether one repair round may fix it.
    pub fn is_repairable(&self) -> bool {
        matches!(self, Outcome::Invalid | Outcome::Mismatch)
    }

    /// The outcome as a short phrase.
    pub fn word(&self) -> &'static str {
        match self {
            Outcome::Verified => "accepted Verified",
            Outcome::Partial => "accepted Partial",
            Outcome::Unchecked => "unchecked",
            Outcome::Mismatch => "mismatch",
            Outcome::Invalid => "invalid",
            Outcome::Empty => "accepted but empty",
            Outcome::Failed(_) => "no answer",
            Outcome::NotRun => "not run",
        }
    }
}

/// How many cards ended each way.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tally {
    /// Accepted `Verified`.
    pub verified: usize,
    /// Accepted `Partial`.
    pub partial: usize,
    /// Unchecked.
    pub unchecked: usize,
    /// Mismatch.
    pub mismatch: usize,
    /// Invalid.
    pub invalid: usize,
    /// Accepted with nothing modelled.
    pub empty: usize,
    /// No answer from the API.
    pub failed: usize,
    /// Skipped by `--resume`.
    pub skipped: usize,
    /// Not run for lack of budget.
    pub not_run: usize,
}

impl Tally {
    /// Counts one card.
    pub fn add(&mut self, outcome: &Outcome) {
        let counter = match outcome {
            Outcome::Verified => &mut self.verified,
            Outcome::Partial => &mut self.partial,
            Outcome::Unchecked => &mut self.unchecked,
            Outcome::Mismatch => &mut self.mismatch,
            Outcome::Invalid => &mut self.invalid,
            Outcome::Empty => &mut self.empty,
            Outcome::Failed(_) => &mut self.failed,
            Outcome::NotRun => &mut self.not_run,
        };
        *counter += 1;
    }

    /// The cards counted.
    pub fn total(&self) -> usize {
        self.verified
            + self.partial
            + self.unchecked
            + self.mismatch
            + self.invalid
            + self.empty
            + self.failed
            + self.skipped
            + self.not_run
    }
}

/// The result of a run.
#[derive(Debug, Clone)]
pub struct ProposeReport {
    /// The model asked.
    pub model: String,
    /// How the cards ended.
    pub tally: Tally,
    /// What was spent.
    pub budget: Budget,
    /// The budget that ended the run early.
    pub stopped: Option<Stop>,
    /// Where the proposals are.
    pub out: PathBuf,
}

impl ProposeReport {
    /// The summary table.
    pub fn render(&self) -> String {
        let tally = &self.tally;
        let rows = [
            ("Accepted Verified", tally.verified),
            ("Accepted Partial", tally.partial),
            ("Unchecked", tally.unchecked),
            ("Mismatch", tally.mismatch),
            ("Invalid", tally.invalid),
            ("Accepted, nothing modelled", tally.empty),
            ("No answer", tally.failed),
            ("Skipped (resume)", tally.skipped),
            ("Not run (budget)", tally.not_run),
        ];
        let mut text = format!("propose: {} cards, model {}\n", tally.total(), self.model);
        for (label, count) in rows {
            let _ = writeln!(text, "  {label:<28}{count:>6}");
        }
        let usage = self.budget.usage();
        let _ = writeln!(
            text,
            "requests {} of {} ({} repair rounds); input tokens {} of {} ({} read from the cache); output tokens {}",
            self.budget.requests(),
            self.budget.max_requests(),
            self.budget.repairs(),
            usage.input_tokens,
            self.budget.max_input_tokens(),
            usage.cache_read_tokens,
            usage.output_tokens
        );
        if let Some(stop) = self.stopped {
            let _ = writeln!(text, "stopped: {stop}; `--resume` continues");
        }
        let _ = write!(text, "proposals in {}", self.out.display());
        text
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every outcome is counted once, the summary lists each row with the requests and tokens spent, and the budget
    //! that ended a run is named.
    use mtg_cards::card::{Card, ImplStatus, Span};
    use mtg_cards::compile::ClauseKind;
    use mtg_cards::load::LoadError;
    use mtg_cards::proposal::{ClauseReport, ClauseState};
    use mtg_cards::support::SupportRecord;
    use rstest::rstest;

    use super::*;
    use crate::propose::model::Usage;

    #[test]
    fn each_outcome_is_counted_once() {
        let mut tally = Tally::default();
        let outcomes = [
            Outcome::Verified,
            Outcome::Verified,
            Outcome::Partial,
            Outcome::Unchecked,
            Outcome::Mismatch,
            Outcome::Invalid,
            Outcome::Empty,
            Outcome::Failed("x".into()),
            Outcome::NotRun,
        ];
        for outcome in &outcomes {
            tally.add(outcome);
        }
        tally.skipped = 2;
        assert_eq!(tally.total(), outcomes.len() + 2);
        assert_eq!((tally.verified, tally.partial, tally.not_run), (2, 1, 1));
    }

    fn report(state: Option<ClauseState>, problems: bool, support: Option<Support>) -> Report {
        Report {
            card: "Card".into(),
            problems: if problems {
                vec![LoadError {
                    file: "f".into(),
                    path: "p".into(),
                    message: "m".into(),
                }]
            } else {
                Vec::new()
            },
            clauses: state
                .into_iter()
                .map(|state| ClauseReport {
                    face: 0,
                    span: Span { start: 0, end: 1 },
                    text: "x".into(),
                    kind: ClauseKind::Static,
                    state,
                })
                .collect(),
            strays: Vec::new(),
            support: support.map(|support| SupportRecord {
                oracle_id: "id".into(),
                support,
                clauses: Vec::new(),
                fingerprint: String::new(),
            }),
            rules: Some(Card {
                oracle_id: "id".into(),
                name: "Card".into(),
                impl_status: ImplStatus::Full,
                faces: Vec::new(),
                notes: None,
            }),
        }
    }

    #[rstest]
    #[case::invalid(report(None, true, None), Outcome::Invalid, true)]
    #[case::mismatch(
        report(Some(ClauseState::Differs { ability: 0, written: "w".into() }), false, Some(Support::Verified)),
        Outcome::Mismatch,
        true
    )]
    #[case::unchecked(
        report(Some(ClauseState::Unwritten { ability: 0, construct: "c".into() }), false, Some(Support::Verified)),
        Outcome::Unchecked,
        false
    )]
    #[case::verified(report(Some(ClauseState::Reads { ability: 0 }), false, Some(Support::Verified)), Outcome::Verified, false)]
    #[case::partial(
        report(Some(ClauseState::Open), false, Some(Support::Partial)),
        Outcome::Partial,
        false
    )]
    #[case::nothing_modelled(
        report(Some(ClauseState::Open), false, Some(Support::Manual)),
        Outcome::Empty,
        false
    )]
    fn a_report_is_an_outcome_and_only_invalid_and_mismatch_are_repaired(
        #[case] report: Report,
        #[case] expected: Outcome,
        #[case] repaired: bool,
    ) {
        let outcome = Outcome::of(&report);
        assert_eq!(outcome, expected);
        assert_eq!(outcome.is_repairable(), repaired);
    }

    #[test]
    fn only_accepted_cards_with_something_modelled_are_kept() {
        let kept: Vec<bool> = [
            Outcome::Verified,
            Outcome::Partial,
            Outcome::Empty,
            Outcome::Unchecked,
            Outcome::Mismatch,
        ]
        .iter()
        .map(Outcome::is_kept)
        .collect();
        assert_eq!(kept, [true, true, false, false, false]);
    }

    #[test]
    fn the_summary_has_a_row_for_every_outcome_and_the_spending() {
        let mut budget = Budget::new(10, 5000);
        let request = crate::propose::model::Request {
            model: "m".into(),
            system: String::new(),
            messages: Vec::new(),
            max_tokens: 1,
        };
        budget.admit(&request, false).unwrap();
        budget.admit(&request, true).unwrap();
        budget.record(Usage {
            input_tokens: 1200,
            cache_read_tokens: 1000,
            output_tokens: 300,
        });
        let report = ProposeReport {
            model: "claude-sonnet-5-5".into(),
            tally: Tally {
                verified: 3,
                skipped: 1,
                not_run: 2,
                ..Tally::default()
            },
            budget,
            stopped: Some(Stop::Requests(10)),
            out: PathBuf::from("/tmp/out"),
        };
        let text = report.render();
        for expected in [
            "propose: 6 cards, model claude-sonnet-5-5",
            "Accepted Verified                3",
            "Skipped (resume)                 1",
            "Not run (budget)                 2",
            "requests 2 of 10 (1 repair rounds); input tokens 1200 of 5000 (1000 read from the cache); output tokens 300",
            "stopped: the budget of 10 requests is spent; `--resume` continues",
            "proposals in /tmp/out",
        ] {
            assert!(text.contains(expected), "missing {expected:?} in\n{text}");
        }
    }
}
