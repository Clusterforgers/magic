//! The mana spent to cast a spell (CR 601.2h): "if {B} was spent to cast it", "if {R} was spent to cast this spell" and
//! "Sacrifice it unless {G} was spent to cast it." read as `Condition::ManaSpent`, the last as a sacrifice that the
//! condition's absence causes.

use super::effects;
use super::tokens::Reader;
use crate::effect::Effect;
use crate::refs::{Pred, Ref};
use crate::value::{Amount, Condition};
use mtg_core::mana::ManaType;

/// The mana a symbol names: `{G}` is green, `{C}` colorless.
fn mana_of(symbol: &str) -> Option<ManaType> {
    let mut letters = symbol.chars();
    let (letter, rest) = (letters.next()?, letters.next());
    if rest.is_some() {
        return None;
    }
    ManaType::ALL
        .into_iter()
        .find(|mana| mana.symbol() == letter)
}

/// "{G} was spent to cast it", or "{G} was spent to cast this spell" with `spell`.
pub fn was_spent(r: &mut Reader<'_>, spell: bool) -> Option<Condition> {
    let mana = mana_of(r.symbol()?)?;
    r.need("was spent to cast")?;
    if spell {
        r.source().then_some(())?;
    } else {
        r.word("it").then_some(())?;
    }
    Some(Condition::ManaSpent(mana))
}

/// The condition `was_spent` reads, written back for the same `spell`.
pub fn was_spent_text(condition: &Condition, spell: bool) -> Option<String> {
    let Condition::ManaSpent(mana) = condition else {
        return None;
    };
    Some(format!(
        "{{{}}} was spent to cast {}",
        mana.symbol(),
        if spell { "this spell" } else { "it" }
    ))
}

/// "Sacrifice it unless {G} was spent to cast it.": the card is sacrificed when that mana was not spent.
pub fn sacrifice_unless(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let sacrifice = effects::sacrifice_itself(r)?;
    r.need("unless")?;
    let spent = was_spent(r, false)?;
    Some(vec![Effect::If {
        condition: Condition::Not(Box::new(spent)),
        then: vec![sacrifice],
        otherwise: Vec::new(),
    }])
}

/// The sentence of an `If` that sacrifices the card unless mana was spent, as `sacrifice_unless` reads it.
pub fn unless_text(effect: &Effect) -> Option<String> {
    let Effect::If {
        condition: Condition::Not(spent),
        then,
        otherwise,
    } = effect
    else {
        return None;
    };
    let [
        Effect::Sacrifice {
            who: Ref::You,
            filter,
            count,
        },
    ] = then.as_slice()
    else {
        return None;
    };
    if !otherwise.is_empty() || filter.0 != [Pred::This] || *count != Amount::one() {
        return None;
    }
    Some(format!(
        "sacrifice ~ unless {}",
        was_spent_text(spent, false)?
    ))
}
