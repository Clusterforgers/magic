//! Wordings of w16 that add no rule: ability words, the article "an", a creature type followed by "creature", the
//! anaphoric "the creature's", and a subject or "Then" that opens a sentence the other side leaves out.

/// The text with each of the wordings below written one way.
pub(super) fn normal(text: &str) -> String {
    let mut text = text.to_owned();
    for step in [
        ability_word,
        article,
        type_creature,
        anaphoric_possessive,
        opening,
        super::w16e_sequence::normal,
        super::w16c_lists::normal,
    ] {
        if let Some(changed) = step(&text) {
            text = changed;
        }
    }
    text
}

/// The ability words: a label in front of an ability that has no rules of its own.
const ABILITY_WORDS: [&str; 34] = [
    "adamant",
    "landfall",
    "threshold",
    "delirium",
    "chroma",
    "constellation",
    "enrage",
    "metalcraft",
    "morbid",
    "ferocious",
    "hellbent",
    "revolt",
    "raid",
    "formidable",
    "spell mastery",
    "coven",
    "alliance",
    "magecraft",
    "heroic",
    "inspired",
    "corrupted",
    "domain",
    "fateful hour",
    "grandeur",
    "kinship",
    "lieutenant",
    "radiance",
    "rally",
    "undergrowth",
    "battalion",
    "cohort",
    "descend 4",
    "eerie",
    "valiant",
];

