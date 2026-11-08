//! The proof of a partial translation: every printed clause is marked as running automatically or left by hand, or the card is refused by name (`docs/cards-dsl.md`, Partial translations).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::{ClauseReport, ClauseState, Report};
use crate::card::Span;
use crate::compile::ClauseKind;

/// One line of a Forge script (`K:`, `A:`, `T:`, `S:` or `R:`) and what the translator made of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerLine {
    /// The face of the card the line belongs to.
    pub face: usize,
    /// The line's tag letter.
    pub tag: String,
    /// The line's own words: its `SpellDescription$`, `TriggerDescription$` or `Description$`; for a keyword the body.
    pub text: String,
    /// The indices, on the proposal's face, of the abilities the line gave; empty for a gap.
    pub abilities: Vec<usize>,
    /// The construct that stopped the line; `None` when it translated.
    pub gap: Option<String>,
}

/// How a printed clause plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mark {
    /// The engine runs the clause.
    Auto,
    /// The players do the clause by hand.
    ByHand,
}

/// The mark of one printed clause.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClauseMark {
    /// The face.
    pub face: usize,
    /// Where the clause lies in the face's oracle text.
    pub span: Span,
    /// How it plays.
    pub mark: Mark,
}

/// The proof: a mark for every printed clause, in text order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartialProof {
    /// One row for each clause of the report.
    pub marks: Vec<ClauseMark>,
}

/// Why a card is not proved, named for the promote report.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refusal {
    /// The rules fail a level of the checks.
    #[error("InvalidRules")]
    InvalidRules,
    /// An ability stands for no clause.
    #[error("Stray: ability {ability} of face {face}")]
    Stray {
        /// The face.
        face: usize,
        /// The ability index.
        ability: usize,
    },
    /// A gap line belongs to a clause that runs automatically without being read back whole.
    #[error("GapInsideLine: {clause} (gap {gap})")]
    GapInsideLine {
        /// The clause.
        clause: String,
        /// The construct that stopped the line.
        gap: String,
    },
    /// A word of an automatic clause that is not read back whole is in no translated line of its abilities.
    #[error("UncoveredLine: {clause} ({word})")]
    UncoveredLine {
        /// The clause.
        clause: String,
        /// The first word without a witness.
        word: String,
    },
    /// A gap line names no clause.
    #[error("UnmappedGap: {gap}")]
    UnmappedGap {
        /// The construct that stopped the line.
        gap: String,
    },
    /// A gap line has no description to match.
    #[error("UntextedGap: {gap}")]
    UntextedGap {
        /// The construct that stopped the line.
        gap: String,
    },
    /// A clause is by hand though no gap line explains it.
    #[error("OpenWithoutGap: {clause}")]
    OpenWithoutGap {
        /// The clause.
        clause: String,
    },
    /// A clause left by hand would change what the automatic lines do.
    #[error("UnsafeLine: {clause} ({class})")]
    UnsafeLine {
        /// The clause.
        clause: String,
        /// The kind of line, as the table of `docs/cards-dsl.md` names it.
        class: &'static str,
    },
    /// No clause runs automatically.
    #[error("NothingRuns")]
    NothingRuns,
}

impl Refusal {
    /// The variant's name, for counting refusals.
    pub fn name(&self) -> &'static str {
        match self {
            Self::InvalidRules => "InvalidRules",
            Self::Stray { .. } => "Stray",
            Self::GapInsideLine { .. } => "GapInsideLine",
            Self::UncoveredLine { .. } => "UncoveredLine",
            Self::UnmappedGap { .. } => "UnmappedGap",
            Self::UntextedGap { .. } => "UntextedGap",
            Self::OpenWithoutGap { .. } => "OpenWithoutGap",
            Self::UnsafeLine { .. } => "UnsafeLine",
            Self::NothingRuns => "NothingRuns",
        }
    }
}

/// Words that carry no meaning when a clause is matched against a script's description.
const STOP: &[&str] = &[
    "a", "an", "the", "of", "to", "and", "or", "it", "its", "is", "this", "that", "on", "in",
    "for", "you", "your", "with", "as", "at", "may", "if", "then", "from", "into", "by", "be",
    "are", "each", "has", "have", "gets", "get",
];

