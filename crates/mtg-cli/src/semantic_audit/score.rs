//! `mutation-score`: how many single-fault mutants of `Full` corpus cards the audit's checks and the checker's read back
//! catch, per mutation kind and per check.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use mtg_cards::card::{Card, ImplStatus};
use mtg_cards::read::{RowState, sheet};
use mtg_cards::support::PrintedCard;
use mtg_testkit::mutate::{MutationKind, mutants};

use super::{CORPUS_DIR, CORPUS_FIXTURES, Entry, Finding, audit, corpus, par_map};
use crate::args::MutationScoreArgs;
use crate::error::CliError;

/// The tally of one mutation kind.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tally {
    /// Mutants made.
    pub mutants: usize,
    /// Mutants with a finding of the audit the original lacks.
    pub audit: usize,
    /// Mutants with a clause that no longer reads back as its text.
    pub readback: usize,
    /// Mutants either catches.
    pub either: usize,
    /// Mutants each check caught.
    pub by_check: BTreeMap<&'static str, usize>,
}

/// The score of a run: the cards mutated and the tally per kind.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MutationScore {
    /// Cards mutated.
    pub cards: usize,
    /// The tally per mutation kind.
    pub kinds: BTreeMap<MutationKind, Tally>,
}

/// The rows of a card that read back as their text, by position.
fn written(card: &Card, printed: &PrintedCard) -> Vec<bool> {
    sheet(card, printed)
        .rows
        .iter()
        .map(|row| matches!(row.state, RowState::Written))
        .collect()
}

/// Mutates each card once per kind and tallies what the audit and the read back catch.
pub fn score(entries: &[Entry], seed: u64) -> MutationScore {
    let numbered: Vec<(u64, &Entry)> = (0u64..).zip(entries).collect();
    let judged = par_map(&numbered, |(n, entry)| {
        let before: BTreeSet<Finding> = audit(&entry.card, &entry.printed).into_iter().collect();
        let read_before = written(&entry.card, &entry.printed);
        mutants(&entry.card, seed.wrapping_add(*n))
            .into_iter()
            .map(|mutant| {
                let after: BTreeSet<Finding> =
                    audit(&mutant.card, &entry.printed).into_iter().collect();
                let new_checks: BTreeSet<&'static str> =
                    after.difference(&before).map(|f| f.check).collect();
                let read_after = written(&mutant.card, &entry.printed);
                let lost = read_before.len() != read_after.len()
                    || read_before.iter().zip(&read_after).any(|(b, a)| *b && !*a);
                (mutant.kind, new_checks, lost)
            })
            .collect::<Vec<_>>()
    });
    let mut out = MutationScore {
        cards: entries.len(),
        ..MutationScore::default()
    };
    for (kind, new_checks, lost) in judged.into_iter().flatten() {
        {
            let tally = out.kinds.entry(kind).or_default();
            tally.mutants += 1;
            tally.audit += usize::from(!new_checks.is_empty());
            tally.readback += usize::from(lost);
            tally.either += usize::from(lost || !new_checks.is_empty());
            for check in new_checks {
                *tally.by_check.entry(check).or_default() += 1;
            }
        }
    }
    out
}

/// A share as a percentage.
fn pct(part: usize, whole: usize) -> String {
    if whole == 0 {
        return "-".into();
    }
    format!("{:.0}%", 100.0 * part as f64 / whole as f64)
}

impl MutationScore {
    /// The report: per kind the mutants and the share each detector catches, the checks that caught them, and the total.
    pub fn render(&self) -> String {
        let mut text = format!(
            "{} cards\nkind\tmutants\taudit\treadback\teither\tchecks\n",
            self.cards
        );
        let mut total = Tally::default();
        for (kind, t) in &self.kinds {
            let checks: Vec<String> = t.by_check.iter().map(|(c, n)| format!("{c} {n}")).collect();
            let _ = writeln!(
                text,
                "{}\t{}\t{}\t{}\t{}\t{}",
                kind.name(),
                t.mutants,
                pct(t.audit, t.mutants),
                pct(t.readback, t.mutants),
                pct(t.either, t.mutants),
                checks.join(", ")
            );
            total.mutants += t.mutants;
            total.audit += t.audit;
            total.readback += t.readback;
            total.either += t.either;
        }
        let _ = writeln!(
            text,
            "all\t{}\t{}\t{}\t{}",
            total.mutants,
            pct(total.audit, total.mutants),
            pct(total.readback, total.mutants),
            pct(total.either, total.mutants)
        );
        text
    }
}

/// `mutation-score`: `--cards` `Full` corpus cards spread evenly over the corpus, one mutant per kind each.
pub fn mutation_score(args: &MutationScoreArgs) -> Result<String, CliError> {
    let all: Vec<Entry> = corpus(Path::new(CORPUS_DIR), Path::new(CORPUS_FIXTURES))?
        .into_iter()
        .filter(|e| e.card.impl_status == ImplStatus::Full)
        .collect();
    let step = (all.len() / args.cards.max(1)).max(1);
    let picked: Vec<Entry> = all.into_iter().step_by(step).take(args.cards).collect();
    Ok(score(&picked, args.seed).render())
}