/// The word with its first letter in upper case.
fn capital(word: &str) -> String {
    let mut chars = word.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// "Landfall \u{2014} Whenever a land enters, ..." as "Whenever a land enters, ...": an ability word says nothing.
fn ability_word(text: &str) -> Option<String> {
    let (label, rest) = text.split_once(" \u{2014} ")?;
    ABILITY_WORDS
        .contains(&label.to_ascii_lowercase().as_str())
        .then(|| capital(rest))
}

/// "an" as "a": the article only follows the sound of the next word.
fn article(text: &str) -> Option<String> {
    let words: Vec<&str> = text
        .split(' ')
        .map(|word| match word {
            "an" => "a",
            "An" => "A",
            _ => word,
        })
        .collect();
    let changed = words.join(" ");
    (changed != text).then_some(changed)
}

/// The words that begin a noun phrase a creature type can follow.
const DETERMINERS: [&str; 11] = [
    "have",
    "more",
    "each",
    "all",
    "a",
    "another",
    "other",
    "the",
    "any",
    "target",
    "attacking",
];

/// Capitalized words that open a sentence and are not a creature type: "Those creatures", "Artifact creatures".
const NOT_TYPES: &[&str] = &[
    "then",
    "also",
    "if",
    "when",
    "whenever",
    "at",
    "until",
    "during",
    "as",
    "unless",
    "otherwise",
    "instead",
    "for",
    "you",
    "up",
    "to",
    "and",
    "or",
    "this",
    "that",
    "the",
    "an",
    "any",
    "no",
    "those",
    "these",
    "other",
    "enchanted",
    "equipped",
    "target",
    "each",
    "all",
    "your",
    "another",
    "attacking",
    "blocking",
    "tapped",
    "untapped",
    "nontoken",
    "legendary",
    "artifact",
    "enchantment",
    "land",
    "white",
    "blue",
    "black",
    "red",
    "green",
    "colorless",
    "snow",
    "basic",
    "multicolored",
    "chosen",
    "token",
];

/// The characters that end a word.
pub(super) const ENDS: [char; 5] = ['.', ',', ';', ':', '"'];

/// "each Ally creature you control" as "each Ally you control": a creature type is a creature.
fn type_creature(text: &str) -> Option<String> {
    let words: Vec<&str> = text.split(' ').collect();
    let mut out: Vec<String> = Vec::with_capacity(words.len());
    for (at, word) in words.iter().enumerate() {
        let core = word.trim_end_matches(ENDS);
        let plural = core == "creatures";
        let creature = (core == "creature" || plural)
            && at >= 1
            && words[at - 1].starts_with(|c: char| c.is_uppercase())
            && !words[at - 1].ends_with(ENDS)
            && (((at == 1 || words[at - 2].ends_with(['.', ':']))
                && !NOT_TYPES.contains(&words[at - 1].to_ascii_lowercase().as_str()))
                || (at >= 2 && DETERMINERS.contains(&words[at - 2].to_ascii_lowercase().as_str())))
            && !matches!(
                words.get(at + 1).map(|next| next.trim_end_matches(ENDS)),
                Some("card" | "cards" | "spell" | "spells" | "token" | "tokens")
            );
        if creature {
            // What follows the dropped noun stays on the type.
            let tail = &word[core.len()..];
            if let Some(last) = out.last_mut() {
                // "Nightmare creatures" is the plural "Nightmares".
                if plural {
                    if last.ends_with('y') {
                        last.pop();
                        last.push_str("ie");
                    }
                    last.push('s');
                }
                last.push_str(tail);
            }
        } else {
            out.push((*word).to_owned());
        }
    }
    let changed = out.join(" ");
    (changed != text).then_some(changed)
}

/// "the creature's controller" as "that creature's controller": the owner of a possessive is the one named before.
fn anaphoric_possessive(text: &str) -> Option<String> {
    let words: Vec<&str> = text.split(' ').collect();
    let mut changed = false;
    let out: Vec<&str> = words
        .iter()
        .enumerate()
        .map(|(at, word)| match words.get(at + 1) {
            Some(next)
                if *word == "the" && next.ends_with("'s") && !next.starts_with("opponent") =>
            {
                changed = true;
                "that"
            }
            _ => word,
        })
        .collect();
    changed.then(|| out.join(" "))
}

/// The verbs a sentence can open with when its subject is left out: "Then draw two cards." for "You draw two cards."
pub(super) const VERBS: &[&str] = &[
    "take",
    "recruit",
    "amass",
    "explore",
    "populate",
    "venture",
    "learn",
    "discover",
    "add",
    "remove",
    "copy",
    "attach",
    "investigate",
    "proliferate",
    "surveil",
    "double",
    "goad",
    "transform",
    "gain",
    "draw",
    "sacrifice",
    "discard",
    "lose",
    "create",
    "put",
    "return",
    "exile",
    "destroy",
    "search",
    "scry",
    "mill",
    "tap",
    "untap",
    "reveal",
    "look",
    "pay",
    "shuffle",
    "gains",
    "draws",
    "loses",
];

/// The text in pieces that each end with ". " or ", " (the last one ends the text).
fn pieces(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(at) = [". ", ", "].iter().filter_map(|mark| rest.find(mark)).min() {
        let (head, tail) = rest.split_at(at + 2);
        out.push(head);
        rest = tail;
    }
    out.push(rest);
    out
}

/// "Each player loses 2 life. You draw two cards." as "... Then draw two cards.", and ", you gain control" as ", gain
/// control": the sentence is an order to its controller, and "Then" only orders the sentences.
fn opening(original: &str) -> Option<String> {
    let text = original.replace(", then ", ". ");
    let mut out = String::with_capacity(text.len());
    let mut sentence = true;
    for piece in pieces(&text) {
        let mut seg = piece;
        let mut edited = false;
        if sentence && let Some(after) = seg.strip_prefix("Then ") {
            seg = after;
            edited = true;
        }
        let you = if sentence { "You " } else { "you " };
        if let Some(after) = seg.strip_prefix(you)
            && let Some(verb) = after.split(' ').next()
            && VERBS.contains(&verb.trim_end_matches(ENDS))
        {
            seg = after;
            edited = true;
        }
        if edited && sentence {
            out.push_str(&capital(seg));
        } else {
            out.push_str(seg);
        }
        sentence = piece.ends_with(". ");
    }
    (out != original).then_some(out)
}
