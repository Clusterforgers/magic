//! `check-rules --gaps`: everything that holds the checked proposals back, grouped by reason and ranked by the popularity
//! of the cards each gap would free (`docs/cards-dsl.md`, Model-assisted reading). With the fix marks of the database
//! (`--marks`) the gaps that free the most marked cards come first and a column shows the marks.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

use mtg_cards::proposal::{ClauseState, RepairClass, Report, Verdict};
use mtg_cards::support::{Clause, Support};

use crate::check_rules::Checked;
use crate::error::CliError;
use crate::popularity::{Popularity, weight_of};

#[cfg(test)]
mod tests;

/// How many gaps the printed table lists; the file has all of them.
pub const TOP: usize = 15;

/// The most examples of a gap.
const EXAMPLES: usize = 5;

/// The fix marks players made, by oracle id: how many accounts marked each card (`FixMarkRepo::ranked`).
pub type Marks = BTreeMap<String, u32>;

/// The longest reason of the printed table; a longer one is cut and ends in `...`.
const WIDTH: usize = 90;

/// What holds a proposal back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    /// A construct of the proposal the renderers cannot write, so the checker cannot read it back.
    MissingWriter,
    /// The rules fail a level of the checks of a rules file.
    Invalid,
    /// An ability is written back as other text than the card, or has no clause in it.
    Mismatch,
    /// A clause the engine does not run, so the card is not `Verified`.
    MissingPrimitive,
    /// A repair the checker made before it judged the file; it holds nothing back.
    Repaired,
}

impl Kind {
    /// The kind as the word of the ledger.
    pub fn word(self) -> &'static str {
        match self {
            Kind::MissingWriter => "missing-writer",
            Kind::Invalid => "invalid",
            Kind::Mismatch => "mismatch",
            Kind::MissingPrimitive => "missing-primitive",
            Kind::Repaired => "repaired",
        }
    }
}

/// One thing that holds a card back: its kind and the reason with names, numbers and ids taken out, so equal gaps are equal.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Blocker {
    /// What kind of gap.
    pub kind: Kind,
    /// The normalised reason.
    pub reason: String,
}

/// A card that stays out of the verified set, with the weight of its popularity and everything that holds it back.
#[derive(Debug, Clone, PartialEq)]
pub struct Held {
    /// The card's name.
    pub name: String,
    /// The weight of its EDHREC rank; 0 without one.
    pub weight: f64,
    /// How many accounts marked it as important to fix; 0 without marks.
    pub marks: u32,
    /// Its blockers; never empty.
    pub blockers: BTreeSet<Blocker>,
}

/// A row of the ledger: one blocker and what it holds back.
#[derive(Debug, Clone, PartialEq)]
pub struct Gap {
    /// The blocker.
    pub blocker: Blocker,
    /// The cards it blocks.
    pub blocked: usize,
    /// The cards that are free once it is gone: those it is the only blocker of.
    pub freed: usize,
    /// The popularity weight of the freed cards.
    pub weight: f64,
    /// The fix marks on the freed cards, summed.
    pub marks: u32,
    /// Up to five cards, the freed ones first and the most played first.
    pub examples: Vec<String>,
}

/// The parts of a construct that has no writer, one for each: `Spell (effect A; effect B)` is `Spell: effect A` and
/// `Spell: effect B`; a construct that names no parts is one.
fn construct_parts(construct: &str) -> Vec<String> {
    let split = construct
        .strip_suffix(')')
        .and_then(|rest| rest.split_once(" ("))
        .filter(|(_, parts)| !parts.contains('('));
    match split {
        Some((head, parts)) => parts
            .split("; ")
            .map(|part| format!("{head}: {part}"))
            .collect(),
        None => vec![construct.to_owned()],
    }
}

