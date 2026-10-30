//! The costs a permission to cast adds, said in the gerund: "by paying 3 life and discarding a card in addition to paying
//! its other costs".

use super::amounts::{cards, cards_text, points, points_text};
use super::costs;
use super::tokens::Reader;
use crate::cost::{Cost, CostItem};
use crate::value::Amount;

/// After "by": the parts joined by "and" or commas, then "in addition to paying its other costs".
pub fn read(r: &mut Reader<'_>) -> Option<Cost> {
    let mut items = vec![item(r)?];
    while r.punct(',') || r.peek_word() == Some("and") {
        r.word("and");
        items.push(item(r)?);
    }
    r.need("in addition to paying its other costs")?;
    Some(Cost(items))
}

/// One part: "paying 3 life", "paying {1}{B}", "discarding a card".
fn item(r: &mut Reader<'_>) -> Option<CostItem> {
    if r.word("discarding") {
        let count = cards(r)?;
        return matches!(count, Amount::Fixed(_)).then_some(CostItem::Discard {
            count,
            filter: None,
        });
    }
    r.word("paying").then_some(())?;
    if let Some(mana) = r.attempt(costs::mana) {
        return (!mana.has_x()).then_some(CostItem::Mana(mana));
    }
    let amount = points(r)?;
    matches!(amount, Amount::Fixed(_)).then_some(())?;
    r.word("life").then_some(())?;
    Some(CostItem::PayLife(amount))
}

/// "by paying 3 life and discarding a card in addition to paying its other costs" for a cost the reader reads, else none.
pub fn text(cost: &Cost) -> Option<String> {
    let parts: Vec<String> = cost.0.iter().map(item_text).collect::<Option<_>>()?;
    let joined = match parts.as_slice() {
        [] => return None,
        [only] => only.clone(),
        [first, second] => format!("{first} and {second}"),
        [init @ .., last] => format!("{}, and {last}", init.join(", ")),
    };
    Some(format!("by {joined} in addition to paying its other costs"))
}

fn item_text(item: &CostItem) -> Option<String> {
    Some(match item {
        CostItem::Mana(mana) if !mana.has_x() => format!("paying {mana}"),
        CostItem::PayLife(amount @ Amount::Fixed(_)) => {
            format!("paying {} life", points_text(amount)?)
        }
        CostItem::Discard {
            count: count @ Amount::Fixed(_),
            filter: None,
        } => format!("discarding {}", cards_text(count)?),
        _ => return None,
    })
}
