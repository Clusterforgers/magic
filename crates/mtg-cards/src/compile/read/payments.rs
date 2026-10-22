//! Payments in an effect: "you may pay {1}. If you do, draw a card." (`Pay` with `paid` effects) and "... unless that
//! player pays {2}", "... unless that player pays 3 life", "... unless you discard a card" (`Pay` with `unpaid` effects).
//! The cost is one part: mana, life, energy, a discard or, after "you may", a sacrifice. Read from the wording and
//! written back.

use super::amounts::{cards, cards_text, points, points_text};
use super::costs;
use super::effects::{mana_cost, one_sentence};
use super::tokens::Reader;
use crate::cost::{Cost, CostItem};
use crate::effect::Effect;
use crate::refs::Ref;
use crate::value::Amount;

/// "You may pay {1}. If you do, draw a card.", "You may pay 2 life. If you do, ...", "You may sacrifice a creature. If
/// you do, ...": the player pays or not, and the sentence after "if you do" follows a payment.
pub fn may_pay(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("you may")?;
    let cost = if r.word("pay") {
        after_pay(r)?
    } else {
        let cost = costs::cost(r)?;
        // A payment the player announces (an X) is not asked in the middle of a resolution.
        matches!(
            cost.0.as_slice(),
            [CostItem::Discard {
                count: Amount::Fixed(_),
                filter: None
            } | CostItem::Sacrifice {
                count: Amount::Fixed(_),
                ..
            }]
        )
        .then_some(cost)?
    };
    r.sentence_end().then_some(())?;
    r.need("if you do")?;
    r.punct(',').then_some(())?;
    let paid = one_sentence(r)?;
    Some(vec![Effect::Pay {
        who: Ref::You,
        cost,
        paid,
        unpaid: Vec::new(),
    }])
}

/// What follows "unless": who pays and what, for the sentence before it (`unpaid`).
pub fn unless(r: &mut Reader<'_>, unpaid: &[Effect]) -> Option<(Ref, Cost)> {
    let who = if r.words("its controller") {
        // The controller of a countered spell: the only object a sentence names.
        let countered = matches!(
            unpaid,
            [Effect::Counter {
                what: Ref::Target(0)
            }]
        );
        countered.then(|| Ref::ControllerOf(Box::new(Ref::Target(0))))?
    } else if r.that_player && r.words("that player") {
        Ref::Triggering
    } else if r.words("you") {
        // A player who pays so that a counter or a choice they made does not happen is not one the table reads.
        let nonsense = unpaid
            .iter()
            .any(|effect| matches!(effect, Effect::Counter { .. } | Effect::May { .. }));
        (!nonsense).then_some(Ref::You)?
    } else {
        return None;
    };
    let third = who != Ref::You;
    let cost = if r.word(if third { "pays" } else { "pay" }) {
        after_pay(r)?
    } else if r.word(if third { "discards" } else { "discard" }) {
        Cost(vec![CostItem::Discard {
            count: cards(r)?,
            filter: None,
        }])
    } else {
        return None;
    };
    Some((who, cost))
}

/// The cost after "pay": `{2}`, `3 life`, `{E}{E}`.
fn after_pay(r: &mut Reader<'_>) -> Option<Cost> {
    if let Some(mana) = r.attempt(mana_cost) {
        return Some(mana);
    }
    if let Some(energy) = r.attempt(|r| {
        let before = std::mem::replace(&mut r.x_free, false);
        let energy = costs::energy(r);
        r.x_free = before;
        energy
    }) {
        return Some(Cost(vec![CostItem::PayEnergy(energy)]));
    }
    let amount = points(r)?;
    // A number of life that is X or what a trigger counted is not the payer's to choose.
    matches!(amount, Amount::Fixed(_)).then_some(())?;
    r.word("life").then_some(())?;
    Some(Cost(vec![CostItem::PayLife(amount)]))
}

/// The one part of a payment, as the verb phrase of the player who pays it: "pay {1}", "pay 2 life", "discard a card".
fn phrase(cost: &Cost) -> Option<String> {
    let [item] = cost.0.as_slice() else {
        return None;
    };
    match item {
        CostItem::Mana(mana) if !mana.has_x() => Some(format!("pay {mana}")),
        CostItem::PayLife(amount @ Amount::Fixed(_)) => {
            Some(format!("pay {} life", points_text(amount)?))
        }
        CostItem::Discard {
            count: count @ Amount::Fixed(_),
            filter: None,
        } => Some(format!("discard {}", cards_text(count)?)),
        CostItem::PayEnergy(Amount::Fixed(_))
        | CostItem::Sacrifice {
            count: Amount::Fixed(_),
            ..
        } => lower(&costs::text(cost)?),
        _ => None,
    }
}

fn lower(text: &str) -> Option<String> {
    let mut chars = text.chars();
    let first = chars.next()?;
    Some(format!("{}{}", first.to_lowercase(), chars.as_str()))
}

/// "you may pay {1}. If you do, draw a card": the payment and the sentence that follows it.
pub fn may_pay_text(cost: &Cost, paid: &str) -> Option<String> {
    Some(format!("you may {}. If you do, {paid}", phrase(cost)?))
}

/// "unless that player pays {2}", "unless you discard a card": what follows the sentence that happens without payment.
pub fn unless_text(payer: &str, who: &Ref, cost: &Cost) -> Option<String> {
    let phrase = phrase(cost)?;
    // A sacrifice is read only after "you may".
    (!phrase.starts_with("sacrifice")).then_some(())?;
    let verb = if *who == Ref::You {
        phrase
    } else {
        let (verb, rest) = phrase.split_once(' ')?;
        format!("{verb}s {rest}")
    };
    Some(format!("unless {payer} {verb}"))
}
