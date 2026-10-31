//! Proposed rules: a rules file written by a person or a model, checked against the printed card
//! without trusting it (`docs/cards-dsl.md`, Model-assisted reading).
//!
//! The text of the card is the only ground truth. A proposal is accepted when it is valid (the
//! levels of a rules file), when every ability is written back by the renderers as a clause of the
//! printed text, and it then has a support state like any other rules.

mod blame;
mod blocks;
mod complete;
mod pairing;
mod render;
mod repair;

pub(crate) use pairing::pair_card;
pub use repair::{Repair, RepairClass};

use std::collections::BTreeSet;

use crate::card::{Card, Span};
use crate::compile::{ClauseKind, compile};
use crate::load::{Library, LoadError, slug};
use crate::support::{Caps, PrintedCard, SupportRecord, support};

/// How a clause of the printed text stands against the proposed abilities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClauseState {
    /// An ability is written back as this clause (`written_variants` equals it, or the keyword reader reads it).
    Reads {
        /// The index of the ability on the face, as proposed.
        ability: usize,
    },
    /// An ability stands for the clause, but the renderers have no writer for a construct of it: it cannot be read back.
    Unwritten {
        /// The index of the ability on the face, as proposed.
        ability: usize,
        /// The ability and the part of it that has no writer.
        construct: String,
    },
    /// An ability stands for the clause and is written back as other text.
    Differs {
        /// The index of the ability on the face, as proposed.
        ability: usize,
        /// The wording of the ability closest to the clause.
        written: String,
    },
    /// A `Manual` ability of the proposal leaves the clause to the table.
    Manual {
        /// Why, as the proposal says.
        reason: String,
    },
    /// No ability stands for the clause; the checker leaves it to the table.
    Open,
}

/// One clause of the printed text and where it stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClauseReport {
    /// The face.
    pub face: usize,
    /// Where the clause lies in the face's oracle text.
    pub span: Span,
    /// The clause as the compiler splits it (reminder text removed, the card's name as `~`).
    pub text: String,
    /// Its shape.
    pub kind: ClauseKind,
    /// How the proposal covers it.
    pub state: ClauseState,
}

/// An ability of the proposal that no clause of the printed text is left for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stray {
    /// The face.
    pub face: usize,
    /// The index of the ability on the face, as proposed.
    pub ability: usize,
    /// The ability, named by its variant.
    pub construct: String,
}

/// What the checker concludes about a proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// The rules fail a level of the checks of a rules file, or the JSON does not parse.
    Invalid,
    /// The rules are valid but an ability is written as other text than the card, or has no clause in it.
    Mismatch,
    /// The rules are valid and nothing contradicts the text, but an ability has constructs the renderers cannot write.
    Unchecked,
    /// The rules are valid and every clause is written back or left to the table.
    Accepted,
}

/// The result of checking a proposal.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    /// The card's name.
    pub card: String,
    /// The problems of levels 1 to 3; empty when the rules are valid.
    pub problems: Vec<LoadError>,
    /// Every clause of the printed text in order, read or not.
    pub clauses: Vec<ClauseReport>,
    /// Abilities of the proposal without a clause.
    pub strays: Vec<Stray>,
    /// The support state of the completed rules; `None` when they are invalid.
    pub support: Option<SupportRecord>,
    /// What the checker repaired in the file before it judged it; empty when the file needed nothing.
    pub repairs: Vec<Repair>,
    /// The rules as the checker completes them: oracle id, name, status and fingerprints from the card, a `Manual`
    /// ability for each open clause, abilities in printed order; `None` when the JSON does not parse.
    pub rules: Option<Card>,
}

impl Report {
    /// Whether the rules are valid and what the reading back found.
    pub fn verdict(&self) -> Verdict {
        if !self.problems.is_empty() || self.rules.is_none() {
            return Verdict::Invalid;
        }
        let differs = self
            .clauses
            .iter()
            .any(|clause| matches!(clause.state, ClauseState::Differs { .. }));
        if differs || !self.strays.is_empty() {
            return Verdict::Mismatch;
        }
        if self.unwritten().next().is_some() {
            Verdict::Unchecked
        } else {
            Verdict::Accepted
        }
    }

    /// The constructs that have no writer, so the clause they stand for cannot be checked by reading back.
    pub fn unwritten(&self) -> impl Iterator<Item = &str> {
        self.clauses
            .iter()
            .filter_map(|clause| match &clause.state {
                ClauseState::Unwritten { construct, .. } => Some(construct.as_str()),
                _ => None,
            })
    }

    /// The clauses nothing models: left to the table by the proposal or by the checker.
    pub fn left(&self) -> impl Iterator<Item = &ClauseReport> {
        self.clauses
            .iter()
            .filter(|clause| matches!(clause.state, ClauseState::Open | ClauseState::Manual { .. }))
    }
}

/// Checks the JSON of a proposed rules file against an imported card.
///
/// The file may leave out what the card gives: the oracle id, the names, the status and the
/// fingerprints are filled in (the status is always derived from what is modelled), and the clauses
/// no ability stands for become `Manual` abilities. `hand_written` supplies the predefined tokens,
/// `names` the lowercase names of the cards a rule may name and `caps` what the engine runs.
pub fn check(
    text: &str,
    printed: &PrintedCard,
    hand_written: &Library,
    names: &BTreeSet<String>,
    caps: &Caps,
) -> Report {
    let file = format!("cards/{}.json", slug(&printed.name));
    let mut report = Report {
        card: printed.name.clone(),
        problems: Vec::new(),
        clauses: Vec::new(),
        strays: Vec::new(),
        support: None,
        rules: None,
        repairs: Vec::new(),
    };
    let mut value = match complete::read(text, &file) {
        Ok(value) => value,
        Err(error) => {
            report.problems.push(error);
            return report;
        }
    };
    report.repairs = repair::repair(&mut value, printed);
    let proposed = match complete::parse(value, printed, &file) {
        Ok(card) => card,
        Err(error) => {
            report.problems.push(error);
            return report;
        }
    };
    let clauses = compile(printed).clauses;
    let paired = pairing::pair_card(&proposed, &clauses, printed);
    let card = complete::finish(proposed, &paired, printed);
    report.problems = hand_written.check_generated(&card, printed, names);
    if report.problems.is_empty() {
        report.support = Some(support(Some(&card), printed, caps));
    }
    report.clauses = paired.clauses;
    report.strays = paired.strays;
    report.rules = Some(card);
    report
}
