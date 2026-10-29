//! An offline measure of the retrieval, with no model: how often the DSL vocabulary of the K examples retrieved for a
//! held-out card covers the vocabulary of the card's own rules, against K examples taken at random.

#![allow(clippy::cast_precision_loss)] // counts of a few thousand cards fit a float exactly

use std::collections::BTreeSet;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use super::retrieve::{W19bIndex, W19bQuery};
use super::{W19bPair, W19bSplit};

/// The kinds the measure counts: effects, triggers, conditions and modifications.
const W19B_KINDS: &[&str] = &["Effect.", "Trigger.", "Condition.", "Mod."];

/// The random draws averaged for the baseline of each card.
const W19B_RANDOM_DRAWS: u64 = 5;

/// The effect, trigger, condition and modification kinds of a pair's rules.
pub fn w19b_vocabulary(pair: &W19bPair) -> BTreeSet<&str> {
    pair.vocabulary
        .iter()
        .map(String::as_str)
        .filter(|kind| W19B_KINDS.iter().any(|prefix| kind.starts_with(prefix)))
        .collect()
}

/// How well one way of choosing K examples covers the held-out cards.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct W19bCoverage {
    /// The share of the cards whose every kind some example uses, 0 to 1.
    pub full: f64,
    /// The mean share of a card's kinds that some example uses, 0 to 1.
    pub mean: f64,
}

/// The measure for each K: the nearest examples and the random baseline.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct W19bMeasure {
    /// The held-out cards measured (those whose rules use a counted kind).
    pub cards: usize,
    /// The train pairs examples are taken from.
    pub train: usize,
    /// `(K, nearest, random)` for each K asked.
    pub rows: Vec<(usize, W19bCoverage, W19bCoverage)>,
}

impl W19bMeasure {
    /// The measure as a few lines of text.
    pub fn render(&self) -> String {
        let mut text = format!(
            "vocabulary coverage of {} held-out cards by examples from {} train pairs (full: every kind covered; mean: share of kinds)",
            self.cards, self.train
        );
        for (k, near, random) in &self.rows {
            let _ = write!(
                text,
                "\nK={k}: nearest full {:.1}% mean {:.1}% | random full {:.1}% mean {:.1}%",
                near.full * 100.0,
                near.mean * 100.0,
                random.full * 100.0,
                random.mean * 100.0
            );
        }
        text
    }
}

/// The share of `wanted` the union of the vocabularies of `examples` covers.
fn w19b_covered(wanted: &BTreeSet<&str>, examples: &[&W19bPair]) -> f64 {
    let have: BTreeSet<&str> = examples
        .iter()
        .flat_map(|pair| w19b_vocabulary(pair))
        .collect();
    wanted.intersection(&have).count() as f64 / wanted.len().max(1) as f64
}

/// `count` train pairs other than the card, chosen by a hash of the card's oracle id and the draw.
fn w19b_random<'a>(
    train: &[&'a W19bPair],
    card: &W19bPair,
    count: usize,
    draw: u64,
) -> Vec<&'a W19bPair> {
    let mut ranked: Vec<(u64, &W19bPair)> = train
        .iter()
        .filter(|pair| pair.oracle_id != card.oracle_id)
        .map(|pair| {
            let digest =
                Sha256::digest(format!("{draw}:{}:{}", card.oracle_id, pair.oracle_id).as_bytes());
            let mut head = [0u8; 8];
            head.copy_from_slice(&digest[..8]);
            (u64::from_be_bytes(head), *pair)
        })
        .collect();
    ranked.sort_by_key(|(key, _)| *key);
    ranked
        .into_iter()
        .take(count)
        .map(|(_, pair)| pair)
        .collect()
}

/// Adds one card's share to a running total of full covers and shares.
fn w19b_add(total: &mut (f64, f64), share: f64, weight: f64) {
    if share >= 1.0 {
        total.0 += weight;
    }
    total.1 += share * weight;
}

/// The measure over the held-out pairs of `pairs` for each K in `ks`, examples taken from the train pairs only.
pub fn w19b_measure(pairs: &[W19bPair], ks: &[usize]) -> W19bMeasure {
    let index = W19bIndex::build(pairs);
    let train: Vec<&W19bPair> = pairs
        .iter()
        .filter(|pair| pair.split == W19bSplit::Train)
        .collect();
    let held: Vec<&W19bPair> = pairs
        .iter()
        .filter(|pair| pair.split == W19bSplit::HeldOut && !w19b_vocabulary(pair).is_empty())
        .collect();
    let n = held.len().max(1) as f64;
    let rows = ks
        .iter()
        .map(|&k| {
            let (mut near, mut random) = ((0.0, 0.0), (0.0, 0.0));
            for card in &held {
                let wanted = w19b_vocabulary(card);
                let found: Vec<&W19bPair> = index
                    .nearest(&W19bQuery::of_pair(card), k, true)
                    .into_iter()
                    .filter_map(|at| pairs.get(at))
                    .collect();
                w19b_add(&mut near, w19b_covered(&wanted, &found), 1.0);
                for draw in 0..W19B_RANDOM_DRAWS {
                    let chosen = w19b_random(&train, card, k, draw);
                    let weight = 1.0 / W19B_RANDOM_DRAWS as f64;
                    w19b_add(&mut random, w19b_covered(&wanted, &chosen), weight);
                }
            }
            let coverage = |(full, mean): (f64, f64)| W19bCoverage {
                full: full / n,
                mean: mean / n,
            };
            (k, coverage(near), coverage(random))
        })
        .collect();
    W19bMeasure {
        cards: held.len(),
        train: train.len(),
        rows,
    }
}
