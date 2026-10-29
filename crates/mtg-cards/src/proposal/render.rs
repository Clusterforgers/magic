//! The report as text: a verdict line, then what to fix, and with `verbose` the clauses that read back too.

use std::fmt::Write as _;

use super::{ClauseReport, ClauseState, Report, Verdict};
use crate::support::Support;

impl Verdict {
    /// The verdict as one word.
    pub fn word(self) -> &'static str {
        match self {
            Verdict::Invalid => "INVALID",
            Verdict::Mismatch => "MISMATCH",
            Verdict::Unchecked => "UNCHECKED",
            Verdict::Accepted => "ACCEPTED",
        }
    }
}

impl Report {
    /// The support state when the proposal is accepted, as a word; the verdict word otherwise.
    pub fn summary(&self) -> String {
        let verdict = self.verdict();
        match (&self.support, verdict) {
            (Some(record), Verdict::Accepted | Verdict::Unchecked) => {
                format!("{} {}", verdict.word(), support_word(record.support))
            }
            _ => verdict.word().to_owned(),
        }
    }

    /// The report: the card and its summary, what was repaired, the problems, each clause that does not read back and, with `verbose`,
    /// each that does; the clauses the engine does not run come last.
    pub fn render(&self, verbose: bool) -> String {
        let mut text = format!("{}: {}\n", self.card, self.summary());
        for repair in &self.repairs {
            let _ = writeln!(text, "  repaired  {repair}");
        }
        for problem in &self.problems {
            let _ = writeln!(text, "  invalid   {problem}");
        }
        for clause in &self.clauses {
            if verbose || !matches!(clause.state, ClauseState::Reads { .. }) {
                let _ = writeln!(text, "  {}", clause_line(clause));
            }
        }
        for stray in &self.strays {
            let _ = writeln!(
                text,
                "  stray     face {} ability {} ({}): no clause of the text is left for it",
                stray.face, stray.ability, stray.construct
            );
        }
        if let Some(record) = &self.support {
            for clause in &record.clauses {
                let _ = writeln!(text, "  blocker   face {}: {}", clause.face, clause.reason);
            }
        }
        text.trim_end().to_owned()
    }
}

fn support_word(state: Support) -> &'static str {
    match state {
        Support::Verified => "Verified",
        Support::Partial => "Partial",
        Support::Manual => "Manual",
        Support::Stale => "Stale",
        Support::Unplayable => "Unplayable",
    }
}

/// One clause: its state, then its text, then what the proposal writes back.
fn clause_line(clause: &ClauseReport) -> String {
    let text = clause.text.replace('\n', " / ");
    match &clause.state {
        ClauseState::Reads { ability } => format!("reads     ability {ability}: {text}"),
        ClauseState::Unwritten { ability, construct } => {
            format!(
                "unwritten ability {ability} has no writer: {construct}\n              clause: {text}"
            )
        }
        ClauseState::Differs { ability, written } => {
            format!(
                "differs   ability {ability} is written back as\n              {written}\n              clause: {text}"
            )
        }
        ClauseState::Manual { reason } => format!("manual    {reason}: {text}"),
        ClauseState::Open => format!("open      no ability: {text}"),
    }
}
