//! Exert as a creature attacks (CR 701.43): "You may exert ~ as it attacks." and "If ~ hasn't been exerted this turn,
//! you may exert it as it attacks." are the keyword `Exert`; the reflexive "When you do, ..." after it is a trigger on the
//! card being exerted (`compile/clauses.rs` splits the two sentences).

use mtg_core::card::CardType;

use super::tokens::Reader;
use crate::ability::Ability;
use crate::keyword::Keyword;
use crate::refs::{Pred, Ref, Selector};
use crate::trigger::TriggerPattern;

/// The pronouns a card calls itself by as it attacks.
const W9D_PRONOUNS: [&str; 3] = ["it", "he", "she"];

/// "You may exert ~ as it attacks" and "If ~ hasn't been exerted this turn, you may exert it as it attacks".
pub(super) fn w9d_exert(r: &mut Reader<'_>) -> Option<Ability> {
    let once_per_turn = r
        .attempt(|r| {
            r.need("if")?;
            r.source().then_some(())?;
            r.need("hasn't been exerted this turn")?;
            r.punct(',').then_some(())?;
            r.need("you may exert it as it attacks")
        })
        .is_some();
    if !once_per_turn {
        r.need("you may exert")?;
        r.source().then_some(())?;
        r.need("as")?;
        W9D_PRONOUNS
            .iter()
            .any(|pronoun| r.word(pronoun))
            .then_some(())?;
        r.need("attacks")?;
    }
    Some(Ability::Keyword(Keyword::Exert { once_per_turn }))
}

/// The wordings of the keyword, without the period; none for another ability.
pub(super) fn w9d_variants(ability: &Ability) -> Option<Vec<String>> {
    let Ability::Keyword(Keyword::Exert { once_per_turn }) = ability else {
        return None;
    };
    Some(if *once_per_turn {
        vec!["If ~ hasn't been exerted this turn, you may exert it as it attacks".to_owned()]
    } else {
        W9D_PRONOUNS
            .iter()
            .map(|pronoun| format!("You may exert ~ as {pronoun} attacks"))
            .collect()
    })
}

/// The creatures "Whenever you exert a creature" names.
fn w9d_yours() -> Selector {
    Selector(vec![
        Pred::Type(CardType::Creature),
        Pred::ControlledBy(Ref::You),
    ])
}

/// After "When" or "Whenever": "you do" (the reflexive trigger of the card's own exert) and "you exert a creature".
pub(super) fn w9d_event(r: &mut Reader<'_>) -> Option<TriggerPattern> {
    r.need("you")?;
    if r.word("do") {
        return Some(TriggerPattern::Exerted {
            what: Selector(vec![Pred::This]),
        });
    }
    r.need("exert a creature")?;
    Some(TriggerPattern::Exerted { what: w9d_yours() })
}

/// The event written back: "When you do", "Whenever you exert a creature".
pub(super) fn w9d_event_text(what: &Selector) -> Option<String> {
    if what.0 == [Pred::This] {
        Some("When you do".to_owned())
    } else {
        (*what == w9d_yours()).then(|| "Whenever you exert a creature".to_owned())
    }
}
