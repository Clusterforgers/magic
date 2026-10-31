//! The funnel of a run (scripts read, translated whole, partly, skipped) and the ranking of unmapped constructs by the
//! cards each holds back, in the line-per-gap TSV shape of the `check-rules --gaps` ledger.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use super::card::CardOut;

/// The most examples of a gap.
const EXAMPLES: usize = 5;

/// How many rows the printed rankings show.
const TOP: usize = 15;

/// The cards a gap holds back.
#[derive(Debug, Default)]
struct Row {
    blocked: BTreeSet<String>,
    freed: usize,
}

/// What a run read and what became of it.
#[derive(Debug, Default)]
pub struct Funnel {
    /// Scripts read.
    pub read: usize,
    /// Scripts of cards the database does not store (with `--stored`).
    pub unstored: usize,
    /// Cards with no rule line at all (vanilla, or only an Oracle line), which have nothing to translate.
    pub plain: usize,
    /// Cards with every rule line translated.
    pub whole: usize,
    /// Cards with some rule lines translated and others left out.
    pub partial: usize,
    /// Cards with rule lines and none translated.
    pub skipped: usize,
    rows: BTreeMap<String, Row>,
    first: BTreeMap<String, usize>,
}

impl Funnel {
    /// Counts a card; true when it has a proposal to write.
    pub fn add(&mut self, card: &CardOut) -> bool {
        self.read += 1;
        let names: BTreeSet<&str> = card.gaps().map(|gap| gap.0.as_str()).collect();
        for name in &names {
            let row = self.rows.entry((*name).to_owned()).or_default();
            row.blocked.insert(card.name.clone());
            if names.len() == 1 {
                row.freed += 1;
            }
        }
        if card.lines() == 0 {
            self.plain += 1;
            return false;
        }
        if card.translated() == 0 {
            self.skipped += 1;
            let reason = card.gaps().next().map_or("?", |gap| gap.0.as_str());
            *self.first.entry(reason.to_owned()).or_default() += 1;
            return false;
        }
        if names.is_empty() {
            self.whole += 1;
        } else {
            self.partial += 1;
        }
        true
    }

    /// The rows by cards held back, then cards freed alone, then name.
    fn ranked(&self) -> Vec<(&String, &Row)> {
        let mut rows: Vec<(&String, &Row)> = self.rows.iter().collect();
        rows.sort_by(|a, b| {
            b.1.blocked
                .len()
                .cmp(&a.1.blocked.len())
                .then(b.1.freed.cmp(&a.1.freed))
                .then(a.0.cmp(b.0))
        });
        rows
    }

    /// The ledger as TSV: `kind`, `reason`, `blocked`, `freed`, `weight`, `examples`.
    pub fn ledger(&self) -> String {
        let mut text = String::from("kind\treason\tblocked\tfreed\tweight\texamples\n");
        for (name, row) in self.ranked() {
            let examples: Vec<&str> = row.blocked.iter().take(EXAMPLES).map(String::as_str).collect();
            let _ = writeln!(
                text,
                "unmapped\t{name}\t{}\t{}\t0\t{}",
                row.blocked.len(),
                row.freed,
                examples.join("; ")
            );
        }
        text
    }

    /// The summary a run prints.
    pub fn render(&self, out: &Path) -> String {
        let mut text = format!(
            "translate-forge: {} scripts read: {} translated whole, {} partly, {} skipped, {} with no rule line, {} not stored\n",
            self.read + self.unstored,
            self.whole,
            self.partial,
            self.skipped,
            self.plain,
            self.unstored
        );
        let mut reasons: Vec<(&String, &usize)> = self.first.iter().collect();
        reasons.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        text.push_str("skipped, by the first unmapped construct:\n");
        for (reason, count) in reasons.into_iter().take(TOP) {
            let _ = writeln!(text, "  {count:>6}  {reason}");
        }
        text.push_str("unmapped constructs, by cards held back (freed alone):\n");
        for (name, row) in self.ranked().into_iter().take(TOP) {
            let _ = writeln!(text, "  {:>6} ({:>5})  {name}", row.blocked.len(), row.freed);
        }
        let _ = write!(text, "proposals and translate-gaps.tsv in {}", out.display());
        text
    }
}
