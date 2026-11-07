//! Oracle wordings of rules the writers say another way, each derived from one wording the writers give: a creature
//! that "can't attack unless you control" what the condition counts, a spell that "costs less to cast if", the colors
//! of a mana choice in any order, a filter land's three pairs, "That creature" for "Then it also", "unless they pay",
//! two changes of one continuous effect in the other order, "instant and sorcery spells" for "instant and/or sorcery
//! spells", and a duration of "your turn" after the effect.

use super::amounts::{count_word, number_word};
use super::subtypes::{is_subtype, singular_subtype};
use crate::ability::Ability;

/// The most wordings the rows add to one ability: a long text could otherwise multiply them.
const LIMIT: usize = 48;

/// The most wordings a static may have for the rows to add to them.
const MOST_GIVEN: usize = 64;

/// Adds the Oracle wordings of each row to the wordings of the ability, one pass over the given wordings.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    // The rows are for the short wordings of one rule; a static of many joined parts has enough wordings already.
    if matches!(ability, Ability::Static { .. }) && texts.len() > MOST_GIVEN {
        return;
    }
    let json = serde_json::to_string(ability).unwrap_or_default();
    let cant = matches!(ability, Ability::Static { .. })
        && (json.contains("\"CantAttack\"") || json.contains("\"CantBlock\""));
    let mut found: Vec<String> = Vec::new();
    for text in texts.iter() {
        if cant {
            found.extend(cant_unless(text));
        }
        found.extend(cost_if(text));
        found.extend(mana_colors(text));
        found.extend(filter_pairs(text));
        found.extend(that_creature(text));
        found.extend(they_pay(text));
        found.extend(changes_swapped(text));
        found.extend(and_or(text));
        found.extend(your_turn_last(text));
        found.extend(may_have(text));
        found.extend(phrases(text));
        found.extend(super::r7a_more::rewrites(text));
        // The triggered abilities have these rows in `r6a_wordings` already.
        if !matches!(ability, Ability::Triggered { .. }) {
            found.extend(super::r6a_wordings::controller_first(text));
            found.extend(super::r6a_wordings::controller_last(text));
        }
    }
    let mut added = 0;
    for text in found {
        if added < LIMIT && !texts.contains(&text) {
            texts.push(text);
            added += 1;
        }
    }
}

/// The restrictions a creature's static can put on it.
const CANT: [(&str, &str); 4] = [
    ("can't attack and can't block", "can't attack or block"),
    ("can't attack or block", "can't attack or block"),
    ("can't attack", "can't attack"),
    ("can't block", "can't block"),
];

/// "As long as you control no other Zombies, ~ can't block." is "~ can't block unless you control another Zombie.", and
/// "As long as you control an untapped land, ~ can't block." is "~ can't block if you control an untapped land."
fn cant_unless(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let body = text.strip_suffix('.').unwrap_or(text);
    let parts = body
        .strip_prefix("As long as you control ")
        .and_then(|rest| rest.split_once(", ~ "))
        .or_else(|| {
            let rest = body.strip_prefix("~ ")?;
            let (cant, held) = rest.split_once(" as long as you control ")?;
            Some((held, cant))
        });
    let Some((held, cant)) = parts else {
        return out;
    };
    let Some((_, cant)) = CANT.iter().find(|(written, _)| *written == cant) else {
        return out;
    };
    if held.contains([',', '.']) {
        return out;
    }
    for wanted in unless_held(held) {
        out.push(format!("~ {cant} unless you control {wanted}."));
    }
    if !held.starts_with("no ") && !held.contains(" or fewer ") {
        out.push(format!("~ {cant} if you control {held}."));
    }
    out
}

/// What "unless you control" names for a condition that holds while you control too few: "no other Zombies" is
/// "another Zombie", "no Knights and Soldiers" is "a Knight or a Soldier", "one or fewer other Wolves" is "two or more
/// other Wolves".
fn unless_held(held: &str) -> Vec<String> {
    if let Some(rest) = held.strip_prefix("no other ") {
        return singular(rest).map_or_else(Vec::new, |one| vec![format!("another {one}")]);
    }
    if let Some(rest) = held.strip_prefix("no ") {
        if let Some((first, second)) = rest.split_once(" and ") {
            let (Some(first), Some(second)) = (singular(first), singular(second)) else {
                return Vec::new();
            };
            return vec![
                format!("{} {first} or {second}", article(&first)),
                format!(
                    "{} {first} or {} {second}",
                    article(&first),
                    article(&second)
                ),
            ];
        }
        return singular(rest)
            .map_or_else(Vec::new, |one| vec![format!("{} {one}", article(&one))]);
    }
    let Some((count, rest)) = held.split_once(" or fewer ") else {
        return Vec::new();
    };
    let Some(more) = number_word(count)
        .and_then(|n| n.checked_add(1))
        .and_then(count_word)
    else {
        return Vec::new();
    };
    vec![
        format!("{more} or more {rest}"),
        format!("at least {more} {rest}"),
    ]
}

