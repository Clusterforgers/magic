//! Amounts: counts of cards, points of life, damage or scrying, and the characteristic of an
//! object, read from their words and written back the same way.

use super::tokens::Reader;
use crate::value::{Amount, StatKind};

/// The count words "one" to "twenty", in order.
const NUMBER_WORDS: [&str; 20] = [
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
    "twenty",
];

/// The number a count word names ("a", "an", "two").
pub fn number_word(word: &str) -> Option<u32> {
    if matches!(word, "a" | "an") {
        return Some(1);
    }
    let at = NUMBER_WORDS.iter().position(|name| *name == word)?;
    u32::try_from(at + 1).ok()
}

/// The word for a count from one to twenty.
pub fn count_word(number: u32) -> Option<&'static str> {
    let at = usize::try_from(number.checked_sub(1)?).ok()?;
    NUMBER_WORDS.get(at).copied()
}

/// How many cards: "a card", "two cards", "X cards".
pub fn cards(r: &mut Reader<'_>) -> Option<Amount> {
    r.attempt(|r| {
        if r.words("that many") {
            return (r.word("card") || r.word("cards")).then_some(Amount::Triggering);
        }
        let count = if r.x_free && r.word("x") {
            Amount::X
        } else {
            let number = number_word(r.peek_word()?)?;
            r.skip();
            Amount::Fixed(i32::try_from(number).ok()?)
        };
        (r.word("card") || r.word("cards")).then_some(count)
    })
}

/// How many points of life, damage or scrying: digits, X, or "that much" (what the trigger's event was).
pub fn points(r: &mut Reader<'_>) -> Option<Amount> {
    if r.words("that much") {
        return Some(Amount::Triggering);
    }
    if r.x_free && r.word("x") {
        return Some(Amount::X);
    }
    Some(Amount::Fixed(i32::try_from(r.number()?).ok()?))
}

/// "its power", "its toughness", "its mana value": a characteristic of the object the last sentence acted on.
pub fn its_stat(r: &mut Reader<'_>) -> Option<Amount> {
    r.attempt(|r| {
        let of = r.it.clone()?;
        r.need("its")?;
        let stat = if r.word("power") {
            StatKind::Power
        } else if r.word("toughness") {
            StatKind::Toughness
        } else if r.words("mana value") {
            StatKind::ManaValue
        } else {
            return None;
        };
        Some(Amount::Stat { of, stat })
    })
}

/// The characteristic as the card names it.
pub const fn stat_name(stat: StatKind) -> Option<&'static str> {
    match stat {
        StatKind::Power => Some("power"),
        StatKind::Toughness => Some("toughness"),
        StatKind::ManaValue => Some("mana value"),
        StatKind::Loyalty => None,
    }
}

/// "a card", "two cards", "X cards".
pub fn cards_text(amount: &Amount) -> Option<String> {
    match amount {
        Amount::Fixed(1) => Some("a card".to_owned()),
        Amount::Fixed(number) => Some(format!(
            "{} cards",
            count_word(u32::try_from(*number).ok()?)?
        )),
        Amount::X => Some("X cards".to_owned()),
        Amount::Triggering => Some("that many cards".to_owned()),
        _ => None,
    }
}

/// Digits, or X.
pub fn points_text(amount: &Amount) -> Option<String> {
    match amount {
        Amount::Fixed(number) if *number > 0 => Some(number.to_string()),
        Amount::X => Some("X".to_owned()),
        Amount::Triggering => Some("that much".to_owned()),
        _ => None,
    }
}
