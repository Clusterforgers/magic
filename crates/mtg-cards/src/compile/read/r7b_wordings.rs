//! Oracle wordings of a rule the writers say another way, each derived from a written wording of the same rule: two mana
//! of two colors as the three pairs ("{U}{U}, {U}{R}, or {R}{R}"), a quoted ability closing its sentence with one
//! period, creatures of a subtype that get a bonus or a combat keyword as "Dinosaurs you control", the card named again
//! as "it", a creature named just before as "it" or "that creature", one "until end of turn" for two effects, "Until end
//! of turn," before a granted ability, two effects of one subject until end of turn joined in one sentence, and the
//! controller's choice between keywords or bonuses as "your choice of".

use super::subtypes::{creature_types, plural_subtype};

/// The most wordings the rows add to the wordings of one ability: the rows chain, so a long text could multiply them.
const LIMIT: usize = 64;

/// The most wordings an ability may have for the rows to add theirs: a part of a joined static multiplies the wordings
/// of the whole by its own.
const BEFORE: usize = 4096;

/// Adds the Oracle wordings of each row to the wordings of an ability.
pub(super) fn add(ability: &crate::ability::Ability, texts: &mut Vec<String>) {
    if texts.len() > BEFORE {
        return;
    }
    // "attached to it" is written for the card, for what it is attached to and for the first target alike: only a rule
    // about the first target makes it "that creature".
    let of_target = texts.iter().any(|text| text.contains("attached to it")) && {
        let json = serde_json::to_string(ability).unwrap_or_default();
        json.contains(r#""AttachedTo":{"Target":0}"#)
            && !json.contains(r#""AttachedTo":"This""#)
            && !json.contains(r#""AttachedTo":"Attached""#)
    };
    let rows: [fn(&str) -> Vec<String>; 16] = [
        pair_mana,
        subtype_bonus,
        card_again,
        if of_target {
            that_creature
        } else {
            that_creature_unattached
        },
        one_duration,
        duration_first,
        one_sentence,
        your_choice,
        super::r7b_more::each_and,
        super::r7b_more::an_vowel,
        super::r7b_more::then_discards,
        super::r7b_more::and_you,
        super::r7b_more::that_has,
        super::r7b_more::flicker_it,
        super::r7b_more::any_number,
        quote_period,
    ];
    let mut seen: std::collections::HashSet<String> = texts.iter().cloned().collect();
    let mut added = 0;
    for row in rows {
        let found: Vec<String> = texts.iter().flat_map(|text| row(text)).collect();
        for text in found {
            if added >= LIMIT {
                return;
            }
            if seen.insert(text.clone()) {
                texts.push(text);
                added += 1;
            }
        }
    }
}

/// "two mana in any combination of {U} and/or {R}" said as "{U}{U}, {U}{R}, or {R}{R}": two mana, each of one of two
/// colors, is one of the three pairs.
pub(super) fn pair_mana(text: &str) -> Vec<String> {
    const HEAD: &str = "two mana in any combination of ";
    let Some(at) = text.find(HEAD) else {
        return Vec::new();
    };
    let rest = &text[at + HEAD.len()..];
    let Some((first, after)) = symbol(rest) else {
        return Vec::new();
    };
    let Some(after) = after.strip_prefix(" and/or ") else {
        return Vec::new();
    };
    let Some((second, tail)) = symbol(after) else {
        return Vec::new();
    };
    vec![format!(
        "{}{first}{first}, {first}{second}, or {second}{second}{tail}",
        &text[..at]
    )]
}

/// The one-letter mana symbol a text opens with, and the rest.
fn symbol(text: &str) -> Option<(&str, &str)> {
    let end = text.find('}')?;
    let found = &text[..=end];
    (found.len() == 3 && found.starts_with('{')).then_some((found, &text[end + 1..]))
}

/// `"... target.".` said as `"... target."`, and `"... target." until end of turn` as `"... target" until end of turn`:
/// the period of a quoted ability is the period of the sentence it ends, and none stands before the sentence goes on.
pub(super) fn quote_period(text: &str) -> Vec<String> {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(".\"") {
        let after = &rest[at + 2..];
        if let Some(tail) = after.strip_prefix('.') {
            out.push_str(&rest[..at + 2]);
            rest = tail;
        } else if after.starts_with(' ') && after[1..].starts_with(|c: char| c.is_ascii_lowercase())
        {
            out.push_str(&rest[..at]);
            out.push('"');
            rest = after;
        } else {
            out.push_str(&rest[..at + 2]);
            rest = after;
        }
    }
    out.push_str(rest);
    if out == text { Vec::new() } else { vec![out] }
}

/// The words a bonus or a keyword that matters only to a creature may be said with after "you control".
const BONUS_WORDS: [&str; 18] = [
    "get",
    "have",
    "gain",
    "and",
    "vigilance",
    "trample",
    "haste",
    "flying",
    "menace",
    "reach",
    "first",
    "double",
    "strike",
    "until",
    "end",
    "of",
    "turn",
    "X",
];

/// "Dinosaur creatures you control get +1/+1" said as "Dinosaurs you control get +1/+1": the permanents of a creature
/// subtype that are not creatures have no power or toughness and never attack or block, so a bonus or a combat keyword
/// given to them changes nothing (CR 109.2 reads the subtype alone as permanents).
pub(super) fn subtype_bonus(text: &str) -> Vec<String> {
    const MIDDLE: &str = " creatures you control ";
    let mut out = Vec::new();
    for (at, _) in text.match_indices(MIDDLE) {
        let before = &text[..at];
        let start = before.rfind(' ').map_or(0, |space| space + 1);
        let word = &before[start..];
        let plural = if word == "outlaw" {
            "outlaws".to_owned()
        } else if creature_types().contains(&word) {
            plural_subtype(word)
        } else {
            continue;
        };
        let rest = &text[at + MIDDLE.len()..];
        let sentence = rest.split_once('.').map_or(rest, |(head, _)| head);
        let bonus = sentence
            .split([' ', ','])
            .filter(|w| !w.is_empty())
            .all(|w| {
                BONUS_WORDS.contains(&w)
                    || (w.contains('/') && w.chars().all(|c| "+-/0123456789X".contains(c)))
            });
        let verb = ["get ", "have ", "gain "]
            .iter()
            .any(|verb| rest.starts_with(verb));
        if bonus && verb {
            out.push(format!("{}{plural} you control {rest}", &text[..start]));
        }
    }
    out
}

/// The singular nouns of an object: between two names of the card, one of them makes "it" another object.
const OBJECTS: [&str; 17] = [
    "creature",
    "card",
    "permanent",
    "token",
    "spell",
    "land",
    "artifact",
    "enchantment",
    "equipment",
    "aura",
    "planeswalker",
    "vehicle",
    "ability",
    "it",
    "its",
    "that",
    "this",
];

/// Whether a stretch of text between two names of the card names no other object: no singular object noun, no target
/// that is not a player, no quotation mark.
fn names_nothing(between: &str) -> bool {
    if between.contains('"') || between.contains("any target") {
        return false;
    }
    let words: Vec<String> = between
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '\'')
        .filter(|w| !w.is_empty())
        .map(str::to_ascii_lowercase)
        .collect();
    words.iter().enumerate().all(|(at, word)| {
        if word == "target" {
            return words
                .get(at + 1)
                .is_some_and(|next| next == "player" || next == "opponent");
        }
        !OBJECTS.contains(&word.as_str())
    })
}

/// "put a +1/+1 counter on ~" said as "put a +1/+1 counter on it" after the card is named: once the card is named, "it"
/// is the card until another object is named. A name after "named" is a name, never the card.
pub(super) fn card_again(text: &str) -> Vec<String> {
    let names: Vec<usize> = text
        .match_indices('~')
        .map(|(at, _)| at)
        .filter(|&at| {
            !text[at..].starts_with("~'s") && (at == 0 || text[..at].ends_with([' ', '"']))
        })
        .collect();
    let mut again = Vec::new();
    for pair in names.windows(2) {
        let (before, at) = (pair[0], pair[1]);
        if names_nothing(&text[before + 1..at]) && !text[..at].ends_with("named ") {
            again.push(at);
        }
    }
    if again.is_empty() || again.len() > 3 {
        return Vec::new();
    }
    let with = |chosen: &[usize]| {
        let mut out = String::with_capacity(text.len() + 8);
        let mut from = 0;
        for &at in chosen {
            out.push_str(&text[from..at]);
            let opens =
                text[..at].trim_end().is_empty() || text[..at].trim_end().ends_with(['.', ':']);
            out.push_str(if opens { "It" } else { "it" });
            from = at + 1;
        }
        out.push_str(&text[from..]);
        out
    };
    let mut out = vec![with(&again)];
    if again.len() > 1 {
        out.extend(again.iter().map(|&at| with(&[at])));
    }
    out
}

/// Whether the last object named before the end of `before` is one creature: "target creature until end of turn.
/// Untap" names a creature last, "attach it to target creature you control. Untap" too, "~ enters, attach" does not.
fn creature_last(before: &str) -> bool {
    let words: Vec<String> = before
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '~' && c != '\'')
        .filter(|w| !w.is_empty())
        .map(str::to_ascii_lowercase)
        .collect();
    for (at, word) in words.iter().enumerate().rev() {
        let after_all = at > 0 && words[at - 1] == "all";
        match word.as_str() {
            // "is a creature" says what something is, and "each creature" is many: neither is "it".
            "creature" => {
                let said = at >= 2
                    && matches!(words[at - 1].as_str(), "a" | "an")
                    && matches!(words[at - 2].as_str(), "is" | "becomes" | "it's" | "that's");
                return at == 0 || (words[at - 1] != "each" && !said);
            }
            _ if after_all => {}
            "~" | "it" | "creatures" | "them" | "they" | "those" => return false,
            other if OBJECTS.contains(&other) => return false,
            _ => {}
        }
    }
    false
}

/// [`that_creature`] for a text whose "attached to it" may name the card.
fn that_creature_unattached(text: &str) -> Vec<String> {
    let mut out = that_creature(text);
    out.retain(|found| !found.contains("attached to that creature"));
    out
}

/// "Untap it" said as "Untap that creature", and "that creature" as "it": both name the creature named last.
pub(super) fn that_creature(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (at, _) in text.match_indices(" it") {
        let end = at + 3;
        let closes = text[end..].is_empty() || text[end..].starts_with([' ', '.', ',']);
        if closes && creature_last(&text[..at]) {
            out.push(format!("{} that creature{}", &text[..at], &text[end..]));
        }
    }
    for (at, found) in text.match_indices("hat creature") {
        let after = &text[at + found.len()..];
        let ends = after.is_empty() || after.starts_with(['.', ',']) || after.starts_with(' ');
        let noun = [" card", " token", " spell"]
            .iter()
            .any(|n| after.starts_with(n));
        let starts = at < 2 || text[..at - 1].ends_with(' ');
        if at == 0 || !ends || noun || !starts || !creature_last(&text[..at - 1]) {
            continue;
        }
        let pronoun = match &text[at - 1..at] {
            "t" => "it",
            "T" => "It",
            _ => continue,
        };
        out.push(format!(
            "{}{pronoun}{}",
            &text[..at - 1],
            &text[at + found.len()..]
        ));
    }
    out.truncate(4);
    out
}

/// "~ loses defender until end of turn and gains vigilance until end of turn" said as "~ loses defender and gains
/// vigilance until end of turn": the duration at the end of a sentence is the duration of each of its effects.
pub(super) fn one_duration(text: &str) -> Vec<String> {
    const FIRST: &str = " until end of turn and ";
    let Some(at) = text.find(FIRST) else {
        return Vec::new();
    };
    let rest = &text[at + FIRST.len()..];
    let sentence = sentence_end(rest).map_or(rest, |end| &rest[..end]);
    if !sentence
        .trim_end_matches('.')
        .ends_with(" until end of turn")
        || sentence.contains('"')
    {
        return Vec::new();
    }
    vec![format!("{} and {rest}", &text[..at])]
}

/// Where the sentence a text opens with ends: the first ". " outside a quotation.
fn sentence_end(text: &str) -> Option<usize> {
    let mut quoted = false;
    let bytes = text.as_bytes();
    for (at, &byte) in bytes.iter().enumerate() {
        match byte {
            b'"' => quoted = !quoted,
            b'.' if !quoted && bytes.get(at + 1) == Some(&b' ') => return Some(at),
            _ => {}
        }
    }
    None
}

/// The words that may open a subject and are written in lower case after a comma.
const LOWER: [&str; 17] = [
    "All",
    "Target",
    "Creatures",
    "Lands",
    "Artifacts",
    "Each",
    "Other",
    "Colorless",
    "Attacking",
    "Blocking",
    "Nontoken",
    "Untapped",
    "Tapped",
    "Another",
    "Enchanted",
    "Equipped",
    "Nonland",
];

/// `Creatures you control gain "..." until end of turn` said as `Until end of turn, creatures you control gain "..."`: a
/// granted ability ends its sentence, so the duration goes first.
pub(super) fn duration_first(text: &str) -> Vec<String> {
    const TAIL: &str = "\" until end of turn";
    let Some(close) = text.rfind(TAIL) else {
        return Vec::new();
    };
    let after = &text[close + TAIL.len()..];
    if !(after.is_empty() || after == ".") {
        return Vec::new();
    }
    let Some(open) = text[..close].find('"') else {
        return Vec::new();
    };
    let head = &text[..open];
    let Some(verb) = [" gains ", " gain "]
        .iter()
        .filter_map(|verb| head.rfind(verb))
        .max()
    else {
        return Vec::new();
    };
    let start = [". ", ": ", ", "]
        .iter()
        .filter_map(|mark| head[..verb].rfind(mark).map(|at| at + mark.len()))
        .max()
        .unwrap_or(0);
    let subject = &text[start..open];
    let first = subject.split(' ').next().unwrap_or_default();
    let opens = start == 0 || text[..start].ends_with(". ") || text[..start].ends_with(": ");
    let lowered = if LOWER.contains(&first) {
        let mut chars = subject.chars();
        chars.next().map_or_else(String::new, |c| {
            c.to_ascii_lowercase().to_string() + chars.as_str()
        })
    } else if first == "~" || first.starts_with(|c: char| c.is_ascii_uppercase()) {
        subject.to_owned()
    } else {
        return Vec::new();
    };
    let duration = if opens {
        "Until end of turn, "
    } else {
        "until end of turn, "
    };
    vec![format!(
        "{}{duration}{lowered}{}\"{after}",
        &text[..start],
        &text[open..close]
    )]
}

/// The verbs of an effect a subject has until end of turn.
const VERBS: [&str; 12] = [
    "gets ", "get ", "gains ", "gain ", "loses ", "lose ", "has ", "have ", "becomes ", "become ",
    "can't ", "can ",
];

/// "Until end of turn, ~ loses defender. ~ gains flying until end of turn." said as "Until end of turn, ~ loses defender
/// and gains flying.": two effects of one subject with one duration are one sentence. When the first effect ends with the
/// verb of the second ("gains trample"), the verb is said once ("gains trample and \"...\"").
pub(super) fn one_sentence(text: &str) -> Vec<String> {
    const HEAD: &str = "Until end of turn, ";
    const TAIL: &str = " until end of turn.";
    let Some(at) = text.find(HEAD) else {
        return Vec::new();
    };
    let first_start = at + HEAD.len();
    let Some(end) = sentence_end(&text[first_start..]) else {
        return Vec::new();
    };
    let first = &text[first_start..first_start + end];
    let second = &text[first_start + end + 2..];
    let Some(second) = second.strip_suffix(TAIL).or_else(|| {
        second
            .strip_suffix(" until end of turn")
            .filter(|s| s.ends_with('"'))
    }) else {
        return Vec::new();
    };
    if sentence_end(second).is_some() {
        return Vec::new();
    }
    let Some(verb) = VERBS
        .iter()
        .find_map(|verb| {
            let rest = second
                .strip_prefix("It ")
                .filter(|_| !plural_subject(first))?;
            rest.starts_with(verb).then_some(rest)
        })
        .or_else(|| same_subject(first, second))
    else {
        return Vec::new();
    };
    let mut out = vec![format!("{}{first} and {verb}.", &text[..first_start])];
    if let Some((word, effect)) = verb.split_once(' ')
        && first.contains(&format!(" and {word} "))
    {
        out.push(format!("{}{first} and {effect}.", &text[..first_start]));
    }
    out
}

/// Whether the subject of an effect is a group.
fn plural_subject(effect: &str) -> bool {
    effect
        .split(' ')
        .any(|w| w.ends_with("creatures") || w == "permanents")
}

/// What the second effect does after the subject the first says too ("Outlaw creatures you control gain" after "outlaw
/// creatures you control get"): the second effect from its verb.
fn same_subject<'a>(first: &str, second: &'a str) -> Option<&'a str> {
    VERBS.iter().find_map(|verb| {
        let at = second.find(verb)?;
        let subject = &second[..at];
        if subject.is_empty() || subject.contains(['.', ',', '"']) {
            return None;
        }
        let mut chars = subject.chars();
        let lowered = chars.next()?.to_ascii_lowercase().to_string() + chars.as_str();
        let rest = first
            .strip_prefix(subject)
            .or_else(|| first.strip_prefix(lowered.as_str()))?;
        VERBS
            .iter()
            .any(|v| rest.starts_with(v))
            .then_some(&second[at..])
    })
}

