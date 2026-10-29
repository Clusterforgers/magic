//! The spell of a cast trigger that is a creature of a subtype or an artifact creature: "Whenever you cast a Phyrexian
//! creature or artifact creature spell" (Brimaz, Blight of Oreskos), read as a creature that has the subtype or is an
//! artifact and written back the same way.

use super::tokens::Reader;
use crate::refs::{Pred, Selector};
use mtg_core::card::{CardType, Subtype};

/// "a Phyrexian creature or artifact creature spell": the kind of the spell, without `Spell`.
pub(super) fn w14b_read(r: &mut Reader<'_>) -> Option<Vec<Pred>> {
    r.attempt(|r| {
        r.word("a").then_some(())?;
        let name = r.name().map(str::to_owned).or_else(|| r.opening_name())?;
        r.is_subtype(&name).then_some(())?;
        r.need("creature or artifact creature spell")?;
        Some(vec![
            Pred::Type(CardType::Creature),
            Pred::AnyOf(vec![
                Pred::Subtype(Subtype(name)),
                Pred::Type(CardType::Artifact),
            ]),
        ])
    })
}

/// What `w14b_read` reads, written back; `None` for any other kind of spell.
pub(super) fn w14b_text(what: &Selector) -> Option<String> {
    let [Pred::Type(CardType::Creature), Pred::AnyOf(either)] = what.0.as_slice() else {
        return None;
    };
    let [Pred::Subtype(Subtype(name)), Pred::Type(CardType::Artifact)] = either.as_slice() else {
        return None;
    };
    Some(format!("a {name} creature or artifact creature spell"))
}
