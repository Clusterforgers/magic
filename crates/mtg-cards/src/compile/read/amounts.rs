//! Amounts: counts of cards, points of life, damage or scrying, and the characteristic of an
//! object, read from their words and written back the same way.

use super::counters;
use super::selectors::{Form, phrase, phrase_text};
use super::tokens::Reader;
use crate::refs::{Pred, Ref};
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

/// How many cards, or a card for each thing counted: "a card", "two cards for each creature you control".
pub fn cards_counted(r: &mut Reader<'_>) -> Option<Amount> {
    r.attempt(|r| {
        let base = cards(r)?;
        counted(r, base)
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

/// What "for each" counts: "creature you control", "Goblin on the battlefield", "creature card in your graveyard",
/// "opponent", "charge counter on ~".
pub fn for_each(r: &mut Reader<'_>) -> Option<Amount> {
    r.attempt(|r| {
        r.need("for each")?;
        if let Some(counted) = r.attempt(counters::counted) {
            return Some(counted);
        }
        if r.word("opponent") {
            return Some(Amount::Players(Ref::Opponents));
        }
        let selector = phrase(r, Form::Count)?;
        // A group that no one holds stands on the battlefield.
        if !selector.0.iter().any(holds) {
            r.words("on the battlefield");
        }
        Some(Amount::Count(selector))
    })
}

/// Whether the predicate says who holds the objects or where they are.
fn holds(pred: &Pred) -> bool {
    match pred {
        // An attacking or blocking creature is on the battlefield, and the card does not say so.
        Pred::ControlledBy(_)
        | Pred::OwnedBy(_)
        | Pred::In(_)
        | Pred::Attacking
        | Pred::Blocking => true,
        Pred::Not(inner) => holds(inner),
        _ => false,
    }
}

/// An amount the effect gives for each thing counted: "a card for each creature you control" is the count itself, "two
/// cards for each ..." is the count times two. Without "for each" the amount is `base`.
pub fn counted(r: &mut Reader<'_>, base: Amount) -> Option<Amount> {
    let Some(each) = r.attempt(for_each) else {
        return Some(base);
    };
    match base {
        Amount::Fixed(1) => Some(each),
        Amount::Fixed(by) if by != 0 => Some(Amount::Times {
            by,
            of: Box::new(each),
        }),
        _ => None,
    }
}

/// What "for each" counts, as the card words it after "for each ".
pub fn for_each_text(counted: &Amount) -> Option<String> {
    Some(match counted {
        Amount::Count(selector) => {
            let phrase = phrase_text(selector, Form::Count)?;
            if selector.0.iter().any(holds) {
                phrase
            } else {
                format!("{phrase} on the battlefield")
            }
        }
        Amount::Players(Ref::Opponents) => "opponent".to_owned(),
        Amount::Counters {
            on: Ref::This,
            counter,
        } => format!("{} counter on ~", counters::kind_text(counter)?),
        _ => return None,
    })
}

/// A counted amount split into what each thing counted gives (a fixed amount) and the words "for each ...":
/// `None` for an amount that is not counted.
pub fn counted_parts(amount: &Amount) -> Option<(Amount, String)> {
    match amount {
        Amount::Times { by, of } => Some((
            Amount::Fixed(*by),
            format!("for each {}", for_each_text(of)?),
        )),
        other => Some((
            Amount::Fixed(1),
            format!("for each {}", for_each_text(other)?),
        )),
    }
}

/// "a card", "two cards", "X cards", "a card for each creature you control".
pub fn cards_text(amount: &Amount) -> Option<String> {
    if let Some((each, tail)) = counted_parts(amount) {
        return Some(format!("{} {tail}", cards_text(&each)?));
    }
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
