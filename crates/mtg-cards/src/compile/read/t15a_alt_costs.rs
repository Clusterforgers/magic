//! An alternative cost of two parts joined by "and" (CR 118.9): "You may pay 1 life and exile a blue card from your hand
//! rather than pay this spell's mana cost."

use super::Reader;
use super::costs;
use crate::ability::Ability;
use crate::cost::{Cost, CostItem};

/// "you may <part> and <part> rather than pay ~'s mana cost.": a cast for both parts in place of the mana cost.
pub(super) fn t15a_alt_cost(r: &mut Reader<'_>) -> Option<Ability> {
    r.words("you may").then_some(())?;
    let first = t15a_part(r)?;
    r.word("and").then_some(())?;
    let second = t15a_part(r)?;
    r.words("rather than pay").then_some(())?;
    r.source().then_some(())?;
    r.punct('\'').then_some(())?;
    r.word("s").then_some(())?;
    r.words("mana cost").then_some(())?;
    Some(Ability::FreeCast {
        condition: None,
        cost: Some(Cost(vec![first, second])),
    })
}

/// One part of the cost: a single item that neither names mana nor an X.
fn t15a_part(r: &mut Reader<'_>) -> Option<CostItem> {
    let cost = costs::cost(r)?;
    match cost.0.as_slice() {
        [item] if !cost.has_x() && !matches!(item, CostItem::Mana(_)) => Some(item.clone()),
        _ => None,
    }
}

/// The sentence of a cast for two parts in place of the mana cost, without the period.
pub(super) fn t15a_alt_cost_texts(ability: &Ability) -> Vec<String> {
    let Ability::FreeCast {
        condition: None,
        cost: Some(cost),
    } = ability
    else {
        return Vec::new();
    };
    let [first, second] = cost.0.as_slice() else {
        return Vec::new();
    };
    let said: Option<Vec<String>> = [first, second]
        .into_iter()
        .map(|item| costs::text(&Cost(vec![item.clone()])).map(|text| costs::lowered(&text)))
        .collect();
    match said.as_deref() {
        Some([first, second]) => {
            vec![format!(
                "you may {first} and {second} rather than pay ~'s mana cost"
            )]
        }
        _ => Vec::new(),
    }
}
