//! The equivalences: each rule rewrites a word sequence to a canonical form, applied to both wordings alike.

use serde::{Deserialize, Serialize};

/// One named equivalence between wordings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rule {
    /// "this creature", "this artifact" and the like name the card itself, as `~` does.
    SelfName,
    /// "it's" reads as "it is", "you've cast" as "you cast", "doesn't" as "does not".
    Contraction,
    /// "she", "her", "he", "him" and "his" read as "it" and "its".
    Pronoun,
    /// "that creature" (or permanent, card, ...) reads as "it", "those creatures" and "them" as "they".
    BackReference,
    /// "and it gains" reads as "and gains": a second verb of the same subject.
    Elision,
    /// "and/or" in a list of kinds reads as "or".
    AndOr,
    /// "Until end of turn, X" reads as "X until end of turn".
    DurationOrder,
    /// "is put into a graveyard from the battlefield" reads as "dies" (rule 700.4).
    Dies,
    /// "Discard X cards, where X is the number of cards in your hand" reads as "Discard your hand".
    DiscardHand,
    /// "a number of N equal to E" reads as "X N, where X is E".
    CountedAsX,
    /// "add three {R}" reads as "add {R}{R}{R}".
    ManaCount,
    /// "on the bottom of a library in any order" reads as "on the bottom of a library" (rule 401.4).
    AnyOrder,
    /// A doubled word ("then then"), "also", "an" for "a", case, commas and periods.
    Punctuation,
}

impl Rule {
    /// Every rule, in the order they are applied.
    pub const ALL: &'static [Rule] = &[
        Rule::SelfName,
        Rule::Contraction,
        Rule::Pronoun,
        Rule::BackReference,
        Rule::Elision,
        Rule::AndOr,
        Rule::DurationOrder,
        Rule::Dies,
        Rule::DiscardHand,
        Rule::CountedAsX,
        Rule::ManaCount,
        Rule::AnyOrder,
        Rule::Punctuation,
    ];

    /// The rule's name, as the report and the wire say it.
    pub const fn name(self) -> &'static str {
        match self {
            Rule::SelfName => "self_name",
            Rule::Contraction => "contraction",
            Rule::Pronoun => "pronoun",
            Rule::BackReference => "back_reference",
            Rule::Elision => "elision",
            Rule::AndOr => "and_or",
            Rule::DurationOrder => "duration_order",
            Rule::Dies => "dies",
            Rule::DiscardHand => "discard_hand",
            Rule::CountedAsX => "counted_as_x",
            Rule::ManaCount => "mana_count",
            Rule::AnyOrder => "any_order",
            Rule::Punctuation => "punctuation",
        }
    }

    /// The rule's heading, in a few words.
    pub const fn title(self) -> &'static str {
        match self {
            Rule::SelfName => "The card's own name",
            Rule::Contraction => "Contractions",
            Rule::Pronoun => "Pronouns",
            Rule::BackReference => "\"that creature\" as \"it\"",
            Rule::Elision => "A repeated subject",
            Rule::AndOr => "\"and/or\" as \"or\"",
            Rule::DurationOrder => "Duration first",
            Rule::Dies => "\"dies\" spelled out",
            Rule::DiscardHand => "Discard your hand",
            Rule::CountedAsX => "A count named X",
            Rule::ManaCount => "Mana symbols counted",
            Rule::AnyOrder => "\"in any order\" left out",
            Rule::Punctuation => "Punctuation and filler",
        }
    }

    /// What a reader checks when a card is listed under the rule.
    pub const fn summary(self) -> &'static str {
        match self {
            Rule::SelfName => {
                "\"this creature\" (or artifact, land, ...) and ~ both name the card."
            }
            Rule::Contraction => "\"it's\" is \"it is\" and \"you've cast\" is \"you cast\".",
            Rule::Pronoun => "she, her, he and him stand where the card says it and its.",
            Rule::BackReference => {
                "\"that creature\", \"those creatures\" and \"them\" point at the same objects as \"it\" and \"they\"."
            }
            Rule::Elision => "\"and it gains\" and \"and gains\" have the same subject.",
            Rule::AndOr => "\"and/or\" in a list of kinds reads as \"or\".",
            Rule::DurationOrder => "\"until end of turn\" stands at the front instead of the end.",
            Rule::Dies => {
                "\"is put into a graveyard from the battlefield\" is what \"dies\" means."
            }
            Rule::DiscardHand => {
                "Discarding as many cards as are in your hand is discarding your hand."
            }
            Rule::CountedAsX => {
                "\"a number of counters equal to N\" is \"X counters, where X is N\"."
            }
            Rule::ManaCount => "\"three {R}\" is {R}{R}{R}.",
            Rule::AnyOrder => "Cards put on the bottom together go in any order their owner likes.",
            Rule::Punctuation => {
                "Only case, commas, periods, a doubled word, \"also\" or \"a\" for \"an\" differ."
            }
        }
    }

    fn apply(self, words: &mut Vec<String>) {
        match self {
            Rule::SelfName => self_name(words),
            Rule::Contraction => contraction(words),
            Rule::Pronoun => map_words(
                words,
                &[
                    ("she", "it"),
                    ("he", "it"),
                    ("him", "it"),
                    ("her", "it"),
                    ("his", "it"),
                    ("its", "it"),
                    ("herself", "itself"),
                    ("himself", "itself"),
                ],
            ),
            Rule::BackReference => back_reference(words),
            Rule::Elision => elision(words),
            Rule::AndOr => map_words(words, &[("and/or", "or")]),
            Rule::DurationOrder => duration_order(words),
            Rule::Dies => {
                replace(
                    words,
                    &words_of("is put into a graveyard from the battlefield"),
                    &["dies"],
                );
                replace(
                    words,
                    &words_of("are put into a graveyard from the battlefield"),
                    &["die"],
                );
            }
            Rule::DiscardHand => replace(
                words,
                &words_of("discard x cards, where x is the number of cards in your hand"),
                &["discard", "your", "hand"],
            ),
            Rule::CountedAsX => counted_as_x(words),
            Rule::ManaCount => mana_count(words),
            Rule::AnyOrder => any_order(words),
            Rule::Punctuation => punctuation(words),
        }
    }
}