/// The text with what differs between equal gaps taken out: the names become `~`, an id `ID`, a number `N`, and white
/// space is one space.
pub fn normalise(text: &str, names: &[String]) -> String {
    let mut names: Vec<&str> = names
        .iter()
        .map(String::as_str)
        .filter(|name| !name.is_empty())
        .collect();
    names.sort_by_key(|name| std::cmp::Reverse(name.len()));
    let mut out = String::new();
    let mut at = 0;
    while at < text.len() {
        let rest = &text[at..];
        let after_word = text[..at].ends_with(|c: char| c.is_alphanumeric());
        if let Some(name) = names
            .iter()
            .find(|name| !after_word && names_a_word(rest, name))
        {
            out.push('~');
            at += name.len();
        } else if let Some(len) = id_len(rest) {
            out.push_str("ID");
            at += len;
        } else if rest.starts_with(|c: char| c.is_ascii_digit()) {
            out.push('N');
            at += rest
                .find(|c: char| !c.is_ascii_digit())
                .unwrap_or(rest.len());
        } else {
            let c = rest.chars().next().unwrap_or(' ');
            out.push(c);
            at += c.len_utf8();
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Whether `text` starts with `name` (the letters of the ASCII alphabet in any case) as a whole word.
fn names_a_word(text: &str, name: &str) -> bool {
    text.get(..name.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(name))
        && !text[name.len()..].starts_with(|c: char| c.is_alphanumeric())
}

/// The length of the id (a UUID) that `text` starts with.
fn id_len(text: &str) -> Option<usize> {
    const SHAPE: [usize; 5] = [8, 4, 4, 4, 12];
    let mut at = 0;
    for (i, len) in SHAPE.into_iter().enumerate() {
        if i > 0 {
            if !text[at..].starts_with('-') {
                return None;
            }
            at += 1;
        }
        let part = text.get(at..at + len)?;
        if !part.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        at += len;
    }
    Some(at)
}

/// Why the game does not run a clause, and whether the reason is the text of the card. A clause no rule stands for names
/// its text, since the reason the checker gives is the same for all of them.
fn unsupported_reason(report: &Report, unsupported: &Clause) -> (String, bool) {
    let left = report
        .left()
        .find(|clause| clause.face == unsupported.face && clause.span == unsupported.span);
    match left {
        Some(clause) => (format!("no rule for: {}", clause.text), true),
        None => (
            unsupported.reason.clone(),
            !unsupported.reason.starts_with("the engine does not run "),
        ),
    }
}

/// What holds a checked proposal back: the problems of an invalid one, and for a valid one the clauses written as other
/// text, the constructs without a writer, the abilities without a clause and the clauses the engine does not run.
pub fn blockers(report: &Report, names: &[String]) -> BTreeSet<Blocker> {
    let mut found = BTreeSet::new();
    // The names are taken out of the text of the card only, never of a name of the DSL or the engine.
    let mut add = |kind: Kind, text: &str, from_card: bool| {
        found.insert(Blocker {
            kind,
            reason: normalise(text, if from_card { names } else { &[] }),
        });
    };
    if report.verdict() == Verdict::Invalid {
        for problem in &report.problems {
            add(Kind::Invalid, &problem.message, false);
        }
        if report.problems.is_empty() {
            add(Kind::Invalid, "the file is not rules", false);
        }
        return found;
    }
    for clause in &report.clauses {
        match &clause.state {
            ClauseState::Differs { written, .. } => {
                let text = format!("wrote \"{written}\" for \"{}\"", clause.text);
                add(Kind::Mismatch, &text, true);
            }
            ClauseState::Unwritten { construct, .. } => {
                for part in construct_parts(construct) {
                    add(Kind::MissingWriter, &part, false);
                }
            }
            _ => {}
        }
    }
    for stray in &report.strays {
        let text = format!("{} has no clause in the text", stray.construct);
        add(Kind::Mismatch, &text, false);
    }
    if let Some(record) = report
        .support
        .as_ref()
        .filter(|record| record.support != Support::Verified)
    {
        for unsupported in &record.clauses {
            let (text, from_card) = unsupported_reason(report, unsupported);
            add(Kind::MissingPrimitive, &text, from_card);
        }
    }
    found
}

/// The cards the checked proposals leave out of the verified set. A card with several proposals counts once, by the one
/// with the fewest blockers (the first of equals); a card with a proposal that nothing holds back is not held, and neither
/// is a card that is `Verified` with no rules (its text is only reminder text), whatever the proposal says.
pub fn held_cards(checked: &Checked, popularity: &Popularity, marks: &Marks) -> Vec<Held> {
    let mut best: BTreeMap<&str, Held> = BTreeMap::new();
    let unverified = checked
        .reports
        .iter()
        .zip(&checked.subjects)
        .filter(|(_, subject)| !subject.plain);
    for (report, subject) in unverified {
        let held = Held {
            name: report.card.clone(),
            weight: popularity.rank(&subject.oracle_id).map_or(0.0, weight_of),
            marks: marks.get(&subject.oracle_id).copied().unwrap_or(0),
            blockers: blockers(report, &subject.names),
        };
        let better = best
            .get(subject.oracle_id.as_str())
            .is_none_or(|kept| held.blockers.len() < kept.blockers.len());
        if better {
            best.insert(&subject.oracle_id, held);
        }
    }
    best.into_values()
        .filter(|held| !held.blockers.is_empty())
        .collect()
}

/// What one blocker has gathered while the cards are counted.
#[derive(Default)]
struct Tally<'a> {
    blocked: usize,
    freed: Vec<&'a Held>,
    others: Vec<&'a Held>,
}

/// The cards by weight, the most played first, then by name.
fn by_weight(cards: &mut [&Held]) {
    cards.sort_by(|a, b| b.weight.total_cmp(&a.weight).then(a.name.cmp(&b.name)));
}

/// The ledger: a row for each blocker, the gaps that free the most marked cards first (all tie without marks), then the
/// most popularity, the most cards freed, the most cards blocked, the kind and the reason.
pub fn rank(held: &[Held]) -> Vec<Gap> {
    let mut tallies: BTreeMap<&Blocker, Tally<'_>> = BTreeMap::new();
    for card in held {
        for blocker in &card.blockers {
            let tally = tallies.entry(blocker).or_default();
            tally.blocked += 1;
            if card.blockers.len() == 1 {
                tally.freed.push(card);
            } else {
                tally.others.push(card);
            }
        }
    }
    let mut gaps: Vec<Gap> = tallies
        .into_iter()
        .map(|(blocker, mut tally)| {
            by_weight(&mut tally.freed);
            by_weight(&mut tally.others);
            Gap {
                blocker: blocker.clone(),
                blocked: tally.blocked,
                freed: tally.freed.len(),
                // Not `sum()`: the sum of nothing is -0.0, which sorts before 0.0.
                weight: tally.freed.iter().fold(0.0, |sum, card| sum + card.weight),
                marks: tally.freed.iter().map(|card| card.marks).sum(),
                examples: tally
                    .freed
                    .iter()
                    .chain(&tally.others)
                    .take(EXAMPLES)
                    .map(|card| card.name.clone())
                    .collect(),
            }
        })
        .collect();
    gaps.sort_by(|a, b| {
        b.marks
            .cmp(&a.marks)
            .then(b.weight.total_cmp(&a.weight))
            .then(b.freed.cmp(&a.freed))
            .then(b.blocked.cmp(&a.blocked))
            .then(a.blocker.cmp(&b.blocker))
    });
    gaps
}

/// A row for each class of repair the checker made: the cards it repaired (`blocked`), those among them that nothing holds
/// back now (`freed`) and the popularity weight of those. A card with several proposals counts once for a class.
pub fn repair_rows(checked: &Checked, popularity: &Popularity) -> Vec<Gap> {
    let mut classes: BTreeMap<RepairClass, Vec<(&str, f64, bool)>> = BTreeMap::new();
    let mut seen: BTreeSet<(RepairClass, &str)> = BTreeSet::new();
    for (report, subject) in checked.reports.iter().zip(&checked.subjects) {
        let free = blockers(report, &subject.names).is_empty();
        let weight = popularity.rank(&subject.oracle_id).map_or(0.0, weight_of);
        for class in report.repairs.iter().map(|repair| repair.class) {
            if seen.insert((class, subject.oracle_id.as_str())) {
                classes
                    .entry(class)
                    .or_default()
                    .push((report.card.as_str(), weight, free));
            }
        }
    }
    let mut rows: Vec<Gap> = classes
        .into_iter()
        .map(|(class, mut cards)| {
            cards.sort_by(|a, b| b.2.cmp(&a.2).then(b.1.total_cmp(&a.1)).then(a.0.cmp(b.0)));
            let freed = cards.iter().filter(|card| card.2);
            Gap {
                blocker: Blocker {
                    kind: Kind::Repaired,
                    reason: class.word().to_owned(),
                },
                blocked: cards.len(),
                freed: freed.clone().count(),
                weight: freed.fold(0.0, |sum, card| sum + card.1),
                marks: 0,
                examples: cards
                    .iter()
                    .take(EXAMPLES)
                    .map(|card| card.0.to_owned())
                    .collect(),
            }
        })
        .collect();
    rows.sort_by(|a, b| b.blocked.cmp(&a.blocked).then(a.blocker.cmp(&b.blocker)));
    rows
}

/// The repairs as one line: the cards repaired for each class.
fn repaired_line(rows: &[Gap]) -> Option<String> {
    if rows.is_empty() {
        return None;
    }
    let parts: Vec<String> = rows
        .iter()
        .map(|row| format!("{} {}", row.blocked, row.blocker.reason))
        .collect();
    Some(format!("repaired before judging: {}", parts.join(", ")))
}

/// A field of the TSV: a tab or a line break is a space.
fn field(text: &str) -> String {
    text.replace(['\t', '\r', '\n'], " ")
}

/// The ledger as TSV: a header, then a row for each gap with the kind, the reason, the cards blocked, the cards freed, the
/// weight of the freed cards and the examples joined by `; `.
pub fn tsv(gaps: &[Gap]) -> String {
    tsv_with(gaps, false)
}

/// [`tsv`] with the fix marks on the freed cards as a `marks` column after `freed`.
pub fn tsv_marked(gaps: &[Gap]) -> String {
    tsv_with(gaps, true)
}

fn tsv_with(gaps: &[Gap], marks: bool) -> String {
    let mark_head = if marks { "\tmarks" } else { "" };
    let mut text = format!("kind\treason\tblocked\tfreed{mark_head}\tweight\texamples\n");
    for gap in gaps {
        let mark_field = if marks {
            format!("\t{}", gap.marks)
        } else {
            String::new()
        };
        let _ = writeln!(
            text,
            "{}\t{}\t{}\t{}{mark_field}\t{:.6}\t{}",
            gap.blocker.kind.word(),
            field(&gap.blocker.reason),
            gap.blocked,
            gap.freed,
            gap.weight,
            field(&gap.examples.join("; ")),
        );
    }
    text
}

/// The first rows of the ledger as a table for the terminal.
pub fn table(gaps: &[Gap], top: usize) -> String {
    table_with(gaps, top, false)
}

/// [`table`] with the fix marks on the freed cards as a `marks` column, the gaps with the most marks first.
pub fn table_marked(gaps: &[Gap], top: usize) -> String {
    table_with(gaps, top, true)
}

fn table_with(gaps: &[Gap], top: usize, marks: bool) -> String {
    let mut text = format!(
        "gaps (top {} of {}; freed: cards this gap alone holds back, weight: their 1 / (EDHREC rank + 100) summed{})\n",
        top.min(gaps.len()),
        gaps.len(),
        if marks {
            ", marks: fix marks on them, ranked first"
        } else {
            ""
        }
    );
    let mark_head = if marks {
        format!(" {:>5}", "marks")
    } else {
        String::new()
    };
    let _ = writeln!(
        text,
        "{:>3}  {:<17} {:>5} {:>7}{mark_head} {:>8}  reason (examples)",
        "#", "kind", "freed", "blocked", "weight"
    );
    for (i, gap) in gaps.iter().take(top).enumerate() {
        let mark_cell = if marks {
            format!(" {:>5}", gap.marks)
        } else {
            String::new()
        };
        let _ = writeln!(
            text,
            "{:>3}  {:<17} {:>5} {:>7}{mark_cell} {:>8.5}  {} ({})",
            i + 1,
            gap.blocker.kind.word(),
            gap.freed,
            gap.blocked,
            gap.weight,
            cut(&gap.blocker.reason),
            gap.examples.join("; "),
        );
    }
    text.trim_end().to_owned()
}

/// The reason cut to the width of the table.
fn cut(text: &str) -> String {
    if text.chars().count() <= WIDTH {
        return text.to_owned();
    }
    let kept: String = text.chars().take(WIDTH - 3).collect();
    format!("{kept}...")
}

/// Writes the ledger of the checked proposals to `path` and returns the text to print: the counts, the top of the ledger
/// and where it is written. With `marks` (the fix marks of the database) the gaps are ranked by them and a column shows
/// them; without, the ledger is as it was.
pub async fn write_ledger(
    checked: &Checked,
    popularity: &Popularity,
    marks: Option<&Marks>,
    path: &Path,
) -> Result<String, CliError> {
    let held = held_cards(checked, popularity, marks.unwrap_or(&Marks::new()));
    let gaps = rank(&held);
    let repaired = repair_rows(checked, popularity);
    let every: Vec<Gap> = gaps
        .iter()
        .cloned()
        .chain(repaired.iter().cloned())
        .collect();
    let (file, top) = match marks {
        Some(_) => (tsv_marked(&every), table_marked(&gaps, TOP)),
        None => (tsv(&every), table(&gaps, TOP)),
    };
    tokio::fs::write(path, file)
        .await
        .map_err(|source| CliError::io(path, source))?;
    let mut text = format!(
        "{}\n{} cards held back by {} gaps\n{top}\ngaps written to {}",
        checked.counts(),
        held.len(),
        gaps.len(),
        path.display()
    );
    if let Some(line) = repaired_line(&repaired) {
        let _ = write!(text, "\n{line}");
    }
    if !checked.written.is_empty() {
        let _ = write!(text, "\n{} rules files written", checked.written.len());
    }
    Ok(text)
}
