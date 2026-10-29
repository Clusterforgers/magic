//! The solved cards nearest to a card: BM25 over the word and word-pair sets of the masked oracle texts, with a boost
//! for shared keywords and card types; ties go by name.

use std::collections::{BTreeSet, HashMap, HashSet};

use mtg_cards::PrintedCard;

use super::{W19bPair, W19bSplit};

/// BM25's saturation; with a set of terms it only shapes the length normalization.
const W19B_K1: f64 = 1.2;

/// BM25's length normalization.
const W19B_B: f64 = 0.75;

/// The score a shared Scryfall keyword adds.
const W19B_KEYWORD_BOOST: f64 = 2.0;

/// The score a shared card type adds.
const W19B_TYPE_BOOST: f64 = 1.0;

/// Rules longer than this many bytes are weighed down: they cost prompt tokens and teach one card.
const W19B_LONG_RULES: f64 = 4000.0;

/// The card types a type line can share.
const W19B_CARD_TYPES: &[&str] = &[
    "artifact",
    "battle",
    "creature",
    "enchantment",
    "instant",
    "kindred",
    "land",
    "planeswalker",
    "sorcery",
];

/// Number words, masked like digits.
const W19B_NUMBER_WORDS: &[&str] = &[
    "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
];

/// The text of a card as the index compares it: reminder text dropped, its own names as `~`, mana symbols as `{m}` (tap
/// and untap kept), numbers as `#`, lowercase.
pub fn w19b_mask(text: &str, names: &[String]) -> String {
    let mut plain = String::with_capacity(text.len());
    let mut depth = 0usize;
    for c in text.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if depth == 0 => plain.push(c),
            _ => {}
        }
    }
    // A legendary card's text calls it by the part of its name before the comma.
    let short = names
        .iter()
        .filter_map(|name| name.split_once(", ").map(|(first, _)| first));
    let mut all: Vec<&str> = names.iter().map(String::as_str).chain(short).collect();
    all.retain(|name| name.chars().count() >= 3);
    all.sort_by_key(|name| std::cmp::Reverse(name.len()));
    for name in all {
        plain = plain.replace(name, "~");
    }
    let mut out = String::with_capacity(plain.len());
    let mut rest = plain.as_str();
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let Some(close) = rest[open..].find('}') else {
            break;
        };
        let symbol = &rest[open + 1..open + close];
        out.push_str(match symbol {
            "T" => " {t} ",
            "Q" => " {q} ",
            _ => " {m} ",
        });
        rest = &rest[open + close + 1..];
    }
    out.push_str(rest);
    out.to_lowercase()
}

/// The terms of a masked text: its words (numbers as `#`) and its pairs of neighbouring words.
pub fn w19b_terms(masked: &str) -> BTreeSet<String> {
    let words: Vec<String> = masked
        .split(|c: char| !(c.is_alphanumeric() || "+-/~{}#".contains(c)))
        .filter(|word| !word.is_empty())
        .map(|word| {
            if W19B_NUMBER_WORDS.contains(&word) {
                "#".to_owned()
            } else {
                let mut masked = String::new();
                for c in word.chars() {
                    if c.is_ascii_digit() {
                        if !masked.ends_with('#') {
                            masked.push('#');
                        }
                    } else {
                        masked.push(c);
                    }
                }
                masked
            }
        })
        .collect();
    let mut terms: BTreeSet<String> = words
        .windows(2)
        .map(|pair| format!("{} {}", pair[0], pair[1]))
        .collect();
    terms.extend(words);
    terms
}

/// The card types of a type line, lowercase.
fn w19b_types(type_line: &str) -> BTreeSet<String> {
    type_line
        .to_lowercase()
        .split(|c: char| !c.is_alphabetic())
        .filter(|word| W19B_CARD_TYPES.contains(word))
        .map(str::to_owned)
        .collect()
}

/// What a card is compared by.
#[derive(Debug, Clone, Default)]
pub struct W19bQuery {
    /// The oracle id, never its own example.
    pub oracle_id: String,
    /// The name, never its own example either.
    pub name: String,
    /// The terms of the masked oracle text.
    pub terms: BTreeSet<String>,
    /// Scryfall's keywords, lowercase.
    pub keywords: BTreeSet<String>,
    /// The card types.
    pub types: BTreeSet<String>,
}

impl W19bQuery {
    /// The query of a card's text, names, keywords and type line.
    pub fn new(
        oracle_id: &str,
        names: &[String],
        oracle_text: &str,
        keywords: &[String],
        type_line: &str,
    ) -> Self {
        Self {
            oracle_id: oracle_id.to_owned(),
            name: names.first().cloned().unwrap_or_default(),
            terms: w19b_terms(&w19b_mask(oracle_text, names)),
            keywords: keywords.iter().map(|k| k.to_lowercase()).collect(),
            types: w19b_types(type_line),
        }
    }

    /// The query of a solved pair.
    pub fn of_pair(pair: &W19bPair) -> Self {
        Self::new(
            &pair.oracle_id,
            &pair.names,
            &pair.oracle_text,
            &pair.keywords,
            &pair.type_line,
        )
    }