/// A plural group in the singular: its first word is the plural of a type or subtype ("creatures with power 4 or
/// greater" is "creature with power 4 or greater").
fn singular(group: &str) -> Option<String> {
    let (noun, rest) = group.split_once(' ').unwrap_or((group, ""));
    let one = singular_subtype(noun, |one| {
        is_subtype(one)
            || [
                "artifact",
                "creature",
                "enchantment",
                "land",
                "planeswalker",
            ]
            .contains(&one)
    })?;
    Some(if rest.is_empty() {
        one
    } else {
        format!("{one} {rest}")
    })
}

/// The indefinite article of a word.
fn article(word: &str) -> &'static str {
    if word.starts_with(['a', 'e', 'i', 'o', 'u', 'A', 'E', 'I', 'O', 'U']) {
        "an"
    } else {
        "a"
    }
}

/// "If there are 8 or more cards in your graveyard, ~ costs {3} less to cast." is "This spell costs {3} less to cast if
/// you have eight or more cards in your graveyard."; the condition keeps its meaning in the Oracle words.
fn cost_if(text: &str) -> Vec<String> {
    let Some(rest) = text.strip_prefix("If ") else {
        return Vec::new();
    };
    let Some((condition, cost)) = rest.split_once(", ~ costs ") else {
        return Vec::new();
    };
    let Some(cost) = cost.strip_suffix(" to cast.") else {
        return Vec::new();
    };
    if condition.contains([',', '.']) || cost.contains([',', '.']) {
        return Vec::new();
    }
    let mut conditions = vec![condition.to_owned()];
    if let Some(cards) = condition.strip_prefix("there are ")
        && let Some((count, rest)) = cards.split_once(" or more ")
        && rest.ends_with(" in your graveyard")
        && let Some(word) = count.parse().ok().and_then(count_word)
    {
        conditions.push(format!("you have {word} or more {rest}"));
    }
    if let Some(life) = condition.strip_prefix("you gained ")
        && life.ends_with(" or more life this turn")
    {
        conditions.push(format!("you've gained {life}"));
    }
    if let Some(life) = condition
        .strip_prefix("you have ")
        .and_then(|rest| rest.strip_suffix(" or less life"))
        && life.parse::<u32>().is_ok()
    {
        conditions.push(format!("your life total is {life} or less"));
    }
    // The one additional cost a card has: a gift promised, or a Bargain paid.
    if condition == "the gift was promised" {
        conditions.push("it's bargained".to_owned());
    }
    conditions
        .into_iter()
        .map(|condition| format!("~ costs {cost} to cast if {condition}."))
        .collect()
}

/// The mana symbols of one color each.
const SYMBOLS: [&str; 6] = ["{W}", "{U}", "{B}", "{R}", "{G}", "{C}"];

/// "Add {W} or {G}." is "Add {G} or {W}.", and "Add {W}, {U}, or {G}." names the same three colors in any order: a
/// choice of one color among a list says nothing by its order.
fn mana_colors(text: &str) -> Vec<String> {
    let Some(at) = text.find("Add ") else {
        return Vec::new();
    };
    let rest = &text[at + "Add ".len()..];
    let Some(end) = rest.find('.') else {
        return Vec::new();
    };
    let list = &rest[..end];
    let symbols: Vec<&str> = list
        .split([',', ' '])
        .filter(|word| !word.is_empty() && *word != "or")
        .collect();
    let joined = |order: &[&str]| match order {
        [one, two] => format!("{one} or {two}"),
        [one, two, three] => format!("{one}, {two}, or {three}"),
        _ => String::new(),
    };
    if !(2..=3).contains(&symbols.len())
        || !symbols.iter().all(|symbol| SYMBOLS.contains(symbol))
        || joined(&symbols) != list
    {
        return Vec::new();
    }
    let orders: &[&[usize]] = if symbols.len() == 2 {
        &[&[1, 0]]
    } else {
        &[&[0, 2, 1], &[1, 0, 2], &[1, 2, 0], &[2, 0, 1], &[2, 1, 0]]
    };
    orders
        .iter()
        .map(|order| joined(&order.iter().map(|&at| symbols[at]).collect::<Vec<_>>()))
        .filter(|other| other != list)
        .map(|other| format!("{}Add {other}{}", &text[..at], &rest[end..]))
        .collect()
}