/// Words that tie a line to a memory the players cannot fill in.
const CROSS: &[&str] = &[
    "chosen",
    "named card",
    "the named",
    "noted",
    "exiled with",
    "the exiled",
    "remembered",
    "imprint",
    "that was exiled",
    "cards exiled",
    "secret",
];

/// The text lowercased with reminder text removed and the card's own name as `~`.
fn clean(text: &str) -> String {
    let mut out = String::new();
    let mut depth = 0u32;
    for c in text.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    let mut text = out
        .to_lowercase()
        .replace("cardname", "~")
        .replace("nickname", "~")
        .replace("\\n", " ");
    for noun in [
        "creature",
        "spell",
        "land",
        "artifact",
        "enchantment",
        "permanent",
        "card",
        "planeswalker",
        "aura",
        "equipment",
        "vehicle",
    ] {
        text = text.replace(&format!("this {noun}"), "~");
    }
    text
}

/// The content words of a text.
fn words(text: &str) -> BTreeSet<String> {
    let keep = |c: char| c.is_alphanumeric() || matches!(c, '~' | '{' | '}' | '+' | '-' | '/');
    clean(text)
        .split(|c: char| !keep(c))
        .filter(|word| !word.is_empty() && !STOP.contains(word))
        .map(str::to_owned)
        .collect()
}

fn any(text: &str, parts: &[&str]) -> bool {
    parts.iter().any(|part| text.contains(part))
}

fn is_hand(clause: &ClauseReport) -> bool {
    matches!(clause.state, ClauseState::Open | ClauseState::Manual { .. })
}

/// The ability a clause is paired with, when it is.
fn ability_of(clause: &ClauseReport) -> Option<usize> {
    match clause.state {
        ClauseState::Reads { ability }
        | ClauseState::Unwritten { ability, .. }
        | ClauseState::Differs { ability, .. } => Some(ability),
        ClauseState::Manual { .. } | ClauseState::Open => None,
    }
}

/// The clauses a gap line belongs to: a keyword line by the start of the clause, any other by shared words (at least
/// seven tenths of the shorter side).
fn clauses_of(line: &LedgerLine, clauses: &[ClauseReport]) -> Vec<usize> {
    let on_face = clauses
        .iter()
        .enumerate()
        .filter(|(_, c)| c.face == line.face);
    if line.tag == "K" {
        let head = line.text.split(':').next().unwrap_or("").to_lowercase();
        return on_face
            .filter(|(_, c)| !head.is_empty() && clean(&c.text).starts_with(&head))
            .map(|(at, _)| at)
            .collect();
    }
    let mine = words(&line.text);
    on_face
        .filter(|(_, c)| {
            let theirs = words(&c.text);
            let common = mine.intersection(&theirs).count();
            let shorter = mine.len().min(theirs.len()).max(1);
            common >= 2.min(shorter) && common * 10 >= shorter * 7
        })
        .map(|(at, _)| at)
        .collect()
}

/// The gap lines' clauses, and the refusals of the gap lines that name none or have no text.
fn gap_hits(
    report: &Report,
    ledger: &[LedgerLine],
    out: &mut Vec<Refusal>,
) -> BTreeMap<usize, String> {
    let mut hit = BTreeMap::new();
    for line in ledger {
        let Some(gap) = &line.gap else { continue };
        if line.tag != "K" && words(&line.text).is_empty() {
            out.push(Refusal::UntextedGap { gap: gap.clone() });
            continue;
        }
        let found = clauses_of(line, &report.clauses);
        if found.is_empty() {
            out.push(Refusal::UnmappedGap { gap: gap.clone() });
        }
        hit.extend(found.into_iter().map(|at| (at, gap.clone())));
    }
    hit
}

/// The first word of an automatic clause that no translated line of its ability witnesses.
fn uncovered(clause: &ClauseReport, ledger: &[LedgerLine]) -> Option<String> {
    let ability = ability_of(clause)?;
    let seen: BTreeSet<String> = ledger
        .iter()
        .filter(|line| {
            line.face == clause.face && line.gap.is_none() && line.abilities.contains(&ability)
        })
        .flat_map(|line| words(&line.text))
        .collect();
    words(&clause.text)
        .into_iter()
        .find(|word| !seen.contains(word))
}

