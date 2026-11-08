//! "If a Dragon was beheld", "if evidence was collected": the Oracle's words for the paid additional cost of a spell whose
//! one additional cost is to behold a card of one subtype or to collect evidence, read as "if this spell's additional
//! cost was paid".

use crate::ability::Ability;
use crate::cost::CostItem;
use crate::refs::Pred;

/// The words "a Dragon was beheld" or "evidence was collected" of a face whose one additional cost is that cost alone.
pub(super) fn paid_words(abilities: &[Ability]) -> Option<String> {
    let mut costs = abilities.iter().filter_map(|ability| match ability {
        Ability::AdditionalCost { cost, .. } => Some(cost),
        _ => None,
    });
    let cost = costs.next()?;
    if costs.next().is_some() {
        return None;
    }
    match cost.0.as_slice() {
        [CostItem::CollectEvidence(_)] => Some("evidence was collected".to_owned()),
        [
            CostItem::Behold {
                filter,
                count,
                exile: false,
            },
        ] => {
            let [Pred::Subtype(subtype)] = filter.0.as_slice() else {
                return None;
            };
            let name = &subtype.0;
            let article = if name.starts_with(['A', 'E', 'I', 'O', 'U']) {
                "an"
            } else {
                "a"
            };
            count
                .is_one()
                .then(|| format!("{article} {name} was beheld"))
        }
        _ => None,
    }
}

/// The clause with "if a Dragon was beheld" said as "if this spell's additional cost was paid"; `None` without the words.
pub(super) fn unpaid_words(text: &str, words: &str) -> Option<String> {
    let mut out: Option<String> = None;
    for head in ["If", "if"] {
        let said = format!("{head} {words}");
        let current = out.as_deref().unwrap_or(text);
        if current.contains(&said) {
            let plain = format!("{head} this spell's additional cost was paid");
            out = Some(current.replace(&said, &plain));
        }
    }
    out
}
