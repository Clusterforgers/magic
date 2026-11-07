//! More Oracle wordings of a rule the writers say another way: card types after "each" joined by "and", "an" before a
//! vowel, "then" between two things one player does, an implied "you" after "and", "that has" for "with" a counter, and
//! "it" for the card a flicker returns.

/// The card types a list after "each" may name.
const TYPES: [&str; 6] = [
    "artifact",
    "creature",
    "enchantment",
    "land",
    "planeswalker",
    "battle",
];

/// "Destroy each artifact, creature, or enchantment" said as "each artifact, creature, and enchantment": a list of card
/// types after "each" names each permanent of any of them, whichever word joins it.
pub(super) fn each_and(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (at, _) in text.match_indices("each ") {
        if at > 0 && !text[..at].ends_with(' ') {
            continue;
        }
        let rest = &text[at + 5..];
        let mut length = 0;
        let mut joined = None;
        let mut last = false;
        for word in rest.split(' ') {
            let bare = word.trim_end_matches(',');
            if TYPES.contains(&bare) {
                if joined.is_some() {
                    last = true;
                    break;
                }
            } else if bare == "or" && joined.is_none() && length > 0 {
                joined = Some(length);
            } else {
                break;
            }
            length += word.len() + 1;
        }
        if let Some(or) = joined
            && last
        {
            let start = at + 5 + or;
            out.push(format!("{}and{}", &text[..start], &text[start + 2..]));
        }
    }
    out
}

/// The words after "a" that open with a vowel sound written with a consonant letter or the other way: none of them
/// takes "an".
const NOT_AN: [&str; 4] = ["one", "uni", "use", "eu"];

/// "Sacrifice a artifact" said as "Sacrifice an artifact": the article before a vowel.
pub(super) fn an_vowel(text: &str) -> Vec<String> {
    let mut out = String::with_capacity(text.len() + 4);
    let mut rest = text;
    while let Some(at) = rest.find(" a ") {
        let after = &rest[at + 3..];
        let vowel = after.starts_with(['a', 'e', 'i', 'o', 'A', 'E', 'I', 'O'])
            && !NOT_AN
                .iter()
                .any(|w| after.to_ascii_lowercase().starts_with(w));
        out.push_str(&rest[..at]);
        out.push_str(if vowel { " an " } else { " a " });
        rest = after;
    }
    out.push_str(rest);
    if out == text { Vec::new() } else { vec![out] }
}

/// "draws a card and discards a card" said as "draws a card, then discards a card": a player's two actions happen in
/// order either way.
pub(super) fn then_discards(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (and, then) in [
        (" and discards ", ", then discards "),
        (" and discard ", ", then discard "),
    ] {
        if text.contains(and) {
            out.push(text.replacen(and, then, 1));
        }
    }
    out
}

/// The verbs an instruction with no subject opens with: it is done by "you".
const IMPERATIVES: [&str; 14] = [
    "return",
    "draw",
    "create",
    "put",
    "exile",
    "destroy",
    "sacrifice",
    "scry",
    "surveil",
    "look",
    "search",
    "tap",
    "untap",
    "mill",
];

/// "you gain 3 life and you get {E}{E}{E}" said as "you gain 3 life and get {E}{E}{E}", and "Return ~ to its owner's hand
/// and you return target card" as "... and return target card": after "and", "you" is implied when the sentence is done
/// by you. Inside a quotation, an instruction after the trigger is done by its controller ("At the beginning of your
/// upkeep, sacrifice a creature").
pub(super) fn and_you(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    // "the sacrificed creature" names a sacrifice of the cost; without one the wording is not this rule's.
    let unpaid = text.contains(" sacrificed ")
        && !text
            .split(": ")
            .next()
            .is_some_and(|cost| cost.contains("Sacrifice"));
    if unpaid {
        return out;
    }
    for (at, _) in text.match_indices(" and you ") {
        let start = [". ", ": ", ", ", "\""]
            .iter()
            .filter_map(|mark| text[..at].rfind(mark).map(|found| found + mark.len()))
            .max()
            .unwrap_or(0);
        let first = text[start..at]
            .split(' ')
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        if first == "you" || IMPERATIVES.contains(&first.as_str()) {
            out.push(format!("{} and {}", &text[..at], &text[at + 9..]));
        }
    }
    if let Some(open) = text.find("\"At the beginning of your ")
        && let Some(comma) = text[open..].find(", you ")
    {
        let at = open + comma;
        let verb = text[at + 6..].split(' ').next().unwrap_or_default();
        if IMPERATIVES.contains(&verb) && !text[open + 1..at].contains(['"', ',']) {
            out.push(format!("{}, {}", &text[..at], &text[at + 6..]));
        }
    }
    out
}

/// "target creature with a -1/-1 counter on it" said as "target creature that has a -1/-1 counter on it".
pub(super) fn that_has(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (at, _) in text.match_indices(" with a ") {
        let rest = &text[at + 8..];
        let Some(end) = rest.find(" counter on it") else {
            continue;
        };
        if rest[..end].contains(' ') {
            continue;
        }
        out.push(format!("{} that has a {rest}", &text[..at]));
    }
    out
}

/// "Exile target creature you control, then return that card to the battlefield" said as "... then return it to the
/// battlefield": the card exiled is the one returned (CR 400.7 makes it a new object, which "it" names as well).
pub(super) fn flicker_it(text: &str) -> Vec<String> {
    const THAT: &str = ", then return that card to the battlefield";
    let mut out = Vec::new();
    for (at, _) in text.match_indices(THAT) {
        let start = [". ", ": ", ", ", "\"", "\u{2022} "]
            .iter()
            .filter_map(|mark| text[..at].rfind(mark).map(|found| found + mark.len()))
            .max()
            .unwrap_or(0);
        let head = &text[start..at];
        let one = [
            "exile target ",
            "Exile target ",
            "exile up to one target ",
            "Exile up to one target ",
        ]
        .iter()
        .any(|lead| head.starts_with(lead));
        if one && !head.contains(" or ") && !head.contains("target creatures") {
            out.push(format!(
                "{}, then return it to the battlefield{}",
                &text[..at],
                &text[at + THAT.len()..]
            ));
        }
    }
    out
}

/// "Up to X target creatures get +1/+1 ..., where X is the number of creatures on the battlefield" said as "Any number of
/// target creatures each get +1/+1 ...": no more creatures can be targeted than there are.
pub(super) fn any_number(text: &str) -> Vec<String> {
    const WHERE: &str = ", where X is the number of creatures on the battlefield";
    let Some(at) = text.find(WHERE) else {
        return Vec::new();
    };
    let without = format!("{}{}", &text[..at], &text[at + WHERE.len()..]);
    if without.matches('X').count() != 1 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (up, any) in [
        (
            "Up to X target creatures ",
            "Any number of target creatures ",
        ),
        (
            "up to X target creatures ",
            "any number of target creatures ",
        ),
    ] {
        if without.contains(up) {
            out.push(without.replacen(up, any, 1));
            out.push(without.replacen(up, &format!("{any}each "), 1));
        }
    }
    out
}
