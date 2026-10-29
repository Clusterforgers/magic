//! `export-pairs`: every `Verified` card as an (oracle text, rules) training pair with a stable held-out split, and the
//! solved cards `propose --solved-examples` retrieves from (`docs/cards-dsl.md`, Proposer).

pub mod measure;
pub mod retrieve;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use mtg_cards::card::Card;
use mtg_cards::load::Library;
use mtg_cards::support::{Support, support};
use mtg_cards::validate::used_primitives;
use mtg_cards::{Caps, PrintedCard, builtin};
use mtg_db::Database;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::args::ExportPairsArgs;
use crate::audit::stored_cards;
use crate::error::CliError;
use crate::propose::examples::rules_json;

/// The share of the cards held out of training when `--held-out-percent` is not given.
pub const W19B_DEFAULT_HELD_OUT_PERCENT: u8 = 5;

/// The seed of the split when `--seed` is not given.
pub const W19B_DEFAULT_SEED: u64 = 0;

/// The layouts of cards no deck holds: tokens, emblems and art cards.
const W19B_TOKEN_LAYOUTS: &[&str] = &["token", "double_faced_token", "emblem", "art_series"];

/// Where a pair's rules come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum W19bSource {
    /// A hand-written file of the corpus (`crates/mtg-cards/cards/`).
    Corpus,
    /// A `card_rules` row the compiler wrote.
    Compiled,
}

/// Which part of the data a pair belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum W19bSplit {
    /// Examples and training data.
    Train,
    /// Kept out of training, to measure on.
    HeldOut,
}

/// The stable split: a hash of the seed and the oracle id puts `percent` of the cards in `HeldOut`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct W19bSplitter {
    /// The share held out, 0 to 100.
    pub percent: u8,
    /// The seed mixed into the hash.
    pub seed: u64,
}

impl Default for W19bSplitter {
    fn default() -> Self {
        Self {
            percent: W19B_DEFAULT_HELD_OUT_PERCENT,
            seed: W19B_DEFAULT_SEED,
        }
    }
}

impl W19bSplitter {
    /// The part of the card with this oracle id; the same id and seed always land in the same part.
    pub fn split(&self, oracle_id: &str) -> W19bSplit {
        let digest = Sha256::digest(format!("{}:{oracle_id}", self.seed).as_bytes());
        let mut head = [0u8; 8];
        head.copy_from_slice(&digest[..8]);
        if u64::from_be_bytes(head) % 100 < u64::from(self.percent) {
            W19bSplit::HeldOut
        } else {
            W19bSplit::Train
        }
    }
}

/// One solved card: its text as the proposer shows it and the rules the checker accepts for it.
#[derive(Debug, Clone, Serialize)]
pub struct W19bPair {
    /// The card's name.
    pub name: String,
    /// The oracle text, faces joined by a blank line.
    pub oracle_text: String,
    /// The type line, faces joined by ` // `.
    pub type_line: String,
    /// The mana cost, faces joined by ` // ` (empty when the card has none).
    pub mana_cost: String,
    /// The rules in the proposal format: `{"faces":[{"abilities":[...]}]}`.
    pub rules: serde_json::Value,
    /// Where the rules come from.
    pub source: W19bSource,
    /// Train or held out.
    pub split: W19bSplit,
    /// The oracle id.
    #[serde(skip)]
    pub oracle_id: String,
    /// The names the text calls the card by: its name and the names of its faces.
    #[serde(skip)]
    pub names: Vec<String>,
    /// Scryfall's keywords of the card.
    #[serde(skip)]
    pub keywords: Vec<String>,
    /// The rules as compact JSON, as an example shows them.
    #[serde(skip)]
    pub rules_text: String,
    /// The primitives the rules use (`Effect.Draw`, `Trigger.ZoneChange`).
    #[serde(skip)]
    pub vocabulary: BTreeSet<String>,
}

/// Whether the card is a token, an emblem or an art card rather than a card of a deck.
fn w19b_is_token(card: &PrintedCard) -> bool {
    W19B_TOKEN_LAYOUTS.contains(&card.layout.as_str())
        || card
            .faces
            .iter()
            .any(|face| face.type_line.starts_with("Token"))
}

