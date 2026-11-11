//! "As an additional cost to cast this spell, you may sacrifice any number of creatures. This spell costs {2} less to cast
//! for each creature sacrificed this way.": a required cost of X things and the spell's own reduction for each of them, one
//! clause of two abilities in order.

use super::costs::additional_texts;
use crate::ability::{Ability, CostChange};
use crate::cost::CostItem;
use crate::refs::Pred;
use crate::value::Amount;

/// Every wording of the two abilities as one clause; empty for any other two.
pub(super) fn texts(first: &Ability, second: &Ability) -> Vec<String> {
    let (
        Ability::AdditionalCost {
            cost,
            required: true,
            repeatable: false,
        },
        Ability::CostModifier {
            applies_to,
            change: CostChange::Reduce,
            mana,
            per,
            condition: None,
            targeting: None,
            activating: None,
            least: 0,
            life: 0,
        },
    ) = (first, second)
    else {
        return Vec::new();
    };
    if applies_to.0.as_slice() != [Pred::This] || mana.has_x() {
        return Vec::new();
    }
    let Some(times) = (match per {
        Amount::X => Some(1),
        Amount::Times { by, of } if **of == Amount::X && *by > 0 => u32::try_from(*by).ok(),
        _ => None,
    }) else {
        return Vec::new();
    };
    let generic = mana.rest().generic();
    if generic == 0 || !mana.rest().is_generic_only(generic) {
        return Vec::new();
    }
    let less = generic * times;
    let (nouns, done): (Vec<String>, &str) = match cost.0.as_slice() {
        [
            CostItem::Sacrifice {
                filter,
                count: Amount::X,
            },
        ] => {
            let mut nouns = vec!["permanent".to_owned()];
            if let [Pred::Type(kind)] = filter.0.as_slice() {
                nouns.push(format!("{kind:?}").to_lowercase());
            }
            (nouns, "sacrificed")
        }
        [
            CostItem::ExileFromGraveyard {
                count: Amount::X, ..
            },
        ] => (vec!["card".to_owned()], "exiled"),
        [
            CostItem::RemoveCountersAmong {
                count: Amount::X, ..
            },
        ] => (vec!["counter".to_owned()], "removed"),
        _ => return Vec::new(),
    };
    let mut out = Vec::new();
    for head in additional_texts(cost, true) {
        if !head.contains(" any number of ") {
            continue;
        }
        for noun in &nouns {
            out.push(format!(
                "{head} ~ costs {{{less}}} less to cast for each {noun} {done} this way."
            ));
        }
    }
    out
}
