//! "As an additional cost to cast this spell, you may exile any number of white cards from your hand. This spell costs
//! {2} less to cast for each card exiled this way.": a repeatable additional cost of one object each time and the spell's
//! own reduction for each payment (`Amount::AdditionalPaid`), one clause of two abilities in order.

use super::costs::additional_texts;
use crate::ability::{Ability, CostChange};
use crate::cost::{Cost, CostItem};
use crate::refs::Pred;
use crate::value::Amount;

/// The cost paid all at once: the one part of a repeatable cost with its count X ("exile X white cards from your hand"),
/// and the noun and verb of "for each card exiled this way"; `None` for a part the writer does not count.
fn counted(cost: &Cost) -> Option<(Cost, &'static str, &'static str)> {
    let [item] = cost.0.as_slice() else {
        return None;
    };
    let x = Amount::X;
    let (item, noun, done) = match item {
        CostItem::ExileFromHand { filter, count } if count.is_one() => (
            CostItem::ExileFromHand {
                filter: filter.clone(),
                count: x,
            },
            "card",
            "exiled",
        ),
        CostItem::TapPermanents { filter, count } if count.is_one() => {
            let noun = if filter.0.contains(&Pred::Type(mtg_core::card::CardType::Creature)) {
                "creature"
            } else {
                "permanent"
            };
            (
                CostItem::TapPermanents {
                    filter: filter.clone(),
                    count: x,
                },
                noun,
                "tapped",
            )
        }
        _ => return None,
    };
    Some((Cost(vec![item]), noun, done))
}

/// "As an additional cost to cast ~, you may pay {1}{G} any number of times.": a repeatable cost of mana alone.
pub(super) fn ep30_mana_texts(cost: &Cost) -> Vec<String> {
    match cost.0.as_slice() {
        [CostItem::Mana(mana)] if !mana.has_x() => vec![format!(
            "As an additional cost to cast ~, you may pay {mana} any number of times."
        )],
        _ => Vec::new(),
    }
}

/// Every wording of the two abilities as one clause; empty for any other two.
pub(super) fn ep30_paid_texts(first: &Ability, second: &Ability) -> Vec<String> {
    let (
        Ability::AdditionalCost {
            cost,
            required: false,
            repeatable: true,
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
    let times = match per {
        Amount::AdditionalPaid => 1,
        Amount::Times { by, of } if **of == Amount::AdditionalPaid && *by > 0 => {
            u32::try_from(*by).unwrap_or(0)
        }
        _ => return Vec::new(),
    };
    let generic = mana.rest().generic();
    if generic == 0 || !mana.rest().is_generic_only(generic) {
        return Vec::new();
    }
    let generic = generic * times;
    let Some((all, noun, done)) = counted(cost) else {
        return Vec::new();
    };
    additional_texts(&all, true)
        .into_iter()
        .filter_map(|head| {
            if head.contains(" any number of ") {
                return Some(head);
            }
            // "tap X untapped creatures you control" as "you may tap any number of untapped creatures you control".
            let body = head.strip_prefix("As an additional cost to cast ~, ")?;
            let (verb, rest) = body.split_once(" X ")?;
            (verb == "tap").then(|| {
                format!("As an additional cost to cast ~, you may {verb} any number of {rest}")
            })
        })
        .map(|head| {
            format!("{head} ~ costs {{{generic}}} less to cast for each {noun} {done} this way.")
        })
        .collect()
}
