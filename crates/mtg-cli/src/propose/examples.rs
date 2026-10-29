//! The hand-written rules nearest to a card, by the words, word pairs and keywords their oracle texts share.

use std::collections::{BTreeMap, HashMap, HashSet};

use mtg_cards::PrintedCard;
use mtg_cards::ability::Ability;
use mtg_cards::card::{Card, ImplStatus};
use mtg_cards::load::Library;
use serde::Serialize;

use super::w19b_solved::W19bSolvedExamples;

/// Rules longer than this many bytes are weighed down: they cost prompt tokens and teach one card.
const LONG_RULES: f64 = 3000.0;

/// Words that carry no mechanic.
const STOPWORDS: &[&str] = &[
    "the", "and", "you", "your", "that", "this", "with", "for", "from", "may", "its", "his", "her",
    "their", "them", "then", "than", "has", "have", "had", "any", "all", "one", "not", "can",
    "are", "was", "were", "until", "when", "whenever", "would", "instead", "each", "other",
    "those", "these", "there", "where", "while", "does", "did",
];

/// A hand-written card as the prompt shows it.
#[derive(Debug, Clone)]
pub struct Example {
    /// The card's name.
    pub name: String,
    /// The card's type line (faces joined by ` // `).
    pub type_line: String,
    /// The oracle text (faces joined by a blank line).
    pub oracle_text: String,
    /// The abilities of every face as compact JSON: `{"faces":[{"abilities":[...]},...]}`.
    pub rules: String,
    terms: Terms,
}

impl Example {
    /// An example that is never near anything: for tests of the prompt.
    pub fn plain(name: &str, type_line: &str, oracle_text: &str, rules: &str) -> Self {
        Self {
            name: name.to_owned(),
            type_line: type_line.to_owned(),
            oracle_text: oracle_text.to_owned(),
            rules: rules.to_owned(),
            terms: Terms::default(),
        }
    }
}

/// Weighted terms of a text.
#[derive(Debug, Clone, Default)]
struct Terms {
    weights: BTreeMap<String, u32>,
    total: u64,
}

impl Terms {
    fn add(&mut self, term: String, weight: u32) {
        if self.weights.insert(term, weight).is_none() {
            self.total += u64::from(weight);
        }
    }

    /// The shared weight over the root of the product of the totals.
    fn similarity(&self, other: &Terms) -> f64 {
        let (small, large) = if self.weights.len() <= other.weights.len() {
            (self, other)
        } else {
            (other, self)
        };
        let shared: u64 = small
            .weights
            .iter()
            .filter(|(term, _)| large.weights.contains_key(*term))
            .map(|(_, weight)| u64::from(*weight))
            .sum();
        #[allow(clippy::cast_precision_loss)]
        let (shared, a, b) = (shared as f64, self.total as f64, other.total as f64);
        if a == 0.0 || b == 0.0 {
            0.0
        } else {
            shared / (a * b).sqrt()
        }
    }
}

/// The terms of a card: its words (1), word pairs (2), Scryfall keywords (3) and the words of its type line (1).
fn terms_of(card: &PrintedCard) -> Terms {
    let mut terms = Terms::default();
    let mut text = card
        .faces
        .iter()
        .map(|face| face.oracle_text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    for name in names_of(card) {
        text = text.replace(&name, "~");
    }
    let words: Vec<String> = text
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '{' && c != '}')
        .filter(|word| word.chars().count() >= 3 || word.starts_with('{'))
        .map(str::to_owned)
        .collect();
    for pair in words.windows(2) {
        terms.add(format!("{} {}", pair[0], pair[1]), 2);
    }
    for word in words {
        if !STOPWORDS.contains(&word.as_str()) {
            terms.add(word, 1);
        }
    }
    for keyword in &card.keywords {
        terms.add(format!("kw:{}", keyword.to_lowercase()), 3);
    }
    for face in &card.faces {
        for word in face
            .type_line
            .to_lowercase()
            .split(|c: char| !c.is_alphabetic())
            .filter(|word| word.len() >= 3)
        {
            terms.add(format!("type:{word}"), 1);
        }
    }
    terms
}

