//! Counters: putting them on an object, removing them from one and moving them from one to
//! another, read from "put a +1/+1 counter on ~", "remove two charge counters from target
//! permanent" and "move a +1/+1 counter from target creature onto ~" and written back the same way.

use super::amounts::{self, count_word, number_word};
use super::defining;
use super::effects::{attached, target_object};
use super::selectors::{each, target};
use super::tokens::Reader;
use super::triggers::article;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{Ref, Targetable};
use crate::value::{Amount, Cmp, Condition, CounterKind};

/// The counters whose effect the rules of the game give and the engine does not play (CR 122.1: exalted and decayed
/// counters, stun, shield, finality; the lore counters of Sagas and the defense counters of Battles); the data holds a
/// counter as a name only, so a clause that puts, removes or moves one stays with the table. The other keyword counters
/// are read: the layers give their keyword (`CounterKind::keyword`).
const RULES_COUNTERS: [&str; 7] = [
    "decayed", "exalted", "finality", "shield", "stun", "defense", "lore",
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
    // The two keyword counters of two words.
    for name in ["first strike", "double strike"] {
        let found = r.attempt(|r| {
            r.words(name).then_some(())?;
            matches!(r.peek_word(), Some("counter" | "counters")).then_some(())
        });
        if found.is_some() {
            return Some(CounterKind::Other(name.to_owned()));
        }
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
    if let Some(any) = r.attempt(|r| c10_any_on_source(r, "counter on")) {
        return Some(any);
    }
    let counter = kind(r)?;
    r.need("counter on")?;
    (r.source() || (r.it == Some(Ref::This) && r.word("it"))).then_some(Amount::Counters {
        on: Ref::This,
        counter,
    })
}

/// "counter on ~", "counters on it": the counters of every kind on the card (CR 122.1).
fn c10_any_on_source(r: &mut Reader<'_>, words: &str) -> Option<Amount> {
    r.need(words)?;
    (r.source() || (r.it == Some(Ref::This) && r.word("it")))
        .then_some(Amount::AnyCounters(Ref::This))
}

/// "charge counters on ~", "+1/+1 counters on this creature": the counters on the card, as "the number of" counts them.
pub(super) fn counted_plural(r: &mut Reader<'_>) -> Option<Amount> {
    if let Some(any) = r.attempt(|r| c10_any_on_source(r, "counters on")) {
        return Some(any);
    }
    let counter = kind(r)?;
    r.need("counters on")?;
    (r.source() || (r.it == Some(Ref::This) && r.word("it"))).then_some(Amount::Counters {
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

/// "an additional +1/+1 counter", "two additional charge counters": the counters a permanent enters with besides any
/// others.
pub fn additional(r: &mut Reader<'_>) -> Option<(CounterKind, i32)> {
    let number = if r.word("an") {
        1
    } else {
        number_word(r.take_word()?)?
    };
    r.need("additional")?;
    let counter = kind(r)?;
    r.word(if number == 1 { "counter" } else { "counters" })
        .then_some(())?;
    Some((counter, i32::try_from(number).ok().filter(|n| *n > 0)?))
}

/// The counters `additional` reads, written back.
pub fn additional_text(counter: &CounterKind, number: i32) -> Option<String> {
    let kind = kind_text(counter)?;
    Some(match number {
        1 => format!("an additional {kind} counter"),
        many => format!(
            "{} additional {kind} counters",
            count_word(u32::try_from(many).ok().filter(|n| *n > 1)?)?
        ),
    })
}

/// The object counters are on, leave or go onto: "~" or one target.
fn place(r: &mut Reader<'_>) -> Option<Ref> {
    if r.source() || (r.it == Some(Ref::This) && r.word("it")) {
        Some(Ref::This)
    } else if r.it == Some(Ref::Triggering) && r.word("it") {
        Some(Ref::Triggering)
    } else if r.it == Some(Ref::Dealer) && r.word("it") {
        Some(Ref::Dealer)
    } else if matches!(r.it, Some(Ref::Bound(_)))
        && (r.word("it") || r.words("that token") || r.words("each of them"))
    {
        // The tokens the sentence before created.
        r.it.clone()
    } else if let Some(host) = attached(r) {
        Some(host)
    } else {
        target_object(r)
    }
}

/// "each of up to two target creatures", "each of two target creatures": several target objects that each get the
/// counters.
fn w2b_each_of_targets(r: &mut Reader<'_>) -> Option<Ref> {
    r.need("each of")?;
    let what = target(r)?;
    let spec = r.targets.first()?;
    (matches!(spec.what, Targetable::Object(_)) && !spec.bounds.is_one()).then_some(what)
}

/// "Put a +1/+1 counter on ~.", "Put two -1/-1 counters on target creature.", "Put a +1/+1 counter
/// on each creature you control."
pub fn put(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("put")?;
    // "a number of +1/+1 counters on ~ equal to the number of creatures you control": the count follows the object.
    let equal = r.words("a number of");
    let (counter, count) = if r.ability && r.triggered && r.words("that many") {
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
        None => r.attempt(w2b_each_of_targets).or_else(|| place(r))?,
    };
    // The card the counters went on is what a later "it" means ("Then if there are five or more counters on it").
    if on == Ref::This {
        r.it = Some(Ref::This);
    }
    // The one target the counters went on is what the next sentence's "it" and "that creature" mean.
    if on == Ref::Target(0) && r.targets.first().is_some_and(|spec| spec.bounds.is_one()) {
        r.it = Some(on.clone());
    }
    let count = if equal {
        defining::equal_to(r)?
    } else {
        // "Put a +1/+1 counter on target creature for each Elf you control."
        amounts::counted(r, count)?
    };
    Some(vec![Effect::PutCounters { on, counter, count }])
}

/// "Remove a +1/+1 counter from ~.", "Remove two charge counters from target permanent.", "Remove up to three counters
/// from target permanent."
pub fn remove(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("remove")?;
    let up_to = r.words("up to");
    let (counter, count) = super::w2f_any_counters::w2f_counters(r)?;
    // A count the controller announces is no effect's count; "a counter" alone is the one "up to" never prints.
    (count != Amount::X && !(up_to && count == Amount::one())).then_some(())?;
    r.need("from")?;
    let from = place(r)?;
    Some(vec![Effect::RemoveCounters {
        from,
        counter,
        count,
        up_to,
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

/// "Double the number of +1/+1 counters on ~.", "Triple the number of each kind of counter on target creature.", "Double
/// the number of each kind of counter you have.": the counters one object or player holds are multiplied.
pub fn multiply(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let factor = if r.word("double") {
        2
    } else {
        r.need("triple")?;
        3
    };
    r.need("the number of")?;
    let counter = if r.words("each kind of counter") {
        if r.words("you have") {
            return Some(vec![Effect::MultiplyCounters {
                on: Ref::You,
                counter: None,
                factor,
            }]);
        }
        None
    } else {
        let counter = kind(r)?;
        r.need("counters")?;
        Some(counter)
    };
    r.need("on")?;
    let on = match each(r) {
        Some(objects) => objects,
        None => place(r)?,
    };
    if on == Ref::This {
        r.it = Some(Ref::This);
    }
    Some(vec![Effect::MultiplyCounters {
        on,
        counter,
        factor,
    }])
}

/// The words for a kind of counter.
pub(super) fn kind_text(counter: &CounterKind) -> Option<&str> {
    match counter {
        // A keyword counter may have two words: "double strike", "first strike".
        CounterKind::Other(name) if !name.chars().all(|c| c.is_ascii_lowercase() || c == ' ') => {
            None
        }
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