/// "Add two mana in any combination of {W} and/or {B}." is "Add {W}{W}, {W}{B}, or {B}{B}.": the three pairs are every
/// combination.
fn filter_pairs(text: &str) -> Option<String> {
    let (head, rest) = text.split_once("Add two mana in any combination of ")?;
    let (pair, tail) = rest.split_once('.')?;
    let (one, two) = pair.split_once(" and/or ")?;
    (SYMBOLS.contains(&one) && SYMBOLS.contains(&two) && one != two)
        .then(|| format!("{head}Add {one}{one}, {one}{two}, or {two}{two}.{tail}"))
}

/// "Put X +1/+1 counters on target creature. Then it also gains trample until end of turn." is "... That creature gains
/// trample until end of turn.": the new sentence names the target the sentence before chose.
fn that_creature(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (written, verb) in [
        (". Then it also gains ", "gains"),
        (". Then it also gets ", "gets"),
    ] {
        let Some(at) = text.find(written) else {
            continue;
        };
        let before = &text[..at];
        let sentence = &before[before.rfind(". ").map_or(0, |stop| stop + 2)..];
        let targets = sentence.matches("target ").count();
        if targets == 1 && sentence.contains("target creature") && !sentence.contains(" you may ") {
            out.push(format!(
                "{before}. That creature {verb} {}",
                &text[at + written.len()..]
            ));
        }
    }
    out
}

/// "that player sacrifices a permanent unless that player pays {1}" is "... a permanent of their choice unless they pay
/// {1}": the player who would pay is the one named before.
fn they_pay(text: &str) -> Vec<String> {
    const PAYS: &str = " unless that player pays ";
    if !text.contains(PAYS) {
        return Vec::new();
    }
    let paid = text.replacen(PAYS, " unless they pay ", 1);
    let mut out = vec![paid.clone()];
    if paid.contains(" sacrifices a permanent unless they pay ") {
        out.push(paid.replacen(
            " sacrifices a permanent unless",
            " sacrifices a permanent of their choice unless",
            1,
        ));
    }
    out
}

/// The two changes of one continuous effect in the other order: "have base power and toughness 1/1 and lose all
/// abilities" is "lose all abilities and have base power and toughness 1/1", "gets +X/+0 and gains trample" is "gains
/// trample and gets +X/+0", and "has defender and is a Wall in addition to its other creature types" the other way.
fn changes_swapped(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (base, loses) in [
        (" have base power and toughness ", " and lose all abilities"),
        (" has base power and toughness ", " and loses all abilities"),
    ] {
        if let Some(at) = text.find(base) {
            let rest = &text[at + base.len()..];
            if let Some((size, tail)) = rest.split_once(loses)
                && size
                    .split_once('/')
                    .is_some_and(|(p, t)| digits(p) && digits(t))
            {
                let have = base.trim_start();
                out.push(format!(
                    "{}{} and {have}{size}{tail}",
                    &text[..at],
                    &loses[" and".len()..]
                ));
            }
        }
    }
    for (gets, gains) in [(" gets ", " gains "), (" get ", " gain ")] {
        let Some(at) = text.find(gets) else {
            continue;
        };
        let rest = &text[at + gets.len()..];
        let Some((bonus, tail)) = rest.split_once(" and") else {
            continue;
        };
        let Some(tail) = tail.strip_prefix(gains) else {
            continue;
        };
        if !bonus.starts_with(['+', '-']) || bonus.contains(' ') {
            continue;
        }
        let end = tail.find([',', '.']).unwrap_or(tail.len());
        let (keyword, after) = tail.split_at(end);
        let (keyword, until) = keyword
            .strip_suffix(" until end of turn")
            .map_or((keyword, ""), |kept| (kept, " until end of turn"));
        if keyword.is_empty() || !keyword.chars().all(|c| c.is_ascii_lowercase() || c == ' ') {
            continue;
        }
        if keyword.contains(" and ") || keyword.contains(" for ") || keyword.contains(" until ") {
            continue;
        }
        out.push(format!(
            "{}{}{keyword} and{}{bonus}{until}{after}",
            &text[..at],
            gains,
            gets
        ));
    }
    if let Some(at) = text.find(" has ")
        && let Some((keyword, tail)) = text[at + " has ".len()..].split_once(" and is a ")
        && keyword.chars().all(|c| c.is_ascii_lowercase())
        && let Some((kind, after)) = tail.split_once(" in addition to its other creature types")
        && !kind.contains(' ')
    {
        out.push(format!(
            "{} is a {kind} in addition to its other creature types and has {keyword}{after}",
            &text[..at]
        ));
    }
    out
}

