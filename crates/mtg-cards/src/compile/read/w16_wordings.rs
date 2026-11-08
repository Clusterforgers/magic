//! General wordings of one meaning that the oracle text and the writers say differently (w16 win-back of `Differs`
//! clauses): the card's gendered pronoun, "dies" and a duration said once in a one-sentence clause here; ability words,
//! articles and creature types (`w16b_labels`), unordered lists (`w16c_lists`), small forms (`w16d_forms`) and joined
//! actions (`w16e_sequence`) in the modules they name. Both sides of a comparison are normalized, so a rule of another
//! meaning still differs.

/// The text with each of the wordings below written one way.
pub(super) fn normal(text: &str) -> String {
    // Lines of a clause join as one text: a line break is a space.
    let mut text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    for step in [
        super::w19r_stutter::once,
        super::w18_forms::early,
        pronouns,
        dies,
        one_duration,
        super::w17_forms::before_lists,
    ] {
        if let Some(changed) = step(&text) {
            text = changed;
        }
    }
    super::w17_wordings::normal(&super::w16d_forms::normal(&super::w16b_labels::normal(
        &text,
    )))
}

/// The words that end a clause where "her" is an object ("exile her.").
const OBJECT_END: [&str; 12] = [
    "to", "into", "on", "from", "and", "or", "then", "if", "unless", "at", "until", "with",
];

/// The text with the characters that end a word split off it: the word and what follows it.
fn split_end(word: &str) -> (&str, &str) {
    let core = word.trim_end_matches(['.', ',', ';', ':', '"']);
    (core, &word[core.len()..])
}

/// "she", "he", "him", "her" and "his" as "it" or "its": a card is "it" in the oracle text, and a gendered card is "she"
/// in what the writers say.
fn pronouns(text: &str) -> Option<String> {
    let words: Vec<&str> = text.split(' ').collect();
    let mut out: Vec<String> = Vec::with_capacity(words.len());
    for (at, word) in words.iter().enumerate() {
        let (core, tail) = split_end(word);
        let lower = core.to_ascii_lowercase();
        let next = words.get(at + 1).map(|next| split_end(next).0);
        let same = match lower.as_str() {
            "she" | "he" | "him" => "it".to_owned(),
            "his" => "its".to_owned(),
            "her" => {
                let object = !tail.is_empty()
                    || next.is_none_or(|next| {
                        OBJECT_END.contains(&next.to_ascii_lowercase().as_str())
                    });
                if object { "it" } else { "its" }.to_owned()
            }
            _ => core.to_owned(),
        };
        let same = if core.starts_with(|c: char| c.is_uppercase()) {
            let mut chars = same.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(chars).collect()
            })
        } else {
            same
        };
        out.push(format!("{same}{tail}"));
    }
    let changed = out.join(" ");
    (changed != text).then_some(changed)
}

/// "is put into a graveyard from the battlefield" as "dies".
fn dies(text: &str) -> Option<String> {
    const LONG: &str = " is put into a graveyard from the battlefield";
    text.contains(LONG).then(|| text.replace(LONG, " dies"))
}

/// A one-sentence clause that says "until end of turn" once says it anywhere: it is dropped and one mark ends the text.
fn one_duration(text: &str) -> Option<String> {
    const TURN: &str = "until end of turn";
    let body = text.trim_end_matches('.');
    let lower = body.to_ascii_lowercase();
    if lower.matches(TURN).count() != 1 || body.contains(". ") || body.contains('"') {
        return None;
    }
    let at = lower.find(TURN)?;
    let (before, after) = (&body[..at], &body[at + TURN.len()..]);
    let before = before.trim_end_matches([' ', ',']);
    let after = after.trim_start_matches([' ', ',']);
    let rest = match (before.is_empty(), after.is_empty()) {
        (true, _) => after.to_owned(),
        (_, true) => before.to_owned(),
        _ => format!("{before} {after}"),
    };
    let mut chars = rest.chars();
    let rest: String = chars.next().map_or_else(String::new, |first| {
        first.to_lowercase().chain(chars).collect()
    });
    Some(format!("{rest} [until end of turn]"))
}
