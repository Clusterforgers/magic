//! Counters: putting them on an object, removing them from one and moving them from one to
//! another, read from "put a +1/+1 counter on ~", "remove two charge counters from target
//! permanent" and "move a +1/+1 counter from target creature onto ~" and written back the same way.

use super::amounts::{self, count_word, number_word};
use super::defining;
use super::effects::{attached, target_object};
use super::selectors::each;
use super::tokens::Reader;
use super::triggers::article;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::Ref;
use crate::value::{Amount, Cmp, Condition, CounterKind};

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
pub(super) fn kind(r: &mut Reader<'_>) -> Option<CounterKind> {
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

/// "charge counter on ~", "+1/+1 counter on it": the counters on the card, as "for each" counts them.
pub(super) fn counted(r: &mut Reader<'_>) -> Option<Amount> {
    let counter = kind(r)?;
    r.need("counter on")?;
    (r.source() || (r.it == Some(Ref::This) && r.word("it"))).then_some(Amount::Counters {
        on: Ref::This,
        counter,
    })
}

/// "charge counters on ~", "+1/+1 counters on this creature": the counters on the card, as "the number of" counts them.
pub(super) fn counted_plural(r: &mut Reader<'_>) -> Option<Amount> {
    let counter = kind(r)?;
    r.need("counters on")?;
    r.source().then_some(Amount::Counters {
        on: Ref::This,
        counter,
    })
}

/// The wordings of a trigger whose intervening "if" says the card has no counters of a kind, with the card called "it"
/// in what follows ("..., return it to its owner's hand").
pub(super) fn pronouns_after_none(ability: &Ability, texts: &[String]) -> Vec<String> {
    let Ability::Triggered {
        intervening_if:
            Some(Condition::Compare {
                left: Amount::Counters { on: Ref::This, .. },
                op: Cmp::Eq,
                right: Amount::Fixed(0),
            }),
        ..
    } = ability
    else {
        return Vec::new();
    };
    texts
        .iter()
        .filter(|text| text.contains(", return ~ to its owner's hand"))
        .map(|text| text.replacen(", return ~ to", ", return it to", 1))
        .collect()
}

/// How many counters of what kind: "a +1/+1 counter", "two charge counters".
pub(super) fn counters(r: &mut Reader<'_>) -> Option<(CounterKind, u32)> {
    let number = number_word(r.take_word()?)?;
    let counter = kind(r)?;
    r.word(if number == 1 { "counter" } else { "counters" })
        .then_some(())?;
    Some((counter, number))
}

/// How many counters a permanent enters with: "a +1/+1 counter", "two charge counters", "X +1/+1
/// counters" (when the card's mana cost has an X).
pub fn entering(r: &mut Reader<'_>) -> Option<(CounterKind, Amount)> {
    let (count, number) = if r.x() {
        (Amount::X, None)
    } else {
        let number = number_word(r.take_word()?)?;
        (Amount::Fixed(i32::try_from(number).ok()?), Some(number))
    };
    let counter = kind(r)?;
    r.word(if number == Some(1) {
        "counter"
    } else {
        "counters"
    })
    .then_some(())?;
    Some((counter, count))
}

/// The object counters are on, leave or go onto: "~" or one target.
fn place(r: &mut Reader<'_>) -> Option<Ref> {
    if r.source() || (r.it == Some(Ref::This) && r.word("it")) {
        Some(Ref::This)
    } else if r.it == Some(Ref::Triggering) && r.word("it") {
        Some(Ref::Triggering)
    } else if let Some(host) = attached(r) {
        Some(host)
    } else {
        target_object(r)
    }
}

/// "Put a +1/+1 counter on ~.", "Put two -1/-1 counters on target creature.", "Put a +1/+1 counter
/// on each creature you control."
pub fn put(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("put")?;
    // "a number of +1/+1 counters on ~ equal to the number of creatures you control": the count follows the object.
    let equal = r.words("a number of");
    let (counter, count) = if r.words("that many") {
        let counter = kind(r)?;
        r.need("counters")?;
        (counter, Amount::Triggering)
    } else if equal {
        let counter = kind(r)?;
        r.need("counters")?;
        (counter, Amount::X)
    } else {
        entering(r)?
    };
    r.need("on")?;
    let on = match each(r) {
        Some(objects) => objects,
        None => place(r)?,
    };
    // The card the counters went on is what a later "it" means ("Then if there are five or more counters on it").
    if on == Ref::This {
        r.it = Some(Ref::This);
    }
    let count = if equal {
        defining::equal_to(r)?
    } else {
        // "Put a +1/+1 counter on target creature for each Elf you control."
        amounts::counted(r, count)?
    };
    Some(vec![Effect::PutCounters { on, counter, count }])
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
pub(super) fn kind_text(counter: &CounterKind) -> Option<&str> {
    match counter {
        CounterKind::Other(name) if !name.chars().all(|c| c.is_ascii_lowercase()) => None,
        CounterKind::Other(name) if name.is_empty() => None,
        other => Some(other.name()),
    }
}

/// The kind a trigger names after "a": "+1/+1 counter", "charge counter".
pub(super) fn trigger_kind(r: &mut Reader<'_>) -> Option<CounterKind> {
    let counter = kind(r)?;
    r.word("counter").then_some(counter)
}

/// "a +1/+1 counter", "a charge counter", "a counter": the counter a trigger names.
pub(super) fn trigger_kind_text(counter: Option<&CounterKind>) -> Option<String> {
    match counter {
        None => Some("a counter".to_owned()),
        Some(counter) => {
            let kind = kind_text(counter)?;
            Some(format!("{} {kind} counter", article(kind)))
        }
    }
}

/// The counters a permanent enters with, as `entering` reads them.
pub fn entering_text(counter: &CounterKind, count: &Amount) -> Option<String> {
    match count {
        Amount::X => Some(format!("X {} counters", kind_text(counter)?)),
        _ => text(counter, count),
    }
}

/// "a +1/+1 counter", "two charge counters": a number of counters of one kind.
pub fn text(counter: &CounterKind, count: &Amount) -> Option<String> {
    if *count == Amount::Triggering {
        return Some(format!("that many {} counters", kind_text(counter)?));
    }
    if *count == Amount::X {
        return Some(format!("X {} counters", kind_text(counter)?));
    }
    let Amount::Fixed(number) = count else {
        return None;
    };
    let kind = kind_text(counter)?;
    Some(match u32::try_from(*number).ok()? {
        0 => return None,
        1 => format!("{} {kind} counter", article(kind)),
        many => format!("{} {kind} counters", count_word(many)?),
    })
}