/// The words and punctuation marks of a wording, lower case; each mana symbol is a word of its own.
pub(super) fn words(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut Vec<String>| {
        if !word.is_empty() {
            out.push(std::mem::take(word));
        }
    };
    for ch in text.chars() {
        match ch {
            c if c.is_whitespace() => flush(&mut word, &mut out),
            ',' | '.' | ':' | ';' | '—' | '•' | '"' => {
                flush(&mut word, &mut out);
                out.push(ch.to_string());
            }
            '{' => {
                flush(&mut word, &mut out);
                word.push('{');
            }
            '}' => {
                word.push('}');
                flush(&mut word, &mut out);
            }
            '’' => word.push('\''),
            c => word.extend(c.to_lowercase()),
        }
    }
    flush(&mut word, &mut out);
    out
}

fn words_of(text: &str) -> Vec<String> {
    words(text)
}

/// The rules of `allowed` that the match needs, or `None` when the canonical forms differ.
pub(super) fn matching(printed: &str, written: &str, allowed: &[Rule]) -> Option<Vec<Rule>> {
    let mut left = words(printed);
    let mut right = words(written);
    if left == right {
        return Some(Vec::new());
    }
    let mut used = Vec::new();
    for rule in Rule::ALL.iter().copied().filter(|r| allowed.contains(r)) {
        let (before_left, before_right) = (left.clone(), right.clone());
        rule.apply(&mut left);
        rule.apply(&mut right);
        if left != before_left || right != before_right {
            used.push(rule);
        }
        if left == right {
            return Some(used);
        }
    }
    None
}

/// Whether `words` holds `pattern` at `at`.
fn at<S: AsRef<str>>(words: &[String], at: usize, pattern: &[S]) -> bool {
    words.len() >= at + pattern.len()
        && words[at..at + pattern.len()]
            .iter()
            .zip(pattern)
            .all(|(w, p)| w == p.as_ref())
}