/// The name of the card, its faces and the part of a legendary name before the comma: what the text calls it.
fn names_of(card: &PrintedCard) -> Vec<String> {
    let mut names = vec![card.name.clone()];
    names.extend(card.faces.iter().map(|face| face.name.clone()));
    let short: Vec<String> = names
        .iter()
        .filter_map(|name| name.split_once(", "))
        .map(|(first, _)| first.to_owned())
        .collect();
    names.extend(short);
    names.retain(|name| name.chars().count() >= 3);
    names.sort_by_key(|name| std::cmp::Reverse(name.len()));
    names.dedup();
    names
}

/// The abilities of a card's faces as compact JSON in serde's field order, the shape a proposal has.
pub(crate) fn rules_json(card: &Card) -> String {
    #[derive(Serialize)]
    struct Rules<'a> {
        faces: Vec<FaceRules<'a>>,
    }
    #[derive(Serialize)]
    struct FaceRules<'a> {
        abilities: &'a [Ability],
    }
    let rules = Rules {
        faces: card
            .faces
            .iter()
            .map(|face| FaceRules {
                abilities: &face.abilities,
            })
            .collect(),
    };
    serde_json::to_string(&rules).unwrap_or_default()
}

/// The hand-written cards whose texts are stored, ready to be ranked against a card.
#[derive(Debug, Default)]
pub struct ExampleIndex {
    examples: Vec<Example>,
    w19b_solved: Option<W19bSolvedExamples>,
}

impl ExampleIndex {
    /// The whole hand-written cards (`Full`, nothing left to the table) that `stored` has the text of.
    pub fn build(library: &Library, stored: &[PrintedCard]) -> Self {
        let by_id: HashMap<&str, &PrintedCard> = stored
            .iter()
            .map(|card| (card.oracle_id.as_str(), card))
            .collect();
        let mut examples: Vec<Example> = library
            .cards()
            .filter(|card| card.impl_status == ImplStatus::Full)
            .filter_map(|rules| {
                let printed = by_id.get(rules.oracle_id.as_str())?;
                Some(Example {
                    name: printed.name.clone(),
                    type_line: printed
                        .faces
                        .iter()
                        .map(|face| face.type_line.as_str())
                        .collect::<Vec<_>>()
                        .join(" // "),
                    oracle_text: printed
                        .faces
                        .iter()
                        .map(|face| face.oracle_text.as_str())
                        .collect::<Vec<_>>()
                        .join("\n\n"),
                    rules: rules_json(rules),
                    terms: terms_of(printed),
                })
            })
            .collect();
        examples.sort_by(|a, b| a.name.cmp(&b.name));
        Self {
            examples,
            w19b_solved: None,
        }
    }

    /// The index that answers with the nearest solved cards instead of the hand-written ones.
    #[must_use]
    pub fn w19b_with_solved(self, solved: W19bSolvedExamples) -> Self {
        Self {
            w19b_solved: Some(solved),
            ..self
        }
    }

    /// How many examples the index holds.
    pub fn len(&self) -> usize {
        self.examples.len()
    }

    /// Whether the index holds none.
    pub fn is_empty(&self) -> bool {
        self.examples.is_empty()
    }

