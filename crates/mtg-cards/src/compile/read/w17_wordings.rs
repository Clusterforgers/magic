//! General wordings of w17 (third win-back of `Differs` clauses): a verb's number ("has" and "have"), the subject "you"
//! an effect leaves out after a trigger, "each X" and "all Xs", a color said before or after its noun, and a pump that
//! adds to a stat. Both sides of a comparison are normalized, so a rule of another meaning still differs.

use super::w16b_labels::ENDS;

/// The text with each of the wordings below written one way.
pub(super) fn normal(text: &str) -> String {
    let mut text = text.to_owned();
    for step in [
        bare_you,
        verb_number,
        each_all,
        target_colors,
        additional_stat,
    ] {
        if let Some(changed) = step(&text) {
            text = changed;
        }
    }
    super::w18_forms::normal(&super::w17_forms::normal(&text))
}

/// The text of the words when it differs from the original.
fn joined(words: &[String], text: &str) -> Option<String> {
    let changed = words.join(" ");
    (changed != text).then_some(changed)
}

/// The word without the characters that end it, and those characters.
fn split_end(word: &str) -> (&str, &str) {
    let core = word.trim_end_matches(ENDS);
    (core, &word[core.len()..])
}

/// The verbs of an effect that "you" may open: the subject of an effect after a trigger is the controller.
const EFFECTS: [&str; 14] = [
    "create",
    "put",
    "draw",
    "investigate",
    "proliferate",
    "gain",
    "lose",
    "mill",
    "scry",
    "sacrifice",
    "exile",
    "return",
    "destroy",
    "tap",
];

/// Whether the words, up to the end of the sentence they are in, open with a trigger or a condition.
fn opens_condition(sentence: &[&str]) -> bool {
    sentence.first().is_some_and(|first| {
        matches!(
            first.to_ascii_lowercase().as_str(),
            "when" | "whenever" | "at" | "if"
        )
    })
}

/// ", you create" as ", create", and "Put a counter on ~ and you create" as "... and create": the controller is the
/// subject of an effect that follows a trigger's comma or an action of an order.
fn bare_you(text: &str) -> Option<String> {
    let words: Vec<&str> = text.split(' ').collect();
    let mut out: Vec<String> = Vec::with_capacity(words.len());
    let mut sentence = 0;
    for (at, word) in words.iter().enumerate() {
        if at > 0 && words[at - 1].ends_with('.') {
            sentence = at;
        }
        let next = words.get(at + 1).map(|next| split_end(next).0);
        let after_comma = at > 0 && words[at - 1].ends_with(',');
        let after_and = at > 0 && words[at - 1] == "and";
        let order = words
            .get(sentence)
            .is_some_and(|first| EFFECTS.contains(&first.to_ascii_lowercase().as_str()));
        let condition = opens_condition(&words[sentence..]);
        // "~ and you create" is two actions of an order: the other side writes them as two sentences.
        let split = *word == "and"
            && order
            && words.get(at + 1) == Some(&"you")
            && words
                .get(at + 2)
                .is_some_and(|verb| EFFECTS.contains(&split_end(verb).0));
        if split {
            if let Some(last) = out.last_mut() {
                last.push('.');
            }
            continue;
        }
        let drop = *word == "you"
            && next.is_some_and(|next| EFFECTS.contains(&next))
            && ((after_comma && condition) || (after_and && order));
        if !drop {
            out.push((*word).to_owned());
        }
    }
    joined(&out, text)
}

/// The verbs whose form follows the number of the subject, as the form of one subject.
const NUMBER: [(&str, &str); 17] = [
    ("put", "puts"),
    ("shuffle", "shuffles"),
    ("search", "searches"),
    ("have", "has"),
    ("deal", "deals"),
    ("become", "becomes"),
    ("get", "gets"),
    ("gain", "gains"),
    ("lose", "loses"),
    ("phase", "phases"),
    ("are", "is"),
    ("enter", "enters"),
    ("attack", "attacks"),
    ("block", "blocks"),
    ("die", "dies"),
    ("don't", "doesn't"),
    ("were", "was"),
];

/// "creatures have" as "creatures has": a verb's number says nothing the subject does not.
fn verb_number(text: &str) -> Option<String> {
    let source: Vec<&str> = text.split(' ').collect();
    // The verbs of an order ("put", "shuffle", "search") take a number only after a subject that is several players.
    let groups = {
        let lower = text.to_ascii_lowercase();
        lower.contains("all players") || lower.contains("each player")
    };
    let words: Vec<String> = source
        .iter()
        .enumerate()
        .map(|(at, word)| {
            let (core, tail) = split_end(word);
            // "you may have it deal" and "to become" are infinitives: they name no subject.
            let infinitive = at > 0
                && matches!(
                    source[at - 1].to_ascii_lowercase().as_str(),
                    "may" | "to" | "can" | "can't" | "must" | "will"
                );
            // "phase out" is a verb; "main phase" is a noun.
            let noun = core == "phase" && source.get(at + 1) != Some(&"out");
            let lower = core.to_ascii_lowercase();
            let capital = core.starts_with(|c: char| c.is_ascii_uppercase());
            match NUMBER.iter().find(|(plural, _)| {
                *plural == lower
                    && (groups || !matches!(lower.as_str(), "put" | "shuffle" | "search"))
            }) {
                Some(_) if noun => (*word).to_owned(),
                // Only a verb no action of an order repeats is changed at the start of a sentence.
                Some((_, single)) if !infinitive && capital => {
                    if matches!(lower.as_str(), "shuffle" | "search") {
                        format!("{}{tail}", capitalized(single))
                    } else {
                        (*word).to_owned()
                    }
                }
                Some((_, single)) if !infinitive => format!("{single}{tail}"),
                _ => (*word).to_owned(),
            }
        })
        .collect();
    joined(&words, text)
}

