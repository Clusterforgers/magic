//! The solved-card examples of `propose --solved-examples K`: the K stored `Verified` cards (hand-written or compiled)
//! nearest to the asked card, from an index built once per run.

use mtg_cards::PrintedCard;
use mtg_db::Database;

use super::examples::Example;
use crate::error::CliError;
use crate::w19b_pairs::retrieve::{W19bIndex, W19bQuery};
use crate::w19b_pairs::{W19bPair, W19bSplitter, w19b_load_pairs};

/// The solved cards as examples and their index.
#[derive(Debug, Clone, Default)]
pub struct W19bSolvedExamples {
    examples: Vec<Example>,
    index: W19bIndex,
    count: usize,
    train_only: bool,
}

impl W19bSolvedExamples {
    /// The examples of `pairs`, `count` for each card, only from the train part when `train_only`.
    pub fn new(pairs: &[W19bPair], count: usize, train_only: bool) -> Self {
        Self {
            examples: pairs
                .iter()
                .map(|pair| {
                    Example::plain(
                        &pair.name,
                        &pair.type_line,
                        &pair.oracle_text,
                        &pair.rules_text,
                    )
                })
                .collect(),
            index: W19bIndex::build(pairs),
            count,
            train_only,
        }
    }

    /// The solved examples of the stored cards, with the default split of `export-pairs`.
    pub async fn load(db: &Database, count: usize, train_only: bool) -> Result<Self, CliError> {
        let pairs = w19b_load_pairs(db, W19bSplitter::default()).await?;
        Ok(Self::new(&pairs, count, train_only))
    }

    /// How many solved cards the index holds.
    pub fn len(&self) -> usize {
        self.examples.len()
    }

    /// Whether it holds none.
    pub fn is_empty(&self) -> bool {
        self.examples.is_empty()
    }

    /// The examples nearest to `card`, nearest first, never the card itself.
    pub fn nearest(&self, card: &PrintedCard) -> Vec<&Example> {
        self.index
            .nearest(&W19bQuery::of_card(card), self.count, self.train_only)
            .into_iter()
            .filter_map(|at| self.examples.get(at))
            .collect()
    }
}