/// Replaces every run of `from` with `to`.
fn replace<S: AsRef<str>>(words: &mut Vec<String>, from: &[S], to: &[&str]) {
    let mut out = Vec::with_capacity(words.len());
    let mut i = 0;
    while i < words.len() {
        if !from.is_empty() && at(words, i, from) {
            out.extend(to.iter().map(|&t| t.to_owned()));
            i += from.len();
        } else {
            out.push(words[i].clone());
            i += 1;
        }
    }
    *words = out;
}

/// Replaces single words.
fn map_words(words: &mut [String], pairs: &[(&str, &str)]) {
    for word in words.iter_mut() {
        if let Some((_, to)) = pairs.iter().find(|(from, _)| word == from) {
            *word = (*to).to_owned();
        }
    }
}

/// The nouns "this <noun>" names the card by.
const SELF_NOUNS: &[&str] = &[
    "creature",
    "artifact",
    "enchantment",
    "land",
    "permanent",
    "planeswalker",
    "spell",
    "card",
    "vehicle",
    "equipment",
    "aura",
    "battle",
    "saga",
    "class",
    "room",
    "case",
];

fn self_name(words: &mut Vec<String>) {
    let mut out = Vec::with_capacity(words.len());
    let mut i = 0;
    while i < words.len() {
        let noun = words.get(i + 1).map(|n| {
            let possessive = n.ends_with("'s");
            (n.strip_suffix("'s").unwrap_or(n), possessive)
        });
        match noun {
            Some((noun, possessive)) if words[i] == "this" && SELF_NOUNS.contains(&noun) => {
                out.push(if possessive { "~'s" } else { "~" }.to_owned());
                i += 2;
            }
            _ => {
                out.push(words[i].clone());
                i += 1;
            }
        }
    }
    *words = out;
}

/// Contractions and their long forms; "you've" before a past participle is the simple past.
const CONTRACTIONS: &[(&str, &[&str])] = &[
    ("it's", &["it", "is"]),
    ("that's", &["that", "is"]),
    ("isn't", &["is", "not"]),
    ("aren't", &["are", "not"]),
    ("doesn't", &["does", "not"]),
    ("don't", &["do", "not"]),
    ("didn't", &["did", "not"]),
    ("you've", &["you"]),
];

fn contraction(words: &mut Vec<String>) {
    let mut out = Vec::with_capacity(words.len());
    for word in words.iter() {
        match CONTRACTIONS.iter().find(|(short, _)| word == short) {
            Some((_, long)) => out.extend(long.iter().map(|&w| w.to_owned())),
            None => out.push(word.clone()),
        }
    }
    *words = out;
}

/// The nouns "that <noun>" and "those <nouns>" point back with.
const BACK_NOUNS: &[&str] = &[
    "creature",
    "artifact",
    "enchantment",
    "land",
    "permanent",
    "planeswalker",
    "token",
    "spell",
    "card",
];

fn back_reference(words: &mut Vec<String>) {
    let mut out = Vec::with_capacity(words.len());
    let mut i = 0;
    while i < words.len() {
        let next = words.get(i + 1).map(String::as_str);
        let singular = next.is_some_and(|n| BACK_NOUNS.contains(&n));
        let plural = next
            .and_then(|n| n.strip_suffix('s'))
            .is_some_and(|n| BACK_NOUNS.contains(&n));
        let possessive = next
            .and_then(|n| n.strip_suffix("'s"))
            .is_some_and(|n| BACK_NOUNS.contains(&n));
        match words[i].as_str() {
            "that" if singular => {
                out.push("it".to_owned());
                i += 2;
            }
            "that" if possessive => {
                out.push("it".to_owned());
                i += 2;
            }
            "those" if plural => {
                out.push("they".to_owned());
                i += 2;
            }
            "them" => {
                out.push("they".to_owned());
                i += 1;
            }
            _ => {
                out.push(words[i].clone());
                i += 1;
            }
        }
    }
    *words = out;
}

/// "and it gains" as "and gains": a pronoun subject right after "and" goes.
fn elision(words: &mut Vec<String>) {
    let mut out: Vec<String> = Vec::with_capacity(words.len());
    for word in words.iter() {
        let after_and = out.last().is_some_and(|w| w == "and");
        if after_and && (word == "it" || word == "they") {
            continue;
        }
        out.push(word.clone());
    }
    *words = out;
}

