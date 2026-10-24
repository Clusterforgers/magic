//! A search for two cards in one sentence: "a Forest card and a Plains card" and "up to two basic land cards that share
//! a land type" read into two searches in a row, the second of which leaves out the card the first found, and write
//! back as the same text.

use mtg_core::card::{CardType, Supertype};

use super::library::{filter_text, what, what_text};
use super::tokens::Reader;
use crate::effect::Effect;
use crate::refs::{Bounds, Pred, Ref, Selector, ZoneKind};
use crate::value::Amount;

/// The second search of a pair: its filter and how many cards it finds.
pub(super) type Second = (Selector, Bounds);

/// The name the second card of a pair is bound to, after the first.
pub(super) fn second_bind(first: &str) -> String {
    format!("{first}2")
}

/// "Up to one" card: each search of a pair that shares a type finds at most one.
fn up_to_one() -> Bounds {
    Bounds::up_to(Some(Amount::one()))
}

/// What the second search leaves out: the card the first found.
fn leaves_out(first: &str) -> Pred {
    Pred::Not(Box::new(Pred::Is(Ref::Bound(first.to_owned()))))
}

/// The basic land cards a search for two of one land type looks through.
fn basic_lands() -> Vec<Pred> {
    vec![
        Pred::Supertype(Supertype::Basic),
        Pred::Type(CardType::Land),
    ]
}

/// The rest of a search for two cards, read after the filter and the count of the first: "and a Plains card" after
/// "a Forest card", or "that share a land type" after "up to two basic land cards". The count of the first search
/// and the second search come back.
pub(super) fn read(
    r: &mut Reader<'_>,
    filter: &Selector,
    count: &Bounds,
    first: &str,
) -> Option<(Bounds, Second)> {
    if count.is_one() {
        r.word("and").then_some(())?;
        let (other, bounds) = what(r)?;
        bounds.is_one().then_some(())?;
        let mut preds = other.0;
        preds.push(leaves_out(first));
        return Some((count.clone(), (Selector(preds), Bounds::default())));
    }
    let two = Bounds::up_to(Some(Amount::Fixed(2)));
    (*count == two && filter.0 == basic_lands()).then_some(())?;
    r.words("that share a land type").then_some(())?;
    let mut preds = filter.0.clone();
    preds.push(Pred::SharesSubtype(Ref::Bound(first.to_owned())));
    preds.push(leaves_out(first));
    Some((up_to_one(), (Selector(preds), up_to_one())))
}

/// The searches of a library: one, or the two of a pair, and the names of what they find in order.
pub(super) fn searches(
    who: &Ref,
    first: (Selector, Bounds, String),
    second: Option<Second>,
    reveal: bool,
) -> (Vec<Effect>, Vec<Ref>) {
    let search = |filter, count, bind: String| Effect::Search {
        who: who.clone(),
        zone: ZoneKind::Library,
        filter,
        count,
        reveal,
        bind,
    };
    let (filter, count, bind) = first;
    let mut found = vec![Ref::Bound(bind.clone())];
    let mut made = Vec::new();
    if let Some((other, other_count)) = second {
        let other_bind = second_bind(&bind);
        found.push(Ref::Bound(other_bind.clone()));
        made.push(search(filter, count, bind));
        made.push(search(other, other_count, other_bind));
    } else {
        made.push(search(filter, count, bind));
    }
    (made, found)
}

/// What a pair of searches looks for as a card words it: "a Forest card and a Plains card" or "up to two basic land
/// cards that share a land type".
pub(super) fn text(
    first: (&Selector, &Bounds, &str),
    second: (&Selector, &Bounds, &str),
) -> Option<String> {
    let ((filter, count, bind), (other, other_count, other_bind)) = (first, second);
    if other_bind != second_bind(bind) {
        return None;
    }
    let (kept, last) = other.0.split_at(other.0.len().checked_sub(1)?);
    if last != [leaves_out(bind)] {
        return None;
    }
    if count.is_one() && other_count.is_one() {
        let second = what_text(&Selector(kept.to_vec()), other_count)?;
        return Some(format!("{} and {second}", what_text(filter, count)?));
    }
    let (kept, shared) = kept.split_at(kept.len().checked_sub(1)?);
    let alike = shared == [Pred::SharesSubtype(Ref::Bound(bind.to_owned()))];
    (alike && kept == filter.0 && filter.0 == basic_lands())
        .then_some(())
        .filter(|()| *count == up_to_one() && *other_count == up_to_one())?;
    Some(format!(
        "up to two {} that share a land type",
        filter_text(filter, true)?
    ))
}
