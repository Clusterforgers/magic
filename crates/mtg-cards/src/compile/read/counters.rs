//! Counters: putting them on an object, removing them from one and moving them from one to
//! another, read from "put a +1/+1 counter on ~", "remove two charge counters from target
//! permanent" and "move a +1/+1 counter from target creature onto ~" and written back the same way.

use super::amounts::{count_word, number_word};
use super::effects::target_object;
use super::selectors::each;
use super::tokens::Reader;
use crate::effect::Effect;
use crate::refs::Ref;
use crate::value::{Amount, CounterKind};

/// The counters whose effect the rules of the game give and not the card (CR 122.1: keyword
/// counters, stun, shield, finality; the lore counters of Sagas and the defense counters of Battles); the data holds a counter as a name only, so a clause that puts,
/// removes or moves one stays with the table.
const RULES_COUNTERS: [&str; 19] = [
    "decayed",
    "deathtouch",
    "exalted",
    "finality",
    "flying",
    "haste",
    "hexproof",
    "indestructible",
    "lifelink",
    "menace",
    "reach",
    "shadow",
    "shield",
    "skulk",
    "stun",
    "trample",
    "vigilance",
    "defense",
    "lore",
];

/// "+1/+1" or "-1/-1", the counters that change a creature's size, or a word that names another
/// kind: "charge", "poison", "time".
fn kind(r: &mut Reader<'_>) -> Option<CounterKind> {
    let size = r.attempt(|r| {
        let plus = r.punct('+');
        if !plus {
            r.punct('-').then_some(())?;
        }
        (r.number()? == 1).then_some(())?;
        r.punct('/').then_some(())?;
        r.punct(if plus { '+' } else { '-' }).then_some(())?;
        (r.number()? == 1).then_some(())?;
        Some(if plus {
            CounterKind::PlusOnePlusOne
        } else {
            CounterKind::MinusOneMinusOne
        })
    });
    if size.is_some() {
        return size;
    }
    let word = r.peek_word()?;
    let named = matches!(r.word_at(1), Some("counter" | "counters"))
        && word.chars().all(|c| c.is_ascii_lowercase());
    named.then_some(())?;
    (!RULES_COUNTERS.contains(&word)).then_some(())?;
    r.skip();
    Some(match word {
        "loyalty" => CounterKind::Loyalty,
        "poison" => CounterKind::Poison,
        "charge" => CounterKind::Charge,
        other => CounterKind::Other(other.to_owned()),
    })
}

/// How many counters of what kind: "a +1/+1 counter", "two charge counters".
fn counters(r: &mut Reader<'_>) -> Option<(CounterKind, u32)> {
    let number = number_word(r.take_word()?)?;
    let counter = kind(r)?;
    r.word(if number == 1 { "counter" } else { "counters" })
        .then_some(())?;
    Some((counter, number))
}

/// The object counters are on, leave or go onto: "~" or one target.
fn place(r: &mut Reader<'_>) -> Option<Ref> {
    if r.source() {
        Some(Ref::This)
    } else {
        target_object(r)
    }
}

/// "Put a +1/+1 counter on ~.", "Put two -1/-1 counters on target creature.", "Put a +1/+1 counter
/// on each creature you control."
pub fn put(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("put")?;
    let (counter, number) = counters(r)?;
    r.need("on")?;
    let on = match each(r) {
        Some(objects) => objects,
        None => place(r)?,
    };
    Some(vec![Effect::PutCounters {
        on,
        counter,
        count: Amount::Fixed(i32::try_from(number).ok()?),
    }])
}

/// "Remove a +1/+1 counter from ~.", "Remove two charge counters from target permanent."
pub fn remove(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("remove")?;
    let (counter, number) = counters(r)?;
    r.need("from")?;
    let from = place(r)?;
    Some(vec![Effect::RemoveCounters {
        from,
        counter,
        count: Amount::Fixed(i32::try_from(number).ok()?),
    }])
}

/// "Move a +1/+1 counter from target creature onto ~.", "Move a charge counter from ~ onto target
/// artifact." One of the two is the card itself.
pub fn moving(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("move")?;
    let (counter, number) = counters(r)?;
    r.need("from")?;
    let from = place(r)?;
    r.need("onto")?;
    let to = place(r)?;
    (from != to).then_some(())?;
    Some(vec![Effect::MoveCounters {
        from,
        to,
        counter,
        count: Amount::Fixed(i32::try_from(number).ok()?),
    }])
}

/// The words for a kind of counter.
fn kind_text(counter: &CounterKind) -> Option<&str> {
    match counter {
        CounterKind::Other(name) if !name.chars().all(|c| c.is_ascii_lowercase()) => None,
        CounterKind::Other(name) if name.is_empty() => None,
        other => Some(other.name()),
    }
}

/// "a +1/+1 counter", "two charge counters": a number of counters of one kind.
pub fn text(counter: &CounterKind, count: &Amount) -> Option<String> {
    let Amount::Fixed(number) = count else {
        return None;
    };
    let kind = kind_text(counter)?;
    Some(match u32::try_from(*number).ok()? {
        0 => return None,
        1 => format!("a {kind} counter"),
        many => format!("{} {kind} counters", count_word(many)?),
    })
}
