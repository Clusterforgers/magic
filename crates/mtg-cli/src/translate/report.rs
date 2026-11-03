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
    weight: f64,
}

/// What a run read and what became of it.
#[derive(Debug, Default)]
pub struct Funnel {
    /// Scripts read.
    pub read: usize,
    /// Cards the corpus or the compiler already makes `Verified`, which the ranking of gaps leaves out.
    pub verified_today: usize,
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
    /// With `--stored`: the cards by their state today, as counts of whole, partial, skipped and plain.
    states: BTreeMap<String, [usize; 4]>,
    rows: BTreeMap<String, Row>,
    first: BTreeMap<String, usize>,
}

impl Funnel {
    /// Counts a card; true when it has a proposal to write.
    pub fn add(&mut self, card: &CardOut, ranked: bool, weight: f64, today: &str) -> bool {
        let kind = self.add_card(card, ranked, weight);
        if !today.is_empty() {
            let slot = match (
                card.lines(),
                card.translated(),
                card.gaps().next().is_none(),
            ) {
                (0, _, _) => 3,
                (_, 0, _) => 2,
                (_, _, true) => 0,
                _ => 1,
            };
            self.states.entry(today.to_owned()).or_default()[slot] += 1;
        }
        kind
    }

    fn add_card(&mut self, card: &CardOut, ranked: bool, weight: f64) -> bool {
        self.read += 1;
        let names: BTreeSet<&str> = card.gaps().map(|gap| gap.0.as_str()).collect();
        if !ranked {
            self.verified_today += 1;
        }
        for name in names.iter().filter(|_| ranked) {
            let row = self.rows.entry((*name).to_owned()).or_default();
            row.blocked.insert(card.name.clone());
            if names.len() == 1 {
                row.freed += 1;
                row.weight += weight;
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
        let mut rows = self.ranked();
        rows.sort_by(|a, b| {
            b.1.weight
                .total_cmp(&a.1.weight)
                .then(b.1.freed.cmp(&a.1.freed))
                .then(b.1.blocked.len().cmp(&a.1.blocked.len()))
        });
        for (name, row) in rows {
            let examples: Vec<&str> = row
                .blocked
                .iter()
                .take(EXAMPLES)
                .map(String::as_str)
                .collect();
            let _ = writeln!(
                text,
                "unmapped\t{name}\t{}\t{}\t{:.6}\t{}",
                row.blocked.len(),
                row.freed,
                row.weight,
                examples.join("; ")
            );
        }
        text
    }

    /// The gaps that need a node the language or the engine lacks, as TSV: `construct`, `blocked`, `freed`, `missing`,
    /// `examples`.
    pub fn engine_needs(&self) -> String {
        let mut text = String::from("construct\tblocked\tfreed\tmissing\texamples\n");
        for (name, row) in self.ranked() {
            let Some(need) = super::needs::need(name) else {
                continue;
            };
            let examples: Vec<&str> = row
                .blocked
                .iter()
                .take(EXAMPLES)
                .map(String::as_str)
                .collect();
            let _ = writeln!(
                text,
                "{name}\t{}\t{}\t{need}\t{}",
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
        if !self.states.is_empty() {
            text.push_str("by state today (whole, partial, skipped, no rule line):\n");
            for (state, [whole, partial, skipped, plain]) in &self.states {
                let _ = writeln!(
                    text,
                    "  {state:<12} {whole:>6} {partial:>6} {skipped:>6} {plain:>6}"
                );
            }
        }
        if self.verified_today > 0 {
            let _ = writeln!(
                text,
                "{} of the proposals are for cards that are Verified already (corpus or compiler); the rankings leave them out",
                self.verified_today
            );
        }
        let mut reasons: Vec<(&String, &usize)> = self.first.iter().collect();
        reasons.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        text.push_str("skipped, by the first unmapped construct:\n");
        for (reason, count) in reasons.into_iter().take(TOP) {
            let _ = writeln!(text, "  {count:>6}  {reason}");
        }
        text.push_str("unmapped constructs, by cards held back (freed alone):\n");
        for (name, row) in self.ranked().into_iter().take(TOP) {
            let _ = writeln!(
                text,
                "  {:>6} ({:>5})  {name}",
                row.blocked.len(),
                row.freed
            );
        }
        let _ = write!(
            text,
            "proposals and translate-gaps.tsv in {}",
            out.display()
        );
        text
    }
}
