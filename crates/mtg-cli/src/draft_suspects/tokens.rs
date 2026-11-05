//! The meaning-bearing tokens of a rules text and the comparison of two texts by token counts.

use std::collections::BTreeMap;

/// Token counts by token name.
pub type Counts = BTreeMap<String, i32>;

/// Phrases of more than one word that carry meaning, matched on the word sequence.
const PHRASES: &[&str] = &[
    "up to",
    "any number",
    "you own",
    "you control",
    "an opponent",
    "that player",
    "their owner",
    "its owner",
    "its controller",
];

/// Single words that carry meaning, by the token they count as (inflections share a token).
const WORDS: &[(&str, &str)] = &[
    ("each", "each"),
    ("all", "all"),
    ("another", "another"),
    ("other", "other"),
    ("others", "other"),
    ("target", "target"),
    ("targets", "target"),
    ("may", "may"),
    ("unless", "unless"),
    ("if", "if"),
    ("instead", "instead"),
    ("until", "until"),
    ("tapped", "tapped"),
    ("attacking", "attacking"),
    ("blocking", "blocking"),
    ("graveyard", "graveyard"),
    ("graveyards", "graveyard"),
    ("exile", "exile"),
    ("exiles", "exile"),
    ("exiled", "exile"),
    ("hand", "hand"),
    ("hands", "hand"),
    ("library", "library"),
    ("libraries", "library"),
    ("top", "top"),
    ("bottom", "bottom"),
    ("battlefield", "battlefield"),
    ("sacrifice", "sacrifice"),
    ("sacrifices", "sacrifice"),
    ("sacrificed", "sacrifice"),
    ("destroy", "destroy"),
    ("destroys", "destroy"),
    ("destroyed", "destroy"),
    ("return", "return"),
    ("returns", "return"),
    ("returned", "return"),
    ("draw", "draw"),
    ("draws", "draw"),
    ("discard", "discard"),
    ("discards", "discard"),
    ("discarded", "discard"),
    ("counter", "counter"),
    ("counters", "counter"),
    ("countered", "counter"),
    ("shuffle", "shuffle"),
    ("shuffles", "shuffle"),
    ("reveal", "reveal"),
    ("reveals", "reveal"),
    ("revealed", "reveal"),
    ("x", "x"),
];

/// Number words and their values.
const NUMBER_WORDS: &[&str] = &[
    "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
];

/// What the renderer legitimately words differently from the Oracle text, as (rendered wording, Oracle wording).
pub const SYNONYMS: &[(&str, &str)] = &[
    ("is put into a graveyard from the battlefield", "dies"),
    ("enters the battlefield", "enters"),
];

/// The text without reminder text in parentheses, lowercased, with each name replaced by `~` and the synonyms applied.
pub fn normalise(text: &str, names: &[String]) -> String {
    let mut plain = String::new();
    let mut depth = 0usize;
    for c in text.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if depth == 0 => plain.push(c),
            _ => {}
        }
    }
    let mut plain = plain.to_lowercase().replace(['\u{2019}', '\u{2018}'], "'");
    let mut sorted: Vec<String> = names.iter().map(|name| name.to_lowercase()).collect();
    sorted.sort_by_key(|name| std::cmp::Reverse(name.len()));
    for name in sorted.iter().filter(|name| !name.is_empty()) {
        plain = plain.replace(name.as_str(), "~");
    }
    for (rendered, oracle) in SYNONYMS {
        plain = plain.replace(rendered, oracle);
    }
    plain
}

/// The `{..}` symbols of a text, each as `{sym}`, and the text without them.
fn take_symbols(text: &str, counts: &mut Counts) -> String {
    let mut rest = String::new();
    let mut open: Option<String> = None;
    for c in text.chars() {
        match (&mut open, c) {
            (None, '{') => open = Some(String::from("{")),
            (Some(symbol), '}') => {
                symbol.push('}');
                *counts.entry(format!("mana:{symbol}")).or_insert(0) += 1;
                open = None;
                rest.push(' ');
            }
            (Some(symbol), c) => symbol.push(c),
            (None, c) => rest.push(c),
        }
    }
    rest
}

/// A power/toughness modifier such as `+2/+2` or `-1/-1`, when the word is one.
fn is_pt(word: &str) -> bool {
    let Some((power, toughness)) = word.split_once('/') else {
        return false;
    };
    let number = |part: &str| {
        let digits = part.trim_start_matches(['+', '-']);
        !digits.is_empty()
            && part.len() - digits.len() <= 1
            && digits.chars().all(|c| c.is_ascii_digit() || c == 'x')
    };
    number(power) && number(toughness)
}

/// The words of a text: letters, digits and the signs of a modifier, with the edge punctuation cut off.
fn words(text: &str) -> Vec<String> {
    text.split(|c: char| c.is_whitespace() || matches!(c, ',' | '.' | ';' | ':' | '"' | '—'))
        .map(|word| {
            word.trim_matches(|c: char| !c.is_alphanumeric() && !matches!(c, '+' | '-' | '~' | '/'))
        })
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
        .collect()
}

fn bump(counts: &mut Counts, token: String) {
    *counts.entry(token).or_insert(0) += 1;
}

