//! Amounts: counts of cards, points of life, damage or scrying, and the characteristic of an
//! object, read from their words and written back the same way.

use super::counters;
use super::defining;
use super::selectors::{Form, phrase, phrase_text};
use super::tokens::Reader;
use crate::modify::Modification;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
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
        if r.ability && r.triggered && r.words("that many") {
            return (r.word("card") || r.word("cards")).then_some(Amount::Triggering);
        }
        let count = if r.x() {
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
        // "cards equal to the number of opponents you have".
        if let Some(count) = r.attempt(|r| {
            r.need("cards")?;
            defining::equal_to(r)
        }) {
            return Some(count);
        }
        let base = cards(r)?;
        counted(r, base)
    })
}

/// How many points of life, damage or scrying: digits, X, or "that much" (what the trigger's event was; a spell
/// that counts an earlier amount of its own, as in "You gain that much life" after a loss, is left to the table).
pub fn points(r: &mut Reader<'_>) -> Option<Amount> {
    if r.triggered && r.words("that much") {
        return Some(Amount::Triggering);
    }
    if r.x() {
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
        StatKind::Loyalty | StatKind::Colors | StatKind::BasicLandTypes => None,
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
        if r.words("creature in your party") {
            return Some(Amount::Party(Ref::You));
        }
        // The permanent that devoured: "draw a card for each creature it devoured" (Skullmulcher).
        if r.words("creature it devoured") {
            return Some(Amount::Devoured);
        }
        if let Some(tally) = super::vote::w2d_tally(r, false) {
            return Some(tally);
        }
        if r.word("opponent") {
            // "Equipped creature gets +1/+0 for each opponent you have."
            r.words("you have");
            return Some(Amount::Players(Ref::Opponents));
        }
        if r.words("time you've cast") {
            (r.words("your commander") || r.words("a commander")).then_some(())?;
            r.need("from the command zone this game")?;
            return Some(Amount::CommanderCasts(Ref::You));
        }
        if let Some(died) = r.attempt(defining::died_each) {
            return Some(died);
        }
        // The colors of the spell that was cast: "put a +1/+1 counter on ~ for each of that spell's colors".
        if r.words("of that spell's colors") {
            return Some(Amount::Stat {
                of: Ref::Triggering,
                stat: StatKind::Colors,
            });
        }
        if let Some(phased) = super::w11b_phase::w11b_phased_each(r) {
            return Some(phased);
        }
        if let Some(destroyed) = super::w2c_counts::w2c_destroyed_each(r) {
            return Some(destroyed);
        }
        if let Some(distinct) = super::w2c_counts::w2c_distinct(r, false) {
            return Some(distinct);
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
    scale(&base, each)
}

/// The count `each` times a fixed, non-zero amount: one is the count itself.
pub fn scale(base: &Amount, each: Amount) -> Option<Amount> {
    match base {
        Amount::Fixed(1) => Some(each),
        Amount::Fixed(by) if *by != 0 => Some(Amount::Times {
            by: *by,
            of: Box::new(each),
        }),
        _ => None,
    }
}

/// A change of power and toughness counted "for each ...": every change that is not zero is that many for each thing
/// counted. A change that is not a fixed number (X) is not counted.
pub fn counted_stats(r: &mut Reader<'_>, stats: Modification) -> Option<Modification> {
    let Modification::ModifyPT { power, toughness } = stats else {
        return Some(stats);
    };
    let Some(each) = r.attempt(for_each) else {
        return Some(Modification::ModifyPT { power, toughness });
    };
    let per = |change: Amount| match change {
        Amount::Fixed(0) => Some(change),
        other => scale(&other, each.clone()),
    };
    // Both stats change by the same number for each thing, or one does not change.
    match (&power, &toughness) {
        (Amount::Fixed(0), Amount::Fixed(0)) => return None,
        (Amount::Fixed(by), Amount::Fixed(other)) if *by != 0 && by != other && *other != 0 => {
            return None;
        }
        _ => {}
    }
    Some(Modification::ModifyPT {
        power: per(power)?,
        toughness: per(toughness)?,
    })
}

/// "creature card exiled this way": the cards of a kind that an earlier exile bound by a name.
fn exiled_this_way(selector: &Selector) -> Option<String> {
    let (mut bound, mut exiled) = (false, false);
    let mut kinds = Vec::new();
    for pred in &selector.0 {
        match pred {
            Pred::Is(Ref::Bound(_)) if !bound => bound = true,
            Pred::In(ZoneKind::Exile) if !exiled => exiled = true,
            other => kinds.push(other.clone()),
        }
    }
    if !(bound && exiled) {
        return None;
    }
    let kind = if kinds.is_empty() {
        String::new()
    } else {
        format!("{} ", phrase_text(&Selector(kinds), Form::Count)?)
    };
    Some(format!("{kind}card exiled this way"))
}

/// What "for each" counts, as the card words it after "for each ".
pub fn for_each_text(counted: &Amount) -> Option<String> {
    Some(match counted {
        Amount::Count(selector) if exiled_this_way(selector).is_some() => {
            exiled_this_way(selector)?
        }
        Amount::Count(selector) => {
            let phrase = phrase_text(selector, Form::Count)?;
            if selector.0.iter().any(holds) {
                phrase
            } else {
                format!("{phrase} on the battlefield")
            }
        }
        Amount::Players(Ref::Opponents) => "opponent".to_owned(),
        Amount::Players(Ref::Bound(_)) => super::vote::w2d_tally_text(counted, false)?,
        Amount::Party(Ref::You) => "creature in your party".to_owned(),
        Amount::Devoured => "creature it devoured".to_owned(),
        Amount::CommanderCasts(Ref::You) => {
            "time you've cast your commander from the command zone this game".to_owned()
        }
        Amount::Stat {
            of: Ref::Triggering,
            stat: StatKind::Colors,
        } => "of that spell's colors".to_owned(),
        Amount::Counters {
            on: Ref::This,
            counter,
        } => format!("{} counter on ~", counters::kind_text(counter)?),
        Amount::AnyCounters(Ref::This) => "counter on ~".to_owned(),
        Amount::Tallied { .. } => defining::died_text(counted, true)?,
        Amount::Destroyed(_) => super::w2c_counts::w2c_destroyed_each_text(counted)?,
        Amount::PhasedOut(_) => super::w11b_phase::w11b_phased_each_text(counted)?,
        Amount::Distinct { .. } => super::w2c_counts::w2c_distinct_text(counted, false)?,
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
        // "X plus 2 damage": the announced X and a fixed number more.
        Amount::Sum(parts) => match parts.as_slice() {
            [Amount::X, Amount::Fixed(more)] if *more > 0 => Some(format!("X plus {more}")),
            _ => None,
        },
        _ => None,
    }
}