/// The word with its first letter in upper case.
fn capitalized(word: &str) -> String {
    let mut chars = word.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_ascii_uppercase().to_string() + chars.as_str()
    })
}

/// The nouns "each" and "all" name.
const KINDS: [&str; 10] = [
    "creature",
    "permanent",
    "land",
    "artifact",
    "enchantment",
    "token",
    "planeswalker",
    "player",
    "opponent",
    "card",
];

/// The words that may stand between "each" and its noun.
const MODIFIERS: [&str; 19] = [
    "snow",
    "noncreature",
    "legendary",
    "multicolored",
    "colorless",
    "nongreen",
    "nonwhite",
    "nonblack",
    "nonred",
    "nonblue",
    "nonland",
    "nontoken",
    "nonartifact",
    "nonbasic",
    "other",
    "tapped",
    "untapped",
    "attacking",
    "blocking",
];

/// "each nonland permanent" as "all nonland permanents".
fn each_all(text: &str) -> Option<String> {
    let words: Vec<&str> = text.split(' ').collect();
    let mut out: Vec<String> = Vec::with_capacity(words.len());
    let mut at = 0;
    while let Some(word) = words.get(at) {
        let mut end = at + 1;
        while words.get(end).is_some_and(|next| {
            MODIFIERS.contains(&next.to_ascii_lowercase().as_str()) || next.starts_with("non-")
        }) {
            end += 1;
        }
        // "for each creature" counts, "for all creatures" does not.
        let counted = at > 0 && words[at - 1].eq_ignore_ascii_case("for");
        let each = word.eq_ignore_ascii_case("each") && !counted;
        let all = word.eq_ignore_ascii_case("all");
        let noun = words.get(end).map(|noun| split_end(noun));
        // "each creature" names the kind by its singular, "all creatures" by its plural.
        let plural = noun.and_then(|(kind, tail)| match (each, all) {
            (true, _) if KINDS.contains(&kind) => Some(format!("{kind}s{tail}")),
            (_, true) => kind
                .strip_suffix('s')
                .filter(|single| KINDS.contains(single))
                .map(|_| format!("{kind}{tail}")),
            _ => None,
        });
        let Some(plural) = plural else {
            out.push((*word).to_owned());
            at += 1;
            continue;
        };
        // "Creatures you control" and "All creatures you control" are the same subject.
        let controlled = words.get(end + 1) == Some(&"you")
            && words
                .get(end + 2)
                .is_some_and(|next| next.starts_with("control"));
        let mut phrase: Vec<String> = words[at + 1..end]
            .iter()
            .map(|word| (*word).to_owned())
            .collect();
        phrase.push(plural);
        let capital = word.starts_with(char::is_uppercase);
        if controlled {
            if capital {
                phrase[0] = capital_of(&phrase[0]);
            }
        } else {
            out.push(if capital { "All" } else { "all" }.to_owned());
        }
        out.extend(phrase);
        at = end + 1;
    }
    joined(&out, text)
}

/// The text with its first letter in upper case.
pub(super) fn capital_of(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// The colors a card may have.
const COLORS: [&str; 6] = ["white", "blue", "black", "red", "green", "colorless"];

/// The nouns a color can describe.
const OBJECTS: [&str; 5] = ["spell", "permanent", "creature", "card", "artifact"];

/// "target red or green spell" as "target spell that's red or green".
fn target_colors(text: &str) -> Option<String> {
    let words: Vec<&str> = text.split(' ').collect();
    let mut out: Vec<String> = Vec::with_capacity(words.len());
    let mut at = 0;
    while let Some(word) = words.get(at) {
        out.push((*word).to_owned());
        at += 1;
        if !word.eq_ignore_ascii_case("target") {
            continue;
        }
        let mut colors: Vec<&str> = Vec::new();
        let mut end = at;
        while let Some(next) = words.get(end) {
            let core = split_end(next).0;
            if COLORS.contains(&core) || (core == "or" && !colors.is_empty()) {
                colors.push(core);
                end += 1;
            } else {
                break;
            }
        }
        let noun = words.get(end).map(|noun| split_end(noun));
        match noun {
            Some((kind, tail))
                if colors.last().is_some_and(|last| *last != "or") && OBJECTS.contains(&kind) =>
            {
                out.push(kind.to_owned());
                let tail_word = format!("that's {}{tail}", colors.join(" "));
                out.push(tail_word);
                at = end + 1;
            }
            _ => {}
        }
    }
    joined(&out, text)
}

/// "gets an additional +1/+0" as "gets +1/+0", and "add an additional {G}" as "add {G}": a pump adds to the stats
/// anyway.
fn additional_stat(text: &str) -> Option<String> {
    let words: Vec<&str> = text.split(' ').collect();
    let mut out: Vec<&str> = Vec::with_capacity(words.len());
    for (at, word) in words.iter().enumerate() {
        let stat = words
            .get(at + 1)
            .is_some_and(|next| next.starts_with(['+', '-', '{']));
        let article = at > 0 && matches!(words[at - 1], "a" | "an");
        if *word == "additional" && article && stat {
            out.pop();
        } else {
            out.push(word);
        }
    }
    let out: Vec<String> = out.into_iter().map(str::to_owned).collect();
    joined(&out, text)
}
