//! Oracle wordings of rules the writers write in other words: a land that enters tapped counts "other lands" (it is not
//! on the battlefield yet), a kicked creature "enters with" its counters "if it was kicked", a copy of "any" permanent on
//! the battlefield, the card types of a creature in Oracle order ("artifact creatures"), "an additional" bonus, a sum
//! that names its constant first, a damage source of a color, and a creature that "can't be blocked except by" others.

use serde_json::Value;

use super::amounts::count_word;
use crate::ability::Ability;

/// Each row is a phrase the writers give and the phrase the Oracle text uses for the same rule, anywhere in a wording.
const PHRASES: &[(&str, &str)] = &[
    ("Creature lands you control", "Land creatures you control"),
    ("creature lands you control", "land creatures you control"),
    ("Creature artifacts", "Artifact creatures"),
    ("creature artifacts", "artifact creatures"),
    (
        "Attacking creature tokens you control",
        "Attacking tokens you control",
    ),
    (
        " for each instant or sorcery card in your graveyard",
        " for each instant and sorcery card in your graveyard",
    ),
    (
        "artifact, battle, creature, enchantment, land, or planeswalker cards",
        "permanent cards",
    ),
    (
        " pays {X}, where X is the number of cards in your graveyard",
        " pays {1} for each card in your graveyard",
    ),
    ("was spent to cast it,", "was spent to cast ~,"),
    (
        "cast your second spell you control each turn",
        "cast your second spell each turn",
    ),
    (" white untapped ", " untapped white "),
    (" blue untapped ", " untapped blue "),
    (" black untapped ", " untapped black "),
    (" red untapped ", " untapped red "),
    (" green untapped ", " untapped green "),
];