/// The kind of a whole line left by hand when it is not safe, else `None`.
fn unsafe_class(kind: ClauseKind, text: &str, spell_runs: bool) -> Option<&'static str> {
    let t = clean(text);
    let entering = [
        "as ~ enters",
        "enters with",
        "enters tapped",
        "enters the",
        "as it enters",
        "enters untapped",
    ];
    match kind {
        ClauseKind::Activated | ClauseKind::Loyalty => None,
        ClauseKind::Triggered => any(&t, &entering[..2]).then_some("entering"),
        ClauseKind::Spell | ClauseKind::Modal => spell_runs.then_some("spell beside a spell line"),
        ClauseKind::Keyword => Some("keyword"),
        _ if any(
            &t,
            &[
                "additional cost",
                "rather than pay",
                "without paying",
                "alternative cost",
                "costs {",
                "cost {",
                "less to cast",
                "more to cast",
                "less to activate",
                "more to activate",
            ],
        ) =>
        {
            Some("cost")
        }
        _ if any(&t, &entering) => Some("entering"),
        _ if any(&t, &["instead", "would", "prevent", "skip"]) => Some("replacement"),
        _ if any(
            &t,
            &[
                "can't",
                "cannot",
                "must",
                "as though",
                "unless",
                "doesn't",
                "don't",
                "only",
                "no more than",
                "can block",
                "can attack",
                "can be",
            ],
        ) =>
        {
            Some("restriction")
        }
        ClauseKind::Static => Some("static"),
    }
}

/// Proves a partial translation: a mark for every printed clause, or the named refusals.
///
/// A clause is `Auto` when it is read back whole, or when it has a witness (every content word is in a translated line
/// of its abilities and no gap line belongs to it); it is `ByHand` when a `Manual` ability or the checker leaves it, a
/// gap line explains it and leaving it cannot change what the automatic lines do. Anything else is refused. The caller
/// refuses a card whose Forge text differs from the printed one, and one that gains nothing over what it plays today.
pub fn prove(report: &Report, ledger: &[LedgerLine]) -> Result<PartialProof, Vec<Refusal>> {
    if !report.problems.is_empty() || report.rules.is_none() {
        return Err(vec![Refusal::InvalidRules]);
    }
    let mut refusals: Vec<Refusal> = report
        .strays
        .iter()
        .map(|stray| Refusal::Stray {
            face: stray.face,
            ability: stray.ability,
        })
        .collect();
    let hit = gap_hits(report, ledger, &mut refusals);
    let cross_auto = report
        .clauses
        .iter()
        .any(|c| !is_hand(c) && any(&clean(&c.text), CROSS));
    for (at, clause) in report.clauses.iter().enumerate() {
        let name = clause.text.clone();
        if is_hand(clause) {
            let spell_runs = report.clauses.iter().any(|o| {
                o.face == clause.face
                    && matches!(o.kind, ClauseKind::Spell | ClauseKind::Modal)
                    && !is_hand(o)
            });
            if !hit.contains_key(&at) {
                refusals.push(Refusal::OpenWithoutGap {
                    clause: name.clone(),
                });
            }
            let class = if any(&clean(&clause.text), CROSS) || cross_auto {
                Some("crossing clauses")
            } else {
                unsafe_class(clause.kind, &clause.text, spell_runs)
            };
            if let Some(class) = class {
                refusals.push(Refusal::UnsafeLine {
                    clause: name,
                    class,
                });
            }
        } else if !matches!(clause.state, ClauseState::Reads { .. }) {
            if let Some(gap) = hit.get(&at) {
                refusals.push(Refusal::GapInsideLine {
                    clause: name.clone(),
                    gap: gap.clone(),
                });
            }
            if let Some(word) = uncovered(clause, ledger) {
                refusals.push(Refusal::UncoveredLine { clause: name, word });
            }
        }
    }
    if report.clauses.iter().all(is_hand) {
        refusals.push(Refusal::NothingRuns);
    }
    if !refusals.is_empty() {
        return Err(refusals);
    }
    let marks = report
        .clauses
        .iter()
        .map(|clause| ClauseMark {
            face: clause.face,
            span: clause.span,
            mark: if is_hand(clause) {
                Mark::ByHand
            } else {
                Mark::Auto
            },
        })
        .collect();
    Ok(PartialProof { marks })
}
