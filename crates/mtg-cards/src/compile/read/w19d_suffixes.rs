//! Suffixes of a noun phrase about combat and damage: "creature blocking it", "creature it's blocking", "creature blocking
//! or blocked by it", "creature that was dealt damage this turn", "creature that's attacking you". The words follow the
//! noun, read and written like the other suffixes (`selectors::suffix`).

use super::selectors::Form;
use super::tokens::Reader;
use crate::refs::{Pred, Ref};

/// The suffix at the reader, when it is one of the above.
pub(super) fn w19d_suffix(r: &mut Reader<'_>) -> Option<Pred> {
    if r.words("blocking or blocked by it") {
        return Some(Pred::AnyOf(vec![
            Pred::Blocks(Ref::This),
            Pred::BlockedBy(Ref::This),
        ]));
    }
    if r.words("blocking it") {
        return Some(Pred::Blocks(Ref::This));
    }
    if r.words("it's blocking") {
        return Some(Pred::BlockedBy(Ref::This));
    }
    if r.words("that was dealt damage this turn") || r.words("that were dealt damage this turn") {
        return Some(Pred::DamagedThisTurn);
    }
    if r.words("that's attacking you") || r.words("that are attacking you") {
        return Some(Pred::AttackingPlayer(Ref::You));
    }
    None
}

/// The words of one of the suffixes above; `None` for any other predicate.
pub(super) fn w19d_suffix_text(pred: &Pred, form: Form) -> Option<String> {
    Some(match pred {
        Pred::Blocks(Ref::This) => "blocking it".to_owned(),
        Pred::BlockedBy(Ref::This) => "it's blocking".to_owned(),
        Pred::AnyOf(any) if *any == [Pred::Blocks(Ref::This), Pred::BlockedBy(Ref::This)] => {
            "blocking or blocked by it".to_owned()
        }
        Pred::DamagedThisTurn if form.plural() => "that were dealt damage this turn".to_owned(),
        Pred::DamagedThisTurn => "that was dealt damage this turn".to_owned(),
        Pred::AttackingPlayer(Ref::You) if form.plural() => "that are attacking you".to_owned(),
        Pred::AttackingPlayer(Ref::You) => "that's attacking you".to_owned(),
        _ => return None,
    })
}
