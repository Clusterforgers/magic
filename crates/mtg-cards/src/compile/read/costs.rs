//! Costs: what is paid before the colon of an activated ability, read from "{2}, {T}, Sacrifice ~"
//! in the order it is printed and written back the same way.

use super::amounts::{cards, cards_text, count_word, number_word, points, points_text};
use super::effects::describes_a_kind;
use super::selectors::{Form, phrase, phrase_text};
use super::tokens::Reader;
use super::triggers::article;
use crate::cost::{Cost, CostItem};
use crate::mana::ManaCostX;
use crate::refs::{Pred, Selector};
use crate::value::Amount;

/// The parts of a cost, separated by commas: `{T}`, `{Q}`, a run of mana symbols, "Pay 2 life",
/// "Sacrifice ~", "Sacrifice another creature", "Discard a card".
pub fn cost(r: &mut Reader<'_>) -> Option<Cost> {
    let mut items = vec![item(r)?];
    while r.punct(',') {
        items.push(item(r)?);
    }
    Some(Cost(items))
}

/// One part of a cost.
fn item(r: &mut Reader<'_>) -> Option<CostItem> {
    if let Some(mana) = r.attempt(mana) {
        return Some(CostItem::Mana(mana));
    }
    if let Some(symbol) = r.attempt(|r| r.symbol().filter(|symbol| matches!(*symbol, "T" | "Q"))) {
        return Some(if symbol == "T" {
            CostItem::Tap
        } else {
            CostItem::Untap
        });
    }
    r.attempt(worded)
}

/// A run of mana symbols other than the tap symbols.
fn mana(r: &mut Reader<'_>) -> Option<ManaCostX> {
    let mut text = String::new();
    while let Some(symbol) =
        r.attempt(|r| r.symbol().filter(|symbol| !matches!(*symbol, "T" | "Q")))
    {
        text.push('{');
        text.push_str(symbol);
        text.push('}');
    }
    if text.is_empty() {
        return None;
    }
    text.parse().ok()
}

/// "Pay 2 life", "Sacrifice ~", "Sacrifice a creature", "Discard a card".
fn worded(r: &mut Reader<'_>) -> Option<CostItem> {
    if r.word("pay") {
        let amount = points(r)?;
        r.word("life").then_some(())?;
        return Some(CostItem::PayLife(amount));
    }
    if r.word("discard") {
        let count = cards(r)?;
        return Some(CostItem::Discard {
            count,
            filter: None,
        });
    }
    r.need("sacrifice")?;
    if r.source() {
        return Some(CostItem::SacrificeSelf);
    }
    sacrifice(r)
}

/// What is sacrificed: "a creature", "another artifact", "two Goblins".
fn sacrifice(r: &mut Reader<'_>) -> Option<CostItem> {
    let (filter, count) = if r.word("a") || r.word("an") || r.peek_word() == Some("another") {
        (phrase(r, Form::Subject)?, 1)
    } else {
        let number = number_word(r.take_word()?).filter(|number| *number > 1)?;
        (phrase(r, Form::Targets)?, number)
    };
    // The payer sacrifices what they control: a cost says nothing of who controls it or where it is.
    filter.0.iter().all(is_cost_filter).then_some(())?;
    Some(CostItem::Sacrifice {
        filter,
        count: Amount::Fixed(i32::try_from(count).ok()?),
    })
}

/// Whether a predicate of a sacrificed permanent says what kind it is, or that it is another one.
fn is_cost_filter(pred: &Pred) -> bool {
    *pred == Pred::Other || describes_a_kind(pred)
}

/// The cost as the card prints it, parts in order: `{2}, {T}, Sacrifice ~`.
pub fn text(cost: &Cost) -> Option<String> {
    let parts: Option<Vec<String>> = cost.0.iter().map(item_text).collect();
    let parts = parts?;
    (!parts.is_empty()).then(|| parts.join(", "))
}

fn item_text(item: &CostItem) -> Option<String> {
    Some(match item {
        CostItem::Mana(mana) => mana.to_string(),
        CostItem::Tap => "{T}".to_owned(),
        CostItem::Untap => "{Q}".to_owned(),
        CostItem::PayLife(amount) => format!("Pay {} life", points_text(amount)?),
        CostItem::SacrificeSelf => "Sacrifice ~".to_owned(),
        CostItem::Sacrifice { filter, count } => sacrifice_text(filter, count)?,
        CostItem::Discard {
            count,
            filter: None,
        } => format!("Discard {}", cards_text(count)?),
        _ => return None,
    })
}

/// "a creature", "another creature", "two other creatures".
fn sacrifice_text(filter: &Selector, count: &Amount) -> Option<String> {
    let Amount::Fixed(number) = count else {
        return None;
    };
    if filter.0.is_empty() || !filter.0.iter().all(is_cost_filter) {
        return None;
    }
    if *number == 1 {
        let text = phrase_text(filter, Form::Subject)?;
        return Some(if filter.0.contains(&Pred::Other) {
            format!("Sacrifice {text}")
        } else {
            format!("Sacrifice {} {text}", article(&text))
        });
    }
    let text = phrase_text(filter, Form::Targets)?;
    Some(format!(
        "Sacrifice {} {text}",
        count_word(u32::try_from(*number).ok().filter(|n| *n > 1)?)?
    ))
}
