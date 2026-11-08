//! "if you revealed a Dragon card or controlled a Dragon as you cast this spell" (Dragons of Tarkir): the optional reveal
//! of the spell paid (`AdditionalCostPaid`) or a permanent of the subtype yours as it was cast (`Condition::AsCast`).

use mtg_core::card::Subtype;

use super::tokens::Reader;
use crate::refs::{Pred, Ref, Selector};
use crate::value::{Amount, Cmp, Condition};

/// The condition for a subtype.
fn presence(kind: &str) -> Condition {
    let controlled = Selector(vec![
        Pred::Subtype(Subtype(kind.to_owned())),
        Pred::ControlledBy(Ref::You),
    ]);
    Condition::Any(vec![
        Condition::AdditionalCostPaid,
        Condition::AsCast(Box::new(Condition::Compare {
            left: Amount::Count(controlled),
            op: Cmp::Ge,
            right: Amount::Fixed(1),
        })),
    ])
}

/// The word as a subtype is spelled: its first letter in upper case, the rest in lower case.
fn spelled(word: &str) -> String {
    let mut letters = word.chars();
    letters.next().map_or_else(String::new, |first| {
        first
            .to_uppercase()
            .chain(letters.flat_map(char::to_lowercase))
            .collect()
    })
}

/// Reads "you revealed a Dragon card or controlled a Dragon as you cast this spell".
pub fn read(r: &mut Reader<'_>) -> Option<Condition> {
    r.need("you revealed a")?;
    let kind = spelled(r.take_word()?);
    r.need("card or controlled a")?;
    (spelled(r.take_word()?) == kind).then_some(())?;
    r.need("as you cast this spell")?;
    Some(presence(&kind))
}

/// The condition `read` reads, written back.
pub fn text(condition: &Condition) -> Option<String> {
    let Condition::Any(parts) = condition else {
        return None;
    };
    let [Condition::AdditionalCostPaid, Condition::AsCast(inner)] = parts.as_slice() else {
        return None;
    };
    let Condition::Compare {
        left: Amount::Count(Selector(preds)),
        op: Cmp::Ge,
        right: Amount::Fixed(1),
    } = &**inner
    else {
        return None;
    };
    let [Pred::Subtype(kind), Pred::ControlledBy(Ref::You)] = preds.as_slice() else {
        return None;
    };
    let kind = &kind.0;
    Some(format!(
        "you revealed a {kind} card or controlled a {kind} as you cast this spell"
    ))
}