/// Counts the tokens of the words, advancing past phrases and keywords.
fn count_words(words: &[String], keywords: &[String], counts: &mut Counts) {
    let mut at = 0;
    while at < words.len() {
        let word = words[at].as_str();
        let matches_at = |phrase: &str| {
            let parts: Vec<&str> = phrase.split(' ').collect();
            words.len() >= at + parts.len()
                && parts
                    .iter()
                    .zip(&words[at..])
                    .all(|(a, b)| *a == b.as_str())
        };
        if let Some(keyword) = keywords.iter().find(|keyword| matches_at(keyword)) {
            bump(counts, format!("keyword:{keyword}"));
            at += keyword.split(' ').count();
        } else if let Some(phrase) = PHRASES.iter().find(|phrase| matches_at(phrase)) {
            bump(counts, (*phrase).to_owned());
            at += phrase.split(' ').count();
        } else {
            count_word(word, counts);
            at += 1;
        }
    }
}

fn count_word(word: &str, counts: &mut Counts) {
    if is_pt(word) {
        bump(counts, format!("pt:{word}"));
    } else if let Some(position) = NUMBER_WORDS.iter().position(|number| *number == word) {
        bump(counts, format!("number:{position}"));
    } else if !word.is_empty() && word.chars().all(|c| c.is_ascii_digit()) {
        bump(
            counts,
            format!(
                "number:{}",
                word.trim_start_matches('0').parse::<u32>().unwrap_or(0)
            ),
        );
    } else if word.starts_with("non-") || (word.starts_with("non") && word.len() >= 6) {
        bump(counts, "non-".to_owned());
    } else if let Some((_, token)) = WORDS.iter().find(|(spelled, _)| *spelled == word) {
        bump(counts, (*token).to_owned());
    }
}

/// The token counts of a normalised text, with the card's keywords (lowercase) counted as tokens of their own.
pub fn count(text: &str, keywords: &[String]) -> Counts {
    let mut counts = Counts::new();
    let rest = take_symbols(text, &mut counts);
    count_words(&words(&rest), keywords, &mut counts);
    counts
}

/// The findings of comparing the Oracle counts with the rendered counts: `missing:` for what the English lacks and
/// `extra:` for what only the English has, once for each token, in token order.
pub fn compare(oracle: &Counts, english: &Counts) -> Vec<String> {
    let mut findings = Vec::new();
    let tokens: std::collections::BTreeSet<&String> = oracle.keys().chain(english.keys()).collect();
    for token in tokens {
        let want = oracle.get(token).copied().unwrap_or(0);
        let have = english.get(token).copied().unwrap_or(0);
        if want > have {
            findings.push(format!("missing:{token}"));
        } else if have > want {
            findings.push(format!("extra:{token}"));
        }
    }
    findings
}

/// The findings for an Oracle text and its rendered English: reminder text ignored, names normalised.
pub fn findings(oracle: &str, english: &str, names: &[String], keywords: &[String]) -> Vec<String> {
    let a = count(&normalise(oracle, names), keywords);
    let b = count(&normalise(english, names), keywords);
    compare(&a, &b)
}

#[cfg(test)]
mod tests {
    //! Goal: the comparer finds a dropped quantifier, a changed number, a lost scope phrase, a wrong mana symbol and a
    //! missing keyword, and ignores reminder text, names and wording that carries the same tokens.
    use rstest::rstest;

    use super::*;

    fn run(oracle: &str, english: &str) -> Vec<String> {
        let names = vec!["Test Card".to_owned()];
        let keywords = vec!["flying".to_owned(), "first strike".to_owned()];
        findings(oracle, english, &names, &keywords)
    }

    #[rstest]
    #[case::clean("Draw two cards.", "Draw two cards.", &[])]
    #[case::number_word_as_digit("Draw two cards.", "Draw 2 cards.", &[])]
    #[case::dropped_up_to("Exile up to two target creatures.", "Exile two target creatures.", &["missing:up to"])]
    #[case::changed_number("Draw three cards.", "Draw two cards.", &["missing:number:3", "extra:number:2"])]
    #[case::lost_you_own("Return target creature you own to your hand.", "Return target creature to your hand.", &["missing:you own"])]
    #[case::extra_word("Return it to your hand.", "Return it to your hand. Draw a card.", &["extra:draw"])]
    #[case::reminder_ignored("Flying (This creature can't be blocked except by creatures with flying.)", "Flying", &[])]
    #[case::name_normalised("Test Card deals 2 damage.", "~ deals 2 damage.", &[])]
    #[case::mana_symbol("{T}: Add {G}.", "{T}: Add {R}.", &["missing:mana:{g}", "extra:mana:{r}"])]
    #[case::pt("Target creature gets +2/+2 until end of turn.", "Target creature gets +2/+1 until end of turn.", &["missing:pt:+2/+2", "extra:pt:+2/+1"])]
    #[case::keyword_lost("Flying, first strike", "Flying", &["missing:keyword:first strike"])]
    fn the_comparer(#[case] oracle: &str, #[case] english: &str, #[case] expected: &[&str]) {
        let mut found = run(oracle, english);
        found.sort();
        let mut expected: Vec<String> = expected.iter().map(|s| (*s).to_owned()).collect();
        expected.sort();
        assert_eq!(found, expected);
    }

    #[rstest]
    #[case("Destroy target nonland permanent.", &["destroy", "non-", "target"])]
    #[case("Each other creature gets +1/+1.", &["each", "other", "pt:+1/+1"])]
    #[case("{2}{W}, sacrifice ~: Draw X cards.", &["mana:{2}", "mana:{w}", "sacrifice", "x", "draw"])]
    fn the_tokeniser(#[case] text: &str, #[case] expected: &[&str]) {
        let counts = count(&normalise(text, &["~".to_owned()]), &[]);
        let mut found: Vec<&str> = counts.keys().map(String::as_str).collect();
        found.sort_unstable();
        let mut expected = expected.to_vec();
        expected.sort_unstable();
        assert_eq!(found, expected);
    }
}