/// The pair of a card and its rules.
fn w19b_pair_of(
    printed: &PrintedCard,
    card: &Card,
    source: W19bSource,
    splitter: W19bSplitter,
) -> Option<W19bPair> {
    let rules_text = rules_json(card);
    let rules = serde_json::from_str(&rules_text).ok()?;
    let joined = |field: fn(&mtg_cards::PrintedFace) -> &str, separator: &str| {
        printed
            .faces
            .iter()
            .map(field)
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join(separator)
    };
    let mut names = vec![printed.name.clone()];
    names.extend(printed.faces.iter().map(|face| face.name.clone()));
    Some(W19bPair {
        name: printed.name.clone(),
        oracle_text: joined(|face| &face.oracle_text, "\n\n"),
        type_line: joined(|face| &face.type_line, " // "),
        mana_cost: joined(|face| &face.mana_cost, " // "),
        rules,
        source,
        split: splitter.split(&printed.oracle_id),
        oracle_id: printed.oracle_id.clone(),
        names,
        keywords: printed.keywords.clone(),
        rules_text,
        vocabulary: card
            .abilities()
            .flat_map(|(_, ability)| used_primitives(ability))
            .collect(),
    })
}

/// The `Verified` non-token cards of `cards` with rules: the hand-written file of `library`, else the compiled rules of
/// `compiled` (by oracle id). Cards without rules text have no pair. In name order.
pub fn w19b_solved_pairs(
    cards: &[PrintedCard],
    library: &Library,
    compiled: &BTreeMap<String, Card>,
    caps: &Caps,
    splitter: W19bSplitter,
) -> Vec<W19bPair> {
    let mut pairs: Vec<W19bPair> = cards
        .iter()
        .filter(|printed| !w19b_is_token(printed))
        .filter_map(|printed| {
            let (card, source) = match library.card(&printed.oracle_id) {
                Some(card) => (card, W19bSource::Corpus),
                None => (compiled.get(&printed.oracle_id)?, W19bSource::Compiled),
            };
            let verified = support(Some(card), printed, caps).support == Support::Verified;
            let pair = w19b_pair_of(printed, card, source, splitter)?;
            (verified && !pair.vocabulary.is_empty()).then_some(pair)
        })
        .collect();
    pairs.sort_by(|a, b| {
        a.name
            .cmp(&b.name)
            .then_with(|| a.oracle_id.cmp(&b.oracle_id))
    });
    pairs
}

/// The solved pairs of the stored cards: the embedded hand-written rules and the stored `card_rules` rows.
pub async fn w19b_load_pairs(
    db: &Database,
    splitter: W19bSplitter,
) -> Result<Vec<W19bPair>, CliError> {
    let library = builtin::library().map_err(|errors| {
        CliError::Config(format!("the embedded rules files do not load:\n{errors}"))
    })?;
    let cards = stored_cards(db).await?;
    let compiled: BTreeMap<String, Card> = db
        .card_rules()
        .all()
        .await?
        .into_iter()
        .filter_map(|row| {
            let card: Card = serde_json::from_value(row.rules).ok()?;
            Some((row.oracle_id.as_str().to_owned(), card))
        })
        .collect();
    Ok(w19b_solved_pairs(
        &cards,
        library,
        &compiled,
        &mtg_engine::caps(),
        splitter,
    ))
}

/// The pairs as JSON Lines, one object per line.
pub fn w19b_jsonl(pairs: &[W19bPair]) -> Result<String, CliError> {
    let mut text = String::new();
    for pair in pairs {
        let line = serde_json::to_string(pair)
            .map_err(|error| CliError::Config(format!("a pair does not serialize: {error}")))?;
        text.push_str(&line);
        text.push('\n');
    }
    Ok(text)
}

/// The one-line summary: the pairs per source and split.
pub fn w19b_summary(pairs: &[W19bPair]) -> String {
    let mut counts: BTreeMap<(W19bSource, W19bSplit), usize> = BTreeMap::new();
    for pair in pairs {
        *counts.entry((pair.source, pair.split)).or_default() += 1;
    }
    let count = |source, split| counts.get(&(source, split)).copied().unwrap_or(0);
    let mut text = format!("{} pairs:", pairs.len());
    for (source, word) in [
        (W19bSource::Corpus, "corpus"),
        (W19bSource::Compiled, "compiled"),
    ] {
        let _ = write!(
            text,
            " {word} {} train + {} held out;",
            count(source, W19bSplit::Train),
            count(source, W19bSplit::HeldOut)
        );
    }
    text.pop();
    text
}

/// Runs `export-pairs`: writes the pairs to `--out` and returns the summary, then the measure when asked.
pub async fn w19b_export_pairs(db: &Database, args: &ExportPairsArgs) -> Result<String, CliError> {
    let splitter = W19bSplitter {
        percent: args.held_out_percent,
        seed: args.seed,
    };
    let pairs = w19b_load_pairs(db, splitter).await?;
    tokio::fs::write(&args.out, w19b_jsonl(&pairs)?)
        .await
        .map_err(|source| CliError::Io {
            path: args.out.clone(),
            source,
        })?;
    let mut text = w19b_summary(&pairs);
    if args.measure {
        text.push('\n');
        text.push_str(&measure::w19b_measure(&pairs, &[3, 5, 8]).render());
    }
    Ok(text)
}

#[cfg(test)]
mod tests;