/// "~ gains flying until end of turn or ~ gains vigilance until end of turn" said as "~ gains your choice of flying or
/// vigilance until end of turn", and "~ gets +1/-1 until end of turn or ~ gets -1/+1 until end of turn" as "~ gets
/// +1/-1 or -1/+1 until end of turn": the controller chooses one, in any order.
pub(super) fn your_choice(text: &str) -> Vec<String> {
    const EOT: &str = " until end of turn";
    let Some(first_end) = text.find(" until end of turn or ") else {
        return Vec::new();
    };
    let head = &text[..first_end];
    let Some((verb, at)) = [" gains ", " gets "]
        .iter()
        .filter_map(|verb| head.rfind(verb).map(|at| (*verb, at)))
        .max_by_key(|(_, at)| *at)
    else {
        return Vec::new();
    };
    let start = [". ", ": ", ", "]
        .iter()
        .filter_map(|mark| head[..at].rfind(mark).map(|found| found + mark.len()))
        .max()
        .unwrap_or(0);
    let subject = &text[start..at];
    let mut choices = vec![&text[at + verb.len()..first_end]];
    let mut rest = &text[first_end + EOT.len()..];
    while let Some(next) = rest.strip_prefix(" or ") {
        let subjects: &[&str] = if matches!(subject, "~" | "It" | "it") {
            &["~", "it"]
        } else {
            &[subject, "it"]
        };
        let named = subjects
            .iter()
            .find_map(|s| next.strip_prefix(&format!("{s}{verb}")));
        let Some(named) = named else {
            return Vec::new();
        };
        let Some(end) = named.find(EOT) else {
            return Vec::new();
        };
        choices.push(&named[..end]);
        rest = &named[end + EOT.len()..];
    }
    if choices.len() > 3
        || choices
            .iter()
            .any(|c| c.contains([',', '.', '"']) || c.contains(" or "))
    {
        return Vec::new();
    }
    let lead = &text[..at + verb.len()];
    let mut out = Vec::new();
    for order in orders(choices.len()) {
        let list: Vec<&str> = order.iter().map(|&i| choices[i]).collect();
        let joined = match list.as_slice() {
            [a, b] => format!("{a} or {b}"),
            [a, b, c] => format!("{a}, {b}, or {c}"),
            _ => return Vec::new(),
        };
        if verb == " gains " {
            out.push(format!("{lead}your choice of {joined}{EOT}{rest}"));
        } else {
            out.push(format!("{lead}{joined}{EOT}{rest}"));
        }
    }
    out
}

/// The orders of two or three choices.
fn orders(count: usize) -> Vec<Vec<usize>> {
    match count {
        2 => vec![vec![0, 1], vec![1, 0]],
        3 => vec![
            vec![0, 1, 2],
            vec![0, 2, 1],
            vec![1, 0, 2],
            vec![1, 2, 0],
            vec![2, 0, 1],
            vec![2, 1, 0],
        ],
        _ => Vec::new(),
    }
}
