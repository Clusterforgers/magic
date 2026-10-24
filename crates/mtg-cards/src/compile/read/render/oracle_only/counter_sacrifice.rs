//! A cost that removes counters and sacrifices the card: "Remove three quest counters from this enchantment and
//! sacrifice it: Draw two cards." The renderers write the sacrifice as a part of the cost of its own.

use crate::ability::Ability;
use crate::cost::CostItem;

/// Adds the wordings with "and sacrifice it" for counters removed from the card and the card sacrificed.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Activated { cost, .. } = ability else {
        return;
    };
    let removes = matches!(
        cost.0.as_slice(),
        [CostItem::RemoveCounters { .. }, CostItem::SacrificeSelf]
    );
    if !removes {
        return;
    }
    let said: Vec<String> = texts
        .iter()
        .filter(|text| text.starts_with("Remove "))
        .filter_map(|text| {
            let (head, rest) = text.split_once(", Sacrifice ~: ")?;
            Some(format!("{head} and sacrifice it: {rest}"))
        })
        .collect();
    texts.extend(said);
}