/// Moves a sentence's leading "until end of turn ," to its end, before the period.
fn duration_order(words: &mut Vec<String>) {
    const LEAD: [&str; 5] = ["until", "end", "of", "turn", ","];
    const BREAKS: [&str; 3] = [".", "then", ":"];
    let mut out: Vec<String> = Vec::with_capacity(words.len());
    let mut i = 0;
    while i < words.len() {
        let starts = out
            .last()
            .is_none_or(|w| BREAKS.contains(&w.as_str()) || w == ",");
        if starts && at(words, i, &LEAD) {
            let rest = i + LEAD.len();
            let end = words[rest..]
                .iter()
                .position(|w| BREAKS.contains(&w.as_str()))
                .map_or(words.len(), |p| rest + p);
            out.extend(words[rest..end].iter().cloned());
            out.extend(["until", "end", "of", "turn"].map(str::to_owned));
            i = end;
        } else {
            out.push(words[i].clone());
            i += 1;
        }
    }
    *words = out;
}

/// "a number of N … equal to E" as "x N … , where x is E", within one sentence, and "cards equal to E" as "x cards ,
/// where x is E".
fn counted_as_x(words: &mut Vec<String>) {
    if let Some(cards) = (1..words.len()).find(|&i| {
        at(words, i, &["cards", "equal", "to"])
            && !NUMBERS.contains(&words[i - 1].as_str())
            && words[i - 1] != "x"
    }) {
        let mut out: Vec<String> = words[..cards].to_vec();
        out.extend(["x", "cards", ",", "where", "x", "is"].map(str::to_owned));
        out.extend(words[cards + 3..].iter().cloned());
        *words = out;
    }
    let Some(start) = (0..words.len()).find(|&i| at(words, i, &["a", "number", "of"])) else {
        return;
    };
    let Some(equal) = (start + 3..words.len())
        .take_while(|&i| words[i] != ".")
        .find(|&i| at(words, i, &["equal", "to"]))
    else {
        return;
    };
    let mut out: Vec<String> = words[..start].to_vec();
    out.push("x".to_owned());
    out.extend(words[start + 3..equal].iter().cloned());
    out.extend([",", "where", "x", "is"].map(str::to_owned));
    out.extend(words[equal + 2..].iter().cloned());
    *words = out;
}

/// Number words as counts.
const NUMBERS: &[&str] = &[
    "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
];

/// "three {r}" as "{r} {r} {r}".
fn mana_count(words: &mut Vec<String>) {
    let mut out = Vec::with_capacity(words.len());
    let mut i = 0;
    while i < words.len() {
        let count = NUMBERS.iter().position(|n| *n == words[i]).map(|n| n + 1);
        let symbol = words
            .get(i + 1)
            .filter(|w| w.starts_with('{') && w.ends_with('}') && w.len() == 3);
        if let (Some(count), Some(symbol)) = (count, symbol) {
            out.extend(std::iter::repeat_n(symbol.clone(), count));
            i += 2;
        } else {
            out.push(words[i].clone());
            i += 1;
        }
    }
    *words = out;
}

/// Drops "in any order" right after "library".
fn any_order(words: &mut Vec<String>) {
    let mut out: Vec<String> = Vec::with_capacity(words.len());
    let mut i = 0;
    while i < words.len() {
        if out.last().is_some_and(|w| w == "library") && at(words, i, &["in", "any", "order"]) {
            i += 3;
        } else {
            out.push(words[i].clone());
            i += 1;
        }
    }
    *words = out;
}

/// The words a renderer may write twice in a row by a slip.
const DOUBLED: &[&str] = &["then", "and", "the", "a"];

/// Drops commas, periods and "also", reads "an" as "a" and a doubled [`DOUBLED`] word once.
fn punctuation(words: &mut Vec<String>) {
    let mut out: Vec<String> = Vec::with_capacity(words.len());
    for word in words.iter() {
        if word == "," || word == "." || word == "also" {
            continue;
        }
        let word = if word == "an" { "a" } else { word.as_str() };
        if out.last().is_some_and(|w| w == word) && DOUBLED.contains(&word) {
            continue;
        }
        out.push(word.to_owned());
    }
    *words = out;
}
