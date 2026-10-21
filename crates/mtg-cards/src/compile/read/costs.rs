//! Costs: what is paid before the colon of an activated ability, read from "{2}, {T}, Sacrifice ~"
//! in the order it is printed and written back the same way.

use super::amounts::{cards, cards_text, count_word, number_word, points, points_text};
use super::counters::{counters, text as counters_text};
use super::effects::describes_a_kind;
use super::selectors::{Form, phrase, phrase_text};
use super::tokens::Reader;
use super::triggers::article;
use crate::cost::{Cost, CostItem};
use crate::mana::ManaCostX;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
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
        if let Some(energy) = r.attempt(energy) {
            return Some(CostItem::PayEnergy(Amount::Fixed(energy)));
        }
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
    if r.word("tap") {
        return tap(r);
    }
    if r.word("exile") {
        return exile(r);
    }
    if r.word("return") {
        return bounce(r);
    }
    if r.word("remove") {
        return remove(r);
    }
    r.need("sacrifice")?;
    if r.source() {
        return Some(CostItem::SacrificeSelf);
    }
    sacrifice(r)
}

/// "a" or "an" for one, a number above one for several: how many a cost takes and how the phrase after it reads.
fn how_many(r: &mut Reader<'_>) -> Option<(Form, i32)> {
    if r.word("a") || r.word("an") {
        return Some((Form::Subject, 1));
    }
    let number = number_word(r.take_word()?).filter(|number| *number > 1)?;
    Some((Form::Targets, i32::try_from(number).ok()?))
}

/// Takes one `pred` out of a filter; `None` when it is not there.
fn without(filter: &mut Selector, pred: &Pred) -> Option<()> {
    let at = filter.0.iter().position(|found| found == pred)?;
    filter.0.remove(at);
    Some(())
}

/// What a cost that takes objects says of them: a kind, and nothing else.
fn kinds_only(filter: Selector) -> Option<Selector> {
    filter.0.iter().all(is_cost_filter).then_some(filter)
}

/// "Tap an untapped creature you control", "Tap two untapped creatures you control": the payer taps untapped
/// permanents they control, which the part says for itself.
fn tap(r: &mut Reader<'_>) -> Option<CostItem> {
    let (form, count) = how_many(r)?;
    let mut filter = phrase(r, form)?;
    without(&mut filter, &Pred::Untapped)?;
    without(&mut filter, &Pred::ControlledBy(Ref::You))?;
    Some(CostItem::TapPermanents {
        filter: kinds_only(filter)?,
        count: Amount::Fixed(count),
    })
}

/// "Exile a creature card from your graveyard", "Exile two cards from your graveyard".
fn exile(r: &mut Reader<'_>) -> Option<CostItem> {
    let (form, count) = how_many(r)?;
    let mut filter = phrase(r, form)?;
    without(&mut filter, &Pred::In(ZoneKind::Graveyard))?;
    without(&mut filter, &Pred::OwnedBy(Ref::You))?;
    Some(CostItem::ExileFromGraveyard {
        filter: kinds_only(filter)?,
        count: Amount::Fixed(count),
    })
}

/// "Return a land you control to its owner's hand".
fn bounce(r: &mut Reader<'_>) -> Option<CostItem> {
    (r.word("a") || r.word("an")).then_some(())?;
    let mut filter = phrase(r, Form::Subject)?;
    without(&mut filter, &Pred::ControlledBy(Ref::You))?;
    r.words("to its owner's hand").then_some(())?;
    Some(CostItem::ReturnToHand {
        filter: kinds_only(filter)?,
        count: Amount::one(),
    })
}

/// "Remove a charge counter from ~": counters of the card itself.
fn remove(r: &mut Reader<'_>) -> Option<CostItem> {
    let (counter, number) = counters(r)?;
    r.need("from")?;
    r.source().then_some(())?;
    Some(CostItem::RemoveCounters {
        counter,
        count: Amount::Fixed(i32::try_from(number).ok()?),
    })
}

/// A run of energy symbols: "{E}{E}" is two energy.
fn energy(r: &mut Reader<'_>) -> Option<i32> {
    let mut count = 0;
    while r
        .attempt(|r| r.symbol().filter(|symbol| *symbol == "E"))
        .is_some()
    {
        count += 1;
    }
    (count > 0).then_some(count)
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
        CostItem::PayEnergy(Amount::Fixed(count)) if *count > 0 => {
            format!("Pay {}", "{E}".repeat(usize::try_from(*count).ok()?))
        }
        CostItem::Sacrifice { filter, count } => sacrifice_text(filter, count)?,
        CostItem::Discard {
            count,
            filter: None,
        } => format!("Discard {}", cards_text(count)?),
        CostItem::TapPermanents { filter, count } => {
            let mut said = filter.clone();
            said.0.extend([Pred::Untapped, Pred::ControlledBy(Ref::You)]);
            format!("Tap {}", taken_text(&said, count)?)
        }
        CostItem::ExileFromGraveyard { filter, count } => {
            let mut said = filter.clone();
            said.0
                .extend([Pred::In(ZoneKind::Graveyard), Pred::OwnedBy(Ref::You)]);
            format!("Exile {}", taken_text(&said, count)?)
        }
        CostItem::ReturnToHand { filter, count } if *count == Amount::one() => {
            let mut said = filter.clone();
            said.0.push(Pred::ControlledBy(Ref::You));
            format!(
                "Return {} to its owner's hand",
                taken_text(&said, count)?
            )
        }
        CostItem::RemoveCounters { counter, count } => {
            format!("Remove {} from ~", counters_text(counter, count)?)
        }
        _ => return None,
    })
}

/// "a creature you control", "two untapped creatures you control": what a cost takes, in the words `phrase` reads.
fn taken_text(said: &Selector, count: &Amount) -> Option<String> {
    let Amount::Fixed(number) = count else {
        return None;
    };
    if *number == 1 {
        let text = phrase_text(said, Form::Subject)?;
        return Some(format!("{} {text}", article(&text)));
    }
    let text = phrase_text(said, Form::Targets)?;
    Some(format!(
        "{} {text}",
        count_word(u32::try_from(*number).ok().filter(|n| *n > 1)?)?
    ))
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
