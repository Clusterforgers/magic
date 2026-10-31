//! The review sheet of a card: for each printed clause the ability of the rules that stands for it, the renderer's wording
//! of that ability and a list of steps of what the game will do, so a player can read the card beside its text.
//!
//! The pairing is the checker's (`proposal`), the wording the renderer's first, and the steps a walk of the whole ability
//! with words for every node (`docs/cards-dsl.md`, Read tier). No row is blank: a clause without an ability says it is
//! played by hand, an ability the renderer cannot word is shown by its steps and its label.

mod effects;
mod generic;
mod parts;
mod phrase;
mod rows;
mod steps;

use serde::{Deserialize, Serialize};

use crate::ability::Ability;
use crate::card::{Card, Span};
use crate::compile::ClauseKind;
use crate::support::PrintedCard;

/// One line of what the game does, with the depth of the block it sits in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
    /// How many blocks deep the line is: 0 for a line of the ability, 1 for one under an "if" or a mode.
    pub depth: u8,
    /// The line in plain words.
    pub text: String,
}

impl Step {
    /// A step at a depth.
    pub(crate) fn new(depth: u8, text: impl Into<String>) -> Step {
        Step {
            depth,
            text: text.into(),
        }
    }
}

/// How the rules stand against one clause of the printed text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RowState {
    /// The renderer writes the ability back as this clause.
    Written,
    /// An ability stands for the clause, but the renderer has no writer for a construct of it.
    Unwritten {
        /// The ability and the part of it that has no writer.
        construct: String,
    },
    /// The renderer writes the ability as other text than the clause.
    Differs {
        /// The wording of the ability closest to the clause.
        written: String,
    },
    /// No ability stands for the clause: the table plays it by hand.
    ByHand {
        /// Why, when the rules say so.
        reason: Option<String>,
    },
    /// An ability that no clause of the printed text stands behind.
    NoClause,
}

/// One printed clause and what the rules make of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadRow {
    /// The face the clause belongs to.
    pub face: usize,
    /// Where the clause lies in the face's oracle text, in Unicode scalar values.
    pub span: Span,
    /// The clause's shape.
    pub kind: ClauseKind,
    /// The clause as printed.
    pub printed: String,
    /// How the rules stand against the clause.
    pub state: RowState,
    /// The abilities of the face that stand for the clause, as indices into the face's abilities.
    pub abilities: Vec<usize>,
    /// The ability is shown on an earlier row (one ability stands for several lines of the text).
    pub continues: bool,
    /// What kind of ability it is, in a few words; never blank.
    pub label: String,
    /// The renderer's first wording of the ability, when it has a writer for it.
    pub wording: Option<String>,
    /// What the game does, never empty.
    pub steps: Vec<Step>,
}

/// The review sheet of a card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadSheet {
    /// The card's name.
    pub card: String,
    /// The names of its faces.
    pub faces: Vec<String>,
    /// One row for each clause of every face, in text order.
    pub rows: Vec<ReadRow>,
    /// Abilities no clause stands behind; empty for rules that read back from the text.
    pub extra: Vec<ReadRow>,
}

impl ReadSheet {
    /// Whether every row has printed words, a label, steps and no blank line.
    pub fn is_complete(&self) -> bool {
        let filled = |text: &str| !text.trim().is_empty();
        let whole = |row: &ReadRow| {
            filled(&row.label)
                && !row.steps.is_empty()
                && row.steps.iter().all(|step| filled(&step.text))
                && row.wording.as_deref().is_none_or(filled)
        };
        self.rows
            .iter()
            .all(|row| whole(row) && filled(&row.printed))
            && self.extra.iter().all(whole)
    }
}

/// The sheet of a card's rules against its printed text.
///
/// The rules may be a proposal completed by the checker, a draft, or a card file or compiled row.
pub fn sheet(card: &Card, printed: &PrintedCard) -> ReadSheet {
    rows::build(card, printed)
}

/// The words of one ability as the sheet shows them: the renderer's first, else the first wording the checker accepts.
pub fn ability_wording(ability: &Ability) -> Option<String> {
    rows::wording(ability)
}

#[cfg(test)]
mod tests {
    //! Goal: a sheet is plain data that survives a round trip through JSON and says when a row is blank.
    use super::*;

    #[test]
    fn a_sheet_with_a_blank_step_is_not_complete() {
        let row = ReadRow {
            face: 0,
            span: Span { start: 0, end: 4 },
            kind: ClauseKind::Spell,
            printed: "Draw.".to_owned(),
            state: RowState::Written,
            abilities: vec![0],
            continues: false,
            label: "spell".to_owned(),
            wording: None,
            steps: vec![Step::new(0, "draw a card")],
        };
        let mut sheet = ReadSheet {
            card: "Test".into(),
            faces: vec!["Test".into()],
            rows: vec![row],
            extra: Vec::new(),
        };
        assert!(sheet.is_complete());
        let json = serde_json::to_string(&sheet).unwrap();
        assert_eq!(serde_json::from_str::<ReadSheet>(&json).unwrap(), sheet);
        sheet.rows[0].steps.push(Step::new(1, "  "));
        assert!(!sheet.is_complete());
    }
}
