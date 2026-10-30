//! Object phrases that compare with another object or read the object's own state: "with the same name as that
//! creature", "that shares a color with it", "that shares a card type with it", "with {X} in its mana cost" and
//! "that's enchanted by an Aura you control". "It" and "that creature" are the target of a spell or an activated
//! ability and the object of the event in a triggered one.

use mtg_core::card::Subtype;

use super::selectors::Form;
use super::tokens::Reader;
use crate::refs::{Pred, Ref, Selector};

/// The object "it" and "that creature" name where the phrase is read.
fn w6d_it(r: &Reader<'_>) -> Ref {
    if r.triggered {
        Ref::Triggering
    } else {
        Ref::Target(0)
    }
}

/// "that" or "that's"/"that are" before a verb phrase.
fn w6d_that_is(r: &mut Reader<'_>, form: Form) -> Option<()> {
    r.need("that")?;
    if form.plural() {
        return r.word("are").then_some(());
    }
    (r.punct('\'') && r.word("s")).then_some(())
}

/// An Aura, an Aura you control or an Equipment attached to the object.
fn w6d_attached(kind: &str, yours: bool) -> Pred {
    let mut held = vec![Pred::Subtype(Subtype(kind.to_owned()))];
    if yours {
        held.push(Pred::ControlledBy(Ref::You));
    }
    Pred::HasAttached(Selector(held))
}

/// "enchanted or equipped".
fn w6d_enchanted_or_equipped() -> Pred {
    Pred::AnyOf(vec![
        w6d_attached("Aura", false),
        w6d_attached("Equipment", false),
    ])
}

/// A phrase after the noun that compares the object with another or reads its own state.
pub(super) fn w6d_suffix(r: &mut Reader<'_>, form: Form) -> Option<Pred> {
    if let Some(pred) = r.attempt(|r| w6d_enchanted_suffix(r, form)) {
        return Some(pred);
    }
    if r.words("with the same name as") {
        return (r.words("that creature") || r.words("that permanent") || r.word("it"))
            .then(|| Pred::SharesName(w6d_it(r)));
    }
    if r.words("with") && r.symbol() == Some("X") {
        let tail = if form.plural() {
            "in their mana costs"
        } else {
            "in its mana cost"
        };
        return r.words(tail).then_some(Pred::XInCost);
    }
    if r.word("that") {
        let verb = if form.plural() { "share" } else { "shares" };
        if r.words(&format!("{verb} a color with it")) {
            return Some(Pred::SharesColor(w6d_it(r)));
        }
        if r.words(&format!("{verb} a card type with it")) {
            return Some(Pred::SharesType(w6d_it(r)));
        }
        return None;
    }
    None
}

/// "that isn't enchanted" and "that aren't enchanted": no Aura is attached to the object.
fn w6d_not_enchanted(r: &mut Reader<'_>, form: Form) -> Option<Pred> {
    r.need("that")?;
    r.need(if form.plural() { "aren't" } else { "isn't" })?;
    r.need("enchanted")?;
    Some(Pred::Not(Box::new(w6d_attached("Aura", false))))
}

/// The object phrase "that's enchanted ..." read from the start of `that`.
fn w6d_enchanted_suffix(r: &mut Reader<'_>, form: Form) -> Option<Pred> {
    if let Some(pred) = r.attempt(|r| w6d_not_enchanted(r, form)) {
        return Some(pred);
    }
    w6d_that_is(r, form)?;
    if r.words("enchanted or equipped") {
        return Some(w6d_enchanted_or_equipped());
    }
    r.need("enchanted by an aura you control")?;
    Some(w6d_attached("Aura", true))
}

/// The words of a predicate of this module after the noun.
pub(super) fn w6d_suffix_text(pred: &Pred, form: Form) -> Option<String> {
    let it = |who: &Ref| match who {
        Ref::Target(0) | Ref::Triggering => Some("it"),
        _ => None,
    };
    let verb = if form.plural() { "share" } else { "shares" };
    let that_is = if form.plural() { "that are" } else { "that's" };
    Some(match pred {
        Pred::XInCost if form.plural() => "with {X} in their mana costs".to_owned(),
        Pred::XInCost => "with {X} in its mana cost".to_owned(),
        Pred::SharesName(Ref::Target(0) | Ref::Triggering) => {
            "with the same name as that creature".to_owned()
        }
        Pred::SharesColor(who) => format!("that {verb} a color with {}", it(who)?),
        Pred::SharesType(who) => format!("that {verb} a card type with {}", it(who)?),
        pred if *pred == w6d_attached("Aura", true) => {
            format!("{that_is} enchanted by an Aura you control")
        }
        Pred::Not(inner) if **inner == w6d_attached("Aura", false) => {
            let not = if form.plural() { "aren't" } else { "isn't" };
            format!("that {not} enchanted")
        }
        pred if *pred == w6d_enchanted_or_equipped() => {
            format!("{that_is} enchanted or equipped")
        }
        _ => return None,
    })
}
