//! A sacrifice that another sacrifice avoids: "At the beginning of your upkeep, sacrifice this enchantment unless you
//! sacrifice a Pegasus." The renderers have no wording for a payment made of a sacrifice.

use super::super::{sacrifice_text, trigger_head};
use crate::ability::Ability;
use crate::cost::CostItem;
use crate::effect::Effect;
use crate::refs::{Ref, Selector};

/// Adds the wordings of a trigger whose only step sacrifices the source unless the player sacrifices a permanent.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when,
        intervening_if,
        limit: None,
        effects,
        ..
    } = ability
    else {
        return;
    };
    let [
        Effect::Pay {
            who: Ref::You,
            cost,
            times,
            paid,
            unpaid,
        },
    ] = effects.as_slice()
    else {
        return;
    };
    let [CostItem::Sacrifice { filter, count }] = cost.0.as_slice() else {
        return;
    };
    let [
        Effect::Sacrifice {
            who: Ref::You,
            filter: itself,
            count: one,
        },
    ] = unpaid.as_slice()
    else {
        return;
    };
    if !paid.is_empty()
        || !times.is_one()
        || !count.is_one()
        || !one.is_one()
        || *itself != Selector::this()
    {
        return;
    }
    let (Some(head), Some(sacrificed)) = (
        trigger_head(when, intervening_if.as_ref()),
        sacrifice_text(filter),
    ) else {
        return;
    };
    texts.push(format!(
        "{} sacrifice ~ unless you sacrifice {sacrificed}.",
        head.join(" ")
    ));
}
