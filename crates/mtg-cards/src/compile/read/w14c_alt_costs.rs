//! Alternative costs of a spell (CR 118.9): "You may sacrifice a Mountain rather than pay this spell's mana cost.", "If
//! you control a Swamp, you may pay 4 life rather than pay this spell's mana cost."

use super::Reader;
use super::costs;
use super::statics::{control, control_texts};
use crate::ability::Ability;
use crate::cost::{Cost, CostItem};

/// "[If you control ..., ]you may <cost> rather than pay ~'s mana cost.": a cast for the cost in place of the mana cost,
/// open while the condition holds.
pub(super) fn w14c_alt_cost(r: &mut Reader<'_>) -> Option<Ability> {
    let condition = if r.word("if") {
        let held = control(r)?;
        r.punct(',').then_some(())?;
        Some(held)
    } else {
        None
    };
    r.words("you may").then_some(())?;
    let cost = w14c_paid(r)?;
    r.words("rather than pay").then_some(())?;
    r.source().then_some(())?;
    r.punct('\'').then_some(())?;
    r.word("s").then_some(())?;
    r.words("mana cost").then_some(())?;
    Some(Ability::FreeCast {
        condition,
        cost: Some(cost),
    })
}

/// The cost: "pay {W}", or one part ("pay 4 life", "sacrifice a Mountain", "return two Islands you control to their
/// owner's hand").
fn w14c_paid(r: &mut Reader<'_>) -> Option<Cost> {
    let paid = r.attempt(|r| {
        r.word("pay").then_some(())?;
        super::costs::mana(r).map(Cost::mana)
    });
    let cost = paid.or_else(|| costs::cost(r))?;
    (!cost.has_x() && !cost.0.is_empty()).then_some(cost)
}

/// The sentences of an alternative cost the reader reads, without the period.
pub(super) fn w14c_alt_cost_texts(ability: &Ability) -> Vec<String> {
    let Ability::FreeCast {
        condition,
        cost: Some(cost),
    } = ability
    else {
        return Vec::new();
    };
    let body = if let [CostItem::Mana(mana)] = cost.0.as_slice() {
        format!("pay {mana}")
    } else {
        let Some(text) = costs::text(cost) else {
            return Vec::new();
        };
        costs::lowered(&text)
    };
    let sentence = format!("you may {body} rather than pay ~'s mana cost");
    match condition {
        None => vec![sentence],
        Some(held) => control_texts(held)
            .into_iter()
            .map(|held| format!("if {held}, {sentence}"))
            .collect(),
    }
}