/// Whether a word is a number.
fn digits(word: &str) -> bool {
    !word.is_empty() && word.chars().all(|c| c.is_ascii_digit())
}

/// "instant and/or sorcery spells" is "instant and sorcery spells" (each spell is one or the other), "attacking and/or
/// blocking creatures" is "attacking or blocking creatures", and "a Lesson and/or Noble card" is "a Lesson or Noble
/// card": one object is one or the other or both.
fn and_or(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (written, printed) in [
        (
            " cast instant and/or sorcery spells",
            " cast instant and sorcery spells",
        ),
        (
            " target attacking and/or blocking creatures",
            " target attacking or blocking creatures",
        ),
    ] {
        if text.contains(written) {
            out.push(text.replacen(written, printed, 1));
        }
    }
    for (at, _) in text.match_indices(" and/or ") {
        let after = &text[at + " and/or ".len()..];
        let mut words = after.split(' ');
        let (Some(_), Some(noun)) = (words.next(), words.next()) else {
            continue;
        };
        let noun = noun.trim_end_matches([',', '.']);
        let before = text[..at].split(' ').next_back().unwrap_or_default();
        if ["card", "spell", "permanent"].contains(&noun) && !before.ends_with(',') {
            out.push(format!("{} or {after}", &text[..at]));
            // "for each creature and planeswalker card" counts each card that is one or the other.
            if text[..at].ends_with(&format!(" for each {before}")) {
                out.push(format!("{} and {after}", &text[..at]));
            }
        }
    }
    out
}

/// "you may it deals 2 damage" is "you may have it deal 2 damage", and "you may they discard a card" is "you may have
/// that player discard a card", and "you may ~ becomes a copy" is "you may have ~ become a copy": a step of another subject
/// than "you" after "you may" takes "have" and the bare verb.
fn may_have(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (written, named) in [
        ("you may it ", "it"),
        ("you may ~ ", "~"),
        ("you may they ", "that player"),
    ] {
        let Some(at) = text.find(written) else {
            continue;
        };
        let rest = &text[at + written.len()..];
        let Some((verb, tail)) = rest.split_once(' ') else {
            continue;
        };
        let bare = if named != "that player" {
            verb.strip_suffix('s')
        } else {
            Some(verb)
        };
        if let Some(bare) = bare
            && bare.chars().all(|c| c.is_ascii_lowercase())
            && !["ha", "i", "doe", "may", "can't"].contains(&bare)
        {
            out.push(format!("{}you may have {named} {bare} {tail}", &text[..at]));
        }
    }
    out
}

/// Exact phrase rows: `(written, printed)`, the same group in the Oracle words.
const PHRASES: [(&str, &str); 2] = [
    // A creature an opponent controls is one "each opponent" controls.
    (
        " on each creature your opponents control",
        " on each creature each opponent controls",
    ),
    // A spell you cast is yours whatever its quality.
    (
        "spells of the chosen type you cast ",
        "spells you cast of the chosen type ",
    ),
];

/// The phrase rows that apply to a wording.
fn phrases(text: &str) -> Vec<String> {
    PHRASES
        .iter()
        .filter(|(written, _)| text.contains(written))
        .map(|(written, printed)| text.replacen(written, printed, 1))
        .collect()
}

/// "During your turn, ~ has first strike." is "~ has first strike during your turn.": the duration after the effect.
fn your_turn_last(text: &str) -> Option<String> {
    let rest = text.strip_prefix("During your turn, ")?;
    let effect = rest.strip_suffix('.')?;
    if effect.contains(['.', ',', '"']) {
        return None;
    }
    let mut chars = effect.chars();
    let first = chars.next()?;
    Some(format!(
        "{}{} during your turn.",
        first.to_uppercase(),
        chars.as_str()
    ))
}