/// Adds the Oracle wordings of the rows to the wordings of an ability.
pub fn add(ability: &Ability, texts: &mut Vec<String>) {
    let source = colored_source(ability);
    // Two rows may meet in one wording ("an additional" bonus counted in words), so the rows run over their own output once.
    let first = rewritten(texts, source);
    let second = rewritten(&first, source);
    for text in first.into_iter().chain(second) {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

/// The wordings the rows give for these wordings.
fn rewritten(texts: &[String], colored_source: bool) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for text in texts {
        for (written, oracle) in PHRASES {
            if text.contains(written) {
                found.push(text.replace(written, oracle));
            }
        }
        found.extend(WHOLE.iter().filter_map(|row| row(text)));
        if colored_source && text.contains(" permanent would deal damage") {
            found.push(text.replace(" permanent would deal damage", " source would deal damage"));
        }
    }
    found
}

/// The rows that rewrite a whole wording; each says `None` for a wording of another shape.
const WHOLE: &[fn(&str) -> Option<String>] = &[
    kicked_counters,
    lands_unless_fewer,
    lands_unless_more,
    lands_if_more,
    copy_on_battlefield,
    exiled_from_hand,
    divided_any_number,
    blocked_except_by,
    sum_constant_first,
    two_targets_each,
    enchanted_creature,
    additional_bonus,
    colors_swapped,
    count_in_words,
    prevented_numeral,
    duration_last,
];

/// "~ enters with two +1/+1 counters on it if this spell was kicked." is "If ~ was kicked, it enters with ...".
fn kicked_counters(text: &str) -> Option<String> {
    let rest = text.strip_prefix("~ enters with ")?;
    let counters = rest.strip_suffix(" if this spell was kicked.")?;
    let counters = counters
        .strip_suffix(" on her")
        .or_else(|| counters.strip_suffix(" on him"))
        .or_else(|| counters.strip_suffix(" on it"))?;
    Some(format!("If ~ was kicked, it enters with {counters} on it."))
}

/// The land is not on the battlefield as it enters: "If you control three or more lands, ~ enters tapped." is "~ enters
/// tapped unless you control two or fewer other lands."
fn lands_unless_fewer(text: &str) -> Option<String> {
    let count = text
        .strip_prefix("If you control ")?
        .strip_suffix(" or more lands, ~ enters tapped.")?;
    let fewer = count_word(super::amounts::number_word(count)?.checked_sub(1)?)?;
    Some(format!(
        "~ enters tapped unless you control {fewer} or fewer other lands."
    ))
}

/// "~ enters tapped unless you control two or more lands." is "... two or more other lands." as the land enters.
fn lands_unless_more(text: &str) -> Option<String> {
    let count = text
        .strip_prefix("~ enters tapped unless you control ")?
        .strip_suffix(" or more lands.")?;
    super::amounts::number_word(count)?;
    Some(format!(
        "~ enters tapped unless you control {count} or more other lands."
    ))
}

/// "If you control two or more lands, ~ enters tapped." is "If you control two or more other lands, ..." as it enters.
fn lands_if_more(text: &str) -> Option<String> {
    let count = text
        .strip_prefix("If you control ")?
        .strip_suffix(" or more lands, ~ enters tapped.")?;
    super::amounts::number_word(count)?;
    Some(format!(
        "If you control {count} or more other lands, ~ enters tapped."
    ))
}

/// A permanent entering as a copy is not on the battlefield yet: "a copy of any other creature on the battlefield" is
/// "a copy of any creature on the battlefield".
fn copy_on_battlefield(text: &str) -> Option<String> {
    let (head, tail) = text.split_once(" enter as a copy of any other ")?;
    let (kind, rest) = tail.split_once(" on the battlefield")?;
    if kind.contains(['.', ',']) {
        return None;
    }
    Some(format!(
        "{head} enter as a copy of any {kind} on the battlefield{rest}"
    ))
}

/// The spell is on the stack while its cost is paid: "exile another red card from your hand rather than pay" is
/// "exile a red card from your hand rather than pay".
fn exiled_from_hand(text: &str) -> Option<String> {
    let (head, tail) = text.split_once(" exile another ")?;
    let (kind, rest) = tail.split_once(" card from your hand rather than pay ")?;
    if kind.contains(['.', ',', ' ']) {
        return None;
    }
    Some(format!(
        "{head} exile a {kind} card from your hand rather than pay {rest}"
    ))
}

/// Each target of divided damage gets at least 1 (CR 601.2d): "deals 4 damage divided as you choose among up to four"
/// is "among any number of".
fn divided_any_number(text: &str) -> Option<String> {
    let (head, tail) = text.split_once(" damage divided as you choose among up to ")?;
    let amount = head.rsplit(' ').next()?.parse::<u32>().ok()?;
    let (count, rest) = tail.split_once(' ')?;
    (count_word(amount)? == count)
        .then(|| format!("{head} damage divided as you choose among any number of {rest}"))
}

/// "Creatures without flying can't block it." is "~ can't be blocked except by creatures with flying."; "Nonblack
/// creatures can't block it." is "~ can't be blocked except by black creatures."
fn blocked_except_by(text: &str) -> Option<String> {
    let subject = text.strip_suffix(" can't block it.")?;
    if let Some(quality) = subject.strip_prefix("Creatures without ") {
        return Some(format!(
            "~ can't be blocked except by creatures with {quality}."
        ));
    }
    let color = subject.strip_prefix("Non")?.strip_suffix(" creatures")?;
    ["white", "blue", "black", "red", "green"]
        .contains(&color)
        .then(|| format!("~ can't be blocked except by {color} creatures."))
}

/// A sum names its constant first in Oracle text: "equal to the number of lands you control plus 1" is "equal to 1
/// plus the number of lands you control".
fn sum_constant_first(text: &str) -> Option<String> {
    let body = text.strip_suffix('.')?;
    let (head, tail) = body.split_once(" the number of ")?;
    let (counted, constant) = tail.rsplit_once(" plus ")?;
    constant.parse::<u32>().ok()?;
    if counted.contains(['.', ',']) || !(head.ends_with(" equal to") || head.ends_with(" X is")) {
        return None;
    }
    Some(format!("{head} {constant} plus the number of {counted}."))
}

/// "deals 3 damage to two target creatures" is "deals 3 damage to each of two target creatures".
fn two_targets_each(text: &str) -> Option<String> {
    let (head, tail) = text.split_once(" damage to two target ")?;
    Some(format!("{head} damage to each of two target {tail}"))
}

/// Prevented damage is a numeral in Oracle text: "prevent two of that damage" is "prevent 2 of that damage".
fn prevented_numeral(text: &str) -> Option<String> {
    let (head, tail) = text.split_once("prevent ")?;
    let (count, rest) = tail.split_once(" of that damage")?;
    let number = super::amounts::number_word(count).filter(|_| count != "a" && count != "an")?;
    Some(format!("{head}prevent {number} of that damage{rest}"))
}

/// Whether the ability replaces damage from a source named by its colors only, which the writers call a permanent.
fn colored_source(ability: &Ability) -> bool {
    let Ok(value) = serde_json::to_value(ability) else {
        return false;
    };
    let source = &value["Replacement"]["when"]["Damage"]["source"];
    source.as_array().is_some_and(|preds| {
        !preds.is_empty()
            && preds
                .iter()
                .all(|pred| matches!(pred, Value::Object(map) if map.contains_key("Color")))
    })
}

/// An Aura's bonus reaches only a creature: "Enchanted creature gets +2/+1." is "As long as enchanted permanent is a
/// creature, it gets +2/+1."
fn enchanted_creature(text: &str) -> Option<String> {
    let rest = text.strip_prefix("Enchanted creature ")?;
    (rest.starts_with("gets ") || rest.starts_with("has "))
        .then(|| format!("As long as enchanted permanent is a creature, it {rest}"))
}

/// "an additional" bonus is the bonus: "that creature gets +2/+2" is "that creature gets an additional +2/+2", and
/// "and 1 damage to each green creature" is "and an additional 1 damage to each green creature".
fn additional_bonus(text: &str) -> Option<String> {
    // Oracle text says "additional" of a bonus on top of another or under a condition; only those wordings get it.
    let bonuses = text.matches(" get").count() + text.matches(" damage to each ").count();
    if bonuses < 2 && !text.to_lowercase().contains("as long as ") {
        return None;
    }
    for (written, oracle) in [
        (" gets +", " gets an additional +"),
        (" gets -", " gets an additional -"),
        (" get +", " get an additional +"),
    ] {
        if let Some(at) = text.rfind(written) {
            return Some(format!(
                "{}{oracle}{}",
                &text[..at],
                &text[at + written.len()..]
            ));
        }
    }
    let (head, tail) = text.split_once(" and ")?;
    let (amount, rest) = tail.split_once(" damage to each ")?;
    amount.parse::<u32>().ok()?;
    Some(format!(
        "{head} and an additional {amount} damage to each {rest}"
    ))
}

/// Two colors joined by "or" name the same colors in either order: "a green or red card" is "a red or green card".
fn colors_swapped(text: &str) -> Option<String> {
    const COLORS: [&str; 5] = ["white", "blue", "black", "red", "green"];
    for first in COLORS {
        for second in COLORS {
            let pair = format!(" {first} or {second} ");
            if first != second && text.contains(&pair) {
                return Some(text.replacen(&pair, &format!(" {second} or {first} "), 1));
            }
        }
    }
    None
}

/// A count of cards in a condition is a word in Oracle text: "there are 7 or more cards" is "there are seven or more
/// cards".
fn count_in_words(text: &str) -> Option<String> {
    let (head, tail) = text.split_once(" there are ")?;
    let (number, rest) = tail.split_once(" or more ")?;
    let word = count_word(number.parse().ok()?)?;
    Some(format!("{head} there are {word} or more {rest}"))
}

/// A duration before the last sentence's subject is the same duration after it: "..., until end of turn, ~ gains double
/// strike." is "..., ~ gains double strike until end of turn."
fn duration_last(text: &str) -> Option<String> {
    let (head, tail) = text.split_once(", until end of turn, ")?;
    let effect = tail.strip_suffix('.')?;
    if effect.contains(['.', ',', '"']) {
        return None;
    }
    Some(format!("{head}, {effect} until end of turn."))
}
