//! Forms of w16 that mean the same to the rules: "when" and "whenever" open the same triggered ability, a number is a
//! word or a digit, "or greater" is "or more", "other target" is "target other", "also" orders nothing, a label in
//! front of a trigger says nothing, and "that creature" is "it" when the text names one object.

/// The text with each of the forms below written one way.
pub(super) fn normal(text: &str) -> String {
    let mut text = text.to_owned();
    for step in [label, words, anaphora, attached_to_itself, team, tense] {
        if let Some(changed) = step(&text) {
            text = changed;
        }
    }
    text
}

/// The number words and the digits they are.
const NUMBERS: [(&str, &str); 10] = [
    ("one", "1"),
    ("two", "2"),
    ("three", "3"),
    ("four", "4"),
    ("five", "5"),
    ("six", "6"),
    ("seven", "7"),
    ("eight", "8"),
    ("nine", "9"),
    ("ten", "10"),
];

/// The characters that end a word.
const ENDS: [char; 5] = ['.', ',', ';', ':', '"'];

/// "Whenever" as "when", a number word as its digit, "or greater" as "or more", "other target" as "target other", and
/// "also" dropped.
fn words(text: &str) -> Option<String> {
    let words: Vec<&str> = text.split(' ').collect();
    let mut out: Vec<String> = Vec::with_capacity(words.len());
    let mut at = 0;
    while let Some(word) = words.get(at) {
        let quote = &word[..word.len() - word.trim_start_matches('"').len()];
        let word = &word[quote.len()..];
        let core = word.trim_end_matches(ENDS);
        let tail = &word[core.len()..];
        let lower = core.to_ascii_lowercase();
        let after_or = out.last().is_some_and(|last| last == "or");
        let digit = NUMBERS.iter().find(|(name, _)| *name == lower);
        if lower == "whenever" {
            let same = if core.starts_with('W') {
                "When"
            } else {
                "when"
            };
            out.push(format!("{quote}{same}{tail}"));
        } else if lower == "also" && tail.is_empty() {
            // "also" orders nothing.
        } else if let Some((_, digit)) = digit {
            out.push(format!("{quote}{digit}{tail}"));
        } else if after_or && lower == "greater" {
            out.push(format!("{quote}more{tail}"));
        } else if after_or && lower == "fewer" {
            out.push(format!("{quote}less{tail}"));
        } else if lower == "other" && words.get(at + 1) == Some(&"target") {
            out.push("target".to_owned());
            out.push("other".to_owned());
            at += 1;
        } else {
            out.push(format!("{quote}{word}"));
        }
        at += 1;
    }
    let changed = out.join(" ");
    (changed != text).then_some(changed)
}

/// What a label may be followed by: an ability that is a trigger or a static condition.
const AFTER_LABEL: [&str; 9] = [
    "when ",
    "whenever ",
    "at the ",
    "as long as ",
    "if ",
    "you may ",
    "~ ",
    "target ",
    "creatures ",
];

/// "Survival \u{2014} At the beginning of your second main phase, ..." as "At the beginning ...": a label in front of
/// a trigger or a condition says nothing, and a chapter number (I, II) is no label.
fn label(text: &str) -> Option<String> {
    let (label, rest) = text.split_once(" \u{2014} ")?;
    let roman = label.chars().all(|c| {
        matches!(c, 'I' | 'V' | 'X' | ',' | ' ') || c.is_ascii_lowercase() && "ivx".contains(c)
    });
    let lower = rest.to_ascii_lowercase();
    if roman
        || label.split(' ').count() > 4
        || !AFTER_LABEL.iter().any(|start| lower.starts_with(start))
    {
        return None;
    }
    let mut chars = rest.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
}

/// The nouns "that ..." can stand for.
const NOUNS: [&str; 9] = [
    "creature",
    "card",
    "land",
    "permanent",
    "artifact",
    "enchantment",
    "spell",
    "token",
    "planeswalker",
];

/// The words that introduce one object.
const INTRODUCERS: [&str; 6] = ["a", "an", "another", "target", "each", "one"];

/// "Exile that creature." as "Exile it.": the text names one object, so "that creature" and "it" are the same one.
fn anaphora(text: &str) -> Option<String> {
    let words: Vec<&str> = text.split(' ').collect();
    let noun = |word: &str| NOUNS.contains(&word.trim_end_matches(ENDS));
    // The card itself is "~" or "this creature": "it" may be the card.
    let itself = |pair: &[&str]| pair[0].eq_ignore_ascii_case("this") && noun(pair[1]);
    if text.contains('~') || words.windows(2).any(itself) {
        return None;
    }
    // The kinds of object the text names besides the ones "that ..." points back to: one kind means one object.
    let kind = |word: &str| {
        let word = word.trim_end_matches(ENDS).trim_end_matches("'s");
        NOUNS
            .iter()
            .find(|noun| **noun == word || format!("{noun}s") == word)
            .copied()
    };
    let mut kinds: Vec<&str> = words
        .iter()
        .enumerate()
        .filter(|(at, _)| *at == 0 || !words[*at - 1].eq_ignore_ascii_case("that"))
        .filter_map(|(_, word)| kind(word))
        .collect();
    kinds.sort_unstable();
    kinds.dedup();
    if kinds.len() > 1 {
        return None;
    }
    let mut out: Vec<String> = Vec::with_capacity(words.len());
    let mut at = 0;
    while let Some(word) = words.get(at) {
        let that = word.eq_ignore_ascii_case("that");
        match words.get(at + 1) {
            Some(next) if that && noun(next.trim_end_matches("'s")) => {
                let tail = &next[next.trim_end_matches(ENDS).len()..];
                let possessive = next.trim_end_matches(ENDS).ends_with("'s");
                let pronoun = if possessive { "its" } else { "it" };
                let pronoun = if word.starts_with('T') {
                    "It".to_owned()
                } else {
                    pronoun.to_owned()
                };
                let pronoun = if possessive && word.starts_with('T') {
                    "Its".to_owned()
                } else {
                    pronoun
                };
                out.push(format!("{pronoun}{tail}"));
                at += 2;
            }
            _ => {
                out.push((*word).to_owned());
                at += 1;
            }
        }
    }
    let changed = out.join(" ");
    (changed != text).then_some(changed)
}

/// "for each Equipment attached to it" as "... attached to ~": with no other object in the text, "it" is the card.
fn attached_to_itself(text: &str) -> Option<String> {
    const ATTACHED: &str = " attached to it";
    let at = text.find(ATTACHED)?;
    let end = at + ATTACHED.len();
    let ends_word = text[end..]
        .chars()
        .next()
        .is_none_or(|c| !c.is_alphanumeric());
    let words: Vec<&str> = text.split(' ').collect();
    let named = words.windows(2).any(|pair| {
        INTRODUCERS.contains(&pair[0].to_ascii_lowercase().as_str())
            && NOUNS.contains(&pair[1].trim_end_matches(ENDS))
    });
    (ends_word && !named && !text.contains("target"))
        .then(|| format!("{} attached to ~{}", &text[..at], &text[end..]))
}

/// "your team controls" as "you control": a game has no teams, so the team is the player.
fn team(text: &str) -> Option<String> {
    text.contains(" your team controls")
        .then(|| text.replace(" your team controls", " you control"))
}

/// "If its mana value was 3 or less" and "If X was 7 or more" as "is": the value is the same one when the text is read.
fn tense(text: &str) -> Option<String> {
    let changed = text
        .replace(" mana value was ", " mana value is ")
        .replace("If X was ", "If X is ");
    (changed != text).then_some(changed)
}