    /// The query of a stored card.
    pub fn of_card(card: &PrintedCard) -> Self {
        let mut names = vec![card.name.clone()];
        names.extend(card.faces.iter().map(|face| face.name.clone()));
        let join = |field: fn(&mtg_cards::PrintedFace) -> &str, separator: &str| {
            card.faces
                .iter()
                .map(field)
                .collect::<Vec<_>>()
                .join(separator)
        };
        Self::new(
            &card.oracle_id,
            &names,
            &join(|face| &face.oracle_text, "\n\n"),
            &card.keywords,
            &join(|face| &face.type_line, " // "),
        )
    }
}

/// One indexed solved card.
#[derive(Debug, Clone)]
struct W19bDoc {
    query: W19bQuery,
    held_out: bool,
    rules_len: usize,
    rules_key: u64,
}

/// The solved cards, ready to be ranked against a card; built once per run.
#[derive(Debug, Clone, Default)]
pub struct W19bIndex {
    docs: Vec<W19bDoc>,
    idf: HashMap<String, f64>,
    average_len: f64,
}

/// A hash of the rules text, so that two cards with the same rules teach once.
fn w19b_rules_key(rules: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    rules.hash(&mut hasher);
    hasher.finish()
}

impl W19bIndex {
    /// The index of `pairs`; a result names a pair by its position.
    pub fn build(pairs: &[W19bPair]) -> Self {
        let docs: Vec<W19bDoc> = pairs
            .iter()
            .map(|pair| W19bDoc {
                query: W19bQuery::of_pair(pair),
                held_out: pair.split == W19bSplit::HeldOut,
                rules_len: pair.rules_text.len(),
                rules_key: w19b_rules_key(&pair.rules_text),
            })
            .collect();
        let mut df: HashMap<&str, usize> = HashMap::new();
        for doc in &docs {
            for term in &doc.query.terms {
                *df.entry(term.as_str()).or_default() += 1;
            }
        }
        #[allow(clippy::cast_precision_loss)]
        let n = docs.len() as f64;
        #[allow(clippy::cast_precision_loss)]
        let idf = df
            .into_iter()
            .map(|(term, count)| {
                let count = count as f64;
                (
                    term.to_owned(),
                    (1.0 + (n - count + 0.5) / (count + 0.5)).ln(),
                )
            })
            .collect();
        #[allow(clippy::cast_precision_loss)]
        let average_len = if docs.is_empty() {
            1.0
        } else {
            docs.iter().map(|doc| doc.query.terms.len()).sum::<usize>() as f64 / n
        };
        Self {
            docs,
            idf,
            average_len,
        }
    }

    /// How many pairs the index holds.
    pub fn len(&self) -> usize {
        self.docs.len()
    }

    /// Whether the index holds none.
    pub fn is_empty(&self) -> bool {
        self.docs.is_empty()
    }

    /// The score of a document for a query.
    fn score(&self, query: &W19bQuery, doc: &W19bDoc) -> f64 {
        #[allow(clippy::cast_precision_loss)]
        let length = doc.query.terms.len() as f64 / self.average_len.max(1.0);
        let norm = (W19B_K1 + 1.0) / (1.0 + W19B_K1 * (1.0 - W19B_B + W19B_B * length));
        let text: f64 = query
            .terms
            .intersection(&doc.query.terms)
            .map(|term| self.idf.get(term).copied().unwrap_or(0.0) * norm)
            .sum();
        #[allow(clippy::cast_precision_loss)]
        let boost = W19B_KEYWORD_BOOST
            * query.keywords.intersection(&doc.query.keywords).count() as f64
            + W19B_TYPE_BOOST * query.types.intersection(&doc.query.types).count() as f64;
        #[allow(clippy::cast_precision_loss)]
        let rules = doc.rules_len as f64;
        (text + boost) / (1.0 + rules / W19B_LONG_RULES)
    }

    /// The positions of the `count` pairs nearest to `query`, nearest first: never the card itself (by oracle id or
    /// name), never a held-out pair when `train_only`, one pair for each distinct rules text; ties go by name.
    pub fn nearest(&self, query: &W19bQuery, count: usize, train_only: bool) -> Vec<usize> {
        let mut scored: Vec<(f64, usize)> = self
            .docs
            .iter()
            .enumerate()
            .filter(|(_, doc)| {
                doc.query.oracle_id != query.oracle_id
                    && doc.query.name != query.name
                    && !(train_only && doc.held_out)
            })
            .map(|(at, doc)| (self.score(query, doc), at))
            .collect();
        scored.sort_by(|a, b| {
            b.0.total_cmp(&a.0)
                .then_with(|| self.docs[a.1].query.name.cmp(&self.docs[b.1].query.name))
        });
        let mut seen = HashSet::new();
        scored
            .into_iter()
            .filter(|(_, at)| seen.insert(self.docs[*at].rules_key))
            .map(|(_, at)| at)
            .take(count)
            .collect()
    }
}