    /// The `count` examples nearest to `card`, nearest first, never the card itself.
    pub fn nearest(&self, card: &PrintedCard, count: usize) -> Vec<&Example> {
        if let Some(solved) = &self.w19b_solved {
            return solved.nearest(card);
        }
        let wanted = terms_of(card);
        let mut scored: Vec<(f64, &Example)> = self
            .examples
            .iter()
            .filter(|example| example.name != card.name)
            .map(|example| {
                #[allow(clippy::cast_precision_loss)]
                let length = example.rules.len() as f64;
                (
                    wanted.similarity(&example.terms) / (1.0 + length / LONG_RULES),
                    example,
                )
            })
            .collect();
        scored.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.name.cmp(&b.1.name)));
        // Two cards with the same rules teach once.
        let mut seen = HashSet::new();
        scored
            .into_iter()
            .map(|(_, example)| example)
            .filter(|example| seen.insert(example.rules.as_str()))
            .take(count)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the nearest examples share the keywords, word pairs and type of the card, never include the card itself, and
    //! the order is the same on every run.
    use mtg_cards::PrintedFace;

    use super::*;

    fn printed(name: &str, type_line: &str, text: &str, keywords: &[&str]) -> PrintedCard {
        PrintedCard {
            oracle_id: format!("id-{name}"),
            name: name.to_owned(),
            keywords: keywords.iter().map(|k| (*k).to_owned()).collect(),
            layout: "normal".into(),
            faces: vec![PrintedFace {
                name: name.to_owned(),
                type_line: type_line.to_owned(),
                oracle_text: text.to_owned(),
                ..PrintedFace::default()
            }],
        }
    }

    fn example(card: &PrintedCard, rules: &str) -> Example {
        Example {
            name: card.name.clone(),
            type_line: card.faces[0].type_line.clone(),
            oracle_text: card.faces[0].oracle_text.clone(),
            // `{}` stands for rules of the card's own, so that no two examples are the same.
            rules: if rules == "{}" {
                format!("{{\"card\":\"{}\"}}", card.name)
            } else {
                rules.to_owned()
            },
            terms: terms_of(card),
        }
    }

    #[test]
    fn the_cards_own_name_is_not_a_term() {
        let card = printed(
            "Serra Angel",
            "Creature",
            "When Serra Angel enters, draw a card.",
            &[],
        );
        let terms = terms_of(&card);
        assert!(terms.weights.contains_key("draw"));
        assert!(!terms.weights.keys().any(|term| term.contains("serra")));
    }

    #[test]
    fn the_short_name_of_a_legendary_card_is_not_a_term_either() {
        let card = printed(
            "Roon, Wanderer",
            "Legendary Creature",
            "Whenever Roon attacks, exile target creature.",
            &[],
        );
        assert!(
            !terms_of(&card)
                .weights
                .keys()
                .any(|term| term.contains("roon"))
        );
    }

    #[test]
    fn keywords_word_pairs_and_the_type_line_pull_a_card_closer() {
        let wanted = printed(
            "Wanted",
            "Creature — Bird",
            "Flying\nWhenever this creature attacks, draw a card.",
            &["Flying"],
        );
        let near = printed(
            "Near",
            "Creature — Drake",
            "Flying\nWhenever this creature attacks, you gain 1 life.",
            &["Flying"],
        );
        let far = printed("Far", "Sorcery", "Destroy target land.", &[]);
        let index = ExampleIndex {
            examples: vec![example(&far, "{}"), example(&near, "{}")],
            ..ExampleIndex::default()
        };
        let found: Vec<&str> = index
            .nearest(&wanted, 2)
            .iter()
            .map(|e| e.name.as_str())
            .collect();
        assert_eq!(found, ["Near", "Far"]);
    }

    #[test]
    fn the_card_itself_is_never_its_own_example_and_ties_go_by_name() {
        let wanted = printed("Same", "Instant", "Draw a card.", &[]);
        let a = printed("Bravo", "Instant", "Draw a card.", &[]);
        let b = printed("Alpha", "Instant", "Draw a card.", &[]);
        let index = ExampleIndex {
            examples: vec![example(&wanted, "{}"), example(&a, "{}"), example(&b, "{}")],
            ..ExampleIndex::default()
        };
        let found: Vec<&str> = index
            .nearest(&wanted, 5)
            .iter()
            .map(|e| e.name.as_str())
            .collect();
        assert_eq!(found, ["Alpha", "Bravo"]);
    }

    #[test]
    fn cards_with_the_same_rules_are_one_example() {
        let wanted = printed("W", "Creature", "Shadow", &["Shadow"]);
        let a = printed("Alpha", "Creature", "Shadow", &["Shadow"]);
        let b = printed("Beta", "Creature", "Shadow", &["Shadow"]);
        let c = printed("Gamma", "Creature", "Shadow and more", &["Shadow"]);
        let index = ExampleIndex {
            examples: vec![
                example(&a, "same"),
                example(&b, "same"),
                example(&c, "other"),
            ],
            ..ExampleIndex::default()
        };
        let found: Vec<&str> = index
            .nearest(&wanted, 5)
            .iter()
            .map(|e| e.name.as_str())
            .collect();
        assert_eq!(found, ["Alpha", "Gamma"]);
    }

    #[test]
    fn long_rules_rank_below_short_ones_of_the_same_wording() {
        let wanted = printed("W", "Instant", "Draw a card.", &[]);
        let long = printed("Long", "Instant", "Draw a card.", &[]);
        let short = printed("Short", "Instant", "Draw a card.", &[]);
        let index = ExampleIndex {
            examples: vec![example(&long, &"x".repeat(9000)), example(&short, "{}")],
            ..ExampleIndex::default()
        };
        assert_eq!(index.nearest(&wanted, 1)[0].name, "Short");
    }
}
