//! Costs: what is paid before the colon of an activated ability, read from "{2}, {T}, Sacrifice ~"
//! in the order it is printed and written back the same way.

use super::amounts::{cards, cards_text, count_word, number_word, points, points_text};
use super::effects::describes_a_kind;
use super::selectors::{Form, phrase, phrase_in, phrase_text};
use super::tokens::Reader;
use super::triggers::article;
use super::w2f_any_counters::w2f_text;
use crate::ability::Ability;
use crate::cost::{Cost, CostItem};
use crate::mana::ManaCostX;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::value::Amount;

/// The parts of a cost, separated by commas: `{T}`, `{Q}`, a run of mana symbols, "Pay 2 life",
/// "Sacrifice ~", "Sacrifice another creature", "Discard a card".
pub fn cost(r: &mut Reader<'_>) -> Option<Cost> {
    // An X in a cost is the one the player announces as they activate the ability: "Pay X life", "Sacrifice X creatures".
    let before = std::mem::replace(&mut r.x_free, true);
    let unpaid = std::mem::replace(&mut r.x_unpaid, !before);
    let items = items(r);
    r.x_free = before;
    r.x_unpaid = unpaid;
    items.map(Cost)
}

fn items(r: &mut Reader<'_>) -> Option<Vec<CostItem>> {
    let mut items = vec![item(r)?];
    loop {
        if let Some(CostItem::Sacrifice { count, .. }) = items.last()
            && *count == Amount::one()
            && let Some(next) = r.attempt(and_sacrifice)
        {
            items.push(next);
        } else if r.punct(',') {
            items.push(item(r)?);
        } else {
            return Some(items);
        }
    }
}

/// "and a Forest" after "Sacrifice a Swamp": one more permanent sacrificed, a part of its own.
fn and_sacrifice(r: &mut Reader<'_>) -> Option<CostItem> {
    r.word("and").then_some(())?;
    (r.word("a") || r.word("an")).then_some(())?;
    let filter = phrase(r, Form::Subject)?;
    filter.0.iter().all(is_cost_filter).then_some(())?;
    Some(CostItem::Sacrifice {
        filter,
        count: Amount::one(),
    })
}

/// One part of a cost.
fn item(r: &mut Reader<'_>) -> Option<CostItem> {
    if let Some(mana) = r.attempt(mana) {
        // An `{X}` paid here is the number announced, which the stat of a later part may name.
        if mana.has_x() {
            r.x_unpaid = false;
        }
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
            return Some(CostItem::PayEnergy(energy));
        }
        let amount = points(r)?;
        r.word("life").then_some(())?;
        return Some(CostItem::PayLife(amount));
    }
    if r.word("discard") {
        return discard(r);
    }
    if r.word("tap") {
        return tap(r);
    }
    if r.word("exile") {
        return exile(r);
    }
    if r.word("reveal") {
        return reveal(r);
    }
    if r.word("return") {
        return bounce(r);
    }
    if r.word("remove") {
        return remove(r);
    }
    if r.word("mill") {
        return cards(r).map(CostItem::Mill);
    }
    r.need("sacrifice")?;
    if r.source() {
        return Some(CostItem::SacrificeSelf);
    }
    sacrifice(r)
}

/// "Discard a card", "Discard two cards", "Discard a land card", "Discard X creature cards".
fn discard(r: &mut Reader<'_>) -> Option<CostItem> {
    if let Some(count) = r.attempt(cards) {
        return Some(CostItem::Discard {
            count,
            filter: None,
        });
    }
    let (form, count) = how_many(r)?;
    let filter = phrase_in(r, form, true)?;
    // "a land" is not a card to discard: the noun must be "card".
    matches!(r.last_word(), Some("card" | "cards")).then_some(())?;
    Some(CostItem::Discard {
        count,
        filter: Some(kinds_only(filter).filter(|kinds| !kinds.0.is_empty())?),
    })
}

/// "a" or "an" for one, a number above one for several, X for the number announced: how many a cost takes and how the
/// phrase after it reads.
fn how_many(r: &mut Reader<'_>) -> Option<(Form, Amount)> {
    if r.word("a") || r.word("an") {
        return Some((Form::Subject, Amount::one()));
    }
    if r.x_free && r.word("x") {
        return Some((Form::Targets, Amount::X));
    }
    let number = number_word(r.take_word()?).filter(|number| *number > 1)?;
    Some((Form::Targets, Amount::Fixed(i32::try_from(number).ok()?)))
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
        count,
    })
}

/// "Exile a creature card from your graveyard", "Exile two cards from your graveyard".
fn exile(r: &mut Reader<'_>) -> Option<CostItem> {
    if r.source() {
        // "Exile ~ from your graveyard" is an ability of the graveyard; "Exile ~" is one of the battlefield.
        r.from_graveyard = r.words("from your graveyard");
        return Some(CostItem::ExileSelf);
    }
    let (form, count) = how_many(r)?;
    let mut filter = phrase(r, form)?;
    let from_hand = without(&mut filter, &Pred::In(ZoneKind::Hand)).is_some();
    if !from_hand {
        without(&mut filter, &Pred::In(ZoneKind::Graveyard))?;
    }
    without(&mut filter, &Pred::OwnedBy(Ref::You))?;
    let filter = kinds_only(filter)?;
    Some(if from_hand {
        CostItem::ExileFromHand { filter, count }
    } else {
        CostItem::ExileFromGraveyard { filter, count }
    })
}

/// "Reveal a creature card from your hand", "Reveal a Giant card from your hand": the card stays in the hand.
fn reveal(r: &mut Reader<'_>) -> Option<CostItem> {
    let (form, count) = how_many(r)?;
    let mut filter = phrase(r, form)?;
    without(&mut filter, &Pred::In(ZoneKind::Hand))?;
    without(&mut filter, &Pred::OwnedBy(Ref::You))?;
    Some(CostItem::RevealFromHand {
        filter: kinds_only(filter)?,
        count,
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
    let (counter, count) = super::w2f_any_counters::w2f_counters(r)?;
    r.need("from")?;
    if r.source() {
        // A counter of any kind from the source asks for its kind, which the engine does not yet.
        counter.as_ref()?;
        return Some(CostItem::RemoveCounters {
            counter,
            count,
            from: None,
        });
    }
    // "Remove a +1/+1 counter from a creature you control": one counter from a permanent the payer picks.
    (count == Amount::one()).then_some(())?;
    let from = c11c_permanent(r)?;
    Some(CostItem::RemoveCounters {
        counter,
        count,
        from: Some(from),
    })
}

/// "a creature you control", "another creature you control", "a permanent you control": a kind of permanent the payer
/// controls, named without "you control".
fn c11c_permanent(r: &mut Reader<'_>) -> Option<Selector> {
    let another = r.peek_word() == Some("another");
    if !another {
        (r.word("a") || r.word("an")).then_some(())?;
    }
    let mut filter = phrase(r, Form::Subject)?;
    without(&mut filter, &Pred::ControlledBy(Ref::You))?;
    (another == filter.0.contains(&Pred::Other)).then_some(())?;
    kinds_only(filter)
}

/// The energy symbols of an amount: "{E}{E}" for two, "X {E}" for the number announced.
pub(super) fn energy_text(amount: &Amount) -> Option<String> {
    match amount {
        Amount::Fixed(count) if *count > 0 => Some("{E}".repeat(usize::try_from(*count).ok()?)),
        Amount::X => Some("X {E}".to_owned()),
        _ => None,
    }
}

/// A run of energy symbols: "{E}{E}" is two energy, "X {E}" the number announced.
pub(super) fn energy(r: &mut Reader<'_>) -> Option<Amount> {
    let x = r.x_free && r.word("x");
    let mut count = 0;
    while r
        .attempt(|r| r.symbol().filter(|symbol| *symbol == "E"))
        .is_some()
    {
        count += 1;
    }
    match (x, count) {
        (true, 1) => Some(Amount::X),
        (false, 1..) => Some(Amount::Fixed(count)),
        _ => None,
    }
}

/// What is sacrificed: "a creature", "another artifact", "two Goblins".
fn sacrifice(r: &mut Reader<'_>) -> Option<CostItem> {
    let (filter, count) = if r.word("a") || r.word("an") || r.peek_word() == Some("another") {
        (phrase(r, Form::Subject)?, Amount::one())
    } else {
        let (_, count) = how_many(r)?;
        (phrase(r, Form::Targets)?, count)
    };
    // The payer sacrifices what they control: a cost says nothing of who controls it or where it is.
    filter.0.iter().all(is_cost_filter).then_some(())?;
    Some(CostItem::Sacrifice { filter, count })
}

/// Whether a predicate of a sacrificed permanent says what kind it is, or that it is another one.
fn is_cost_filter(pred: &Pred) -> bool {
    *pred == Pred::Other || describes_a_kind(pred)
}

/// Whether the cost takes objects out of their zones (a sacrifice, a discard, an exile), which the effects name as
/// "the sacrificed creature's power".
pub(super) fn takes_objects(cost: &Cost) -> bool {
    cost.0.iter().any(|item| match item {
        CostItem::OneOf(costs) => costs.iter().any(takes_objects),
        other => matches!(
            other,
            CostItem::Sacrifice { .. }
                | CostItem::Discard { .. }
                | CostItem::ExileFromGraveyard { .. }
                | CostItem::ExileFromHand { .. }
                | CostItem::RevealFromHand { .. }
        ),
    })
}

/// The cost as the card prints it, parts in order: `{2}, {T}, Sacrifice ~`.
pub fn text(cost: &Cost) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    let mut after_sacrifice = false;
    for item in &cost.0 {
        let part = item_text(item)?;
        let single = matches!(item, CostItem::Sacrifice { filter, count }
            if *count == Amount::one() && !filter.0.contains(&Pred::Other));
        // "Sacrifice a Swamp and a Forest": a run of single sacrifices is one clause.
        match parts.last_mut() {
            Some(last) if single && after_sacrifice => {
                last.push_str(" and ");
                last.push_str(part.strip_prefix("Sacrifice ")?);
            }
            _ => parts.push(part),
        }
        after_sacrifice = single;
    }
    (!parts.is_empty()).then(|| parts.join(", "))
}

fn item_text(item: &CostItem) -> Option<String> {
    Some(match item {
        CostItem::Mana(mana) => mana.to_string(),
        CostItem::Tap => "{T}".to_owned(),
        CostItem::Untap => "{Q}".to_owned(),
        CostItem::PayLife(amount) => format!("Pay {} life", points_text(amount)?),
        CostItem::SacrificeSelf => "Sacrifice ~".to_owned(),
        CostItem::ExileSelf => "Exile ~ from your graveyard".to_owned(),
        CostItem::PayEnergy(Amount::Fixed(count)) if *count > 0 => {
            format!("Pay {}", "{E}".repeat(usize::try_from(*count).ok()?))
        }
        CostItem::PayEnergy(Amount::X) => "Pay X {E}".to_owned(),
        CostItem::Mill(count) => format!("Mill {}", cards_text(count)?),
        CostItem::Sacrifice { filter, count } => sacrifice_text(filter, count)?,
        CostItem::Discard {
            count,
            filter: None,
        } => format!("Discard {}", cards_text(count)?),
        CostItem::Discard {
            count,
            filter: Some(filter),
        } => {
            let mut said = filter.clone();
            said.0
                .extend([Pred::In(ZoneKind::Hand), Pred::OwnedBy(Ref::You)]);
            let text = taken_text(&said, count)?;
            format!("Discard {}", text.strip_suffix(" from your hand")?)
        }
        CostItem::TapPermanents { filter, count } => tap_text(filter, count)?,
        CostItem::ExileFromGraveyard { filter, count } => {
            let mut said = filter.clone();
            said.0
                .extend([Pred::In(ZoneKind::Graveyard), Pred::OwnedBy(Ref::You)]);
            format!("Exile {}", taken_text(&said, count)?)
        }
        CostItem::ExileFromHand { filter, count } => {
            let mut said = filter.clone();
            said.0
                .extend([Pred::In(ZoneKind::Hand), Pred::OwnedBy(Ref::You)]);
            format!("Exile {}", taken_text(&said, count)?)
        }
        CostItem::RevealFromHand { filter, count } => {
            let mut said = filter.clone();
            said.0
                .extend([Pred::In(ZoneKind::Hand), Pred::OwnedBy(Ref::You)]);
            format!("Reveal {}", taken_text(&said, count)?)
        }
        CostItem::OneOf(costs) => choice_text(costs)?,
        CostItem::ReturnToHand { filter, count } if *count == Amount::one() => {
            let mut said = filter.clone();
            said.0.push(Pred::ControlledBy(Ref::You));
            format!("Return {} to its owner's hand", taken_text(&said, count)?)
        }
        CostItem::RemoveCounters {
            counter,
            count,
            from: None,
        } => {
            format!("Remove {} from ~", w2f_text(counter.as_ref(), count)?)
        }
        CostItem::RemoveCounters {
            counter,
            count,
            from: Some(from),
        } => {
            let mut said = from.clone();
            said.0.push(Pred::ControlledBy(Ref::You));
            format!(
                "Remove {} from {}",
                w2f_text(counter.as_ref(), count)?,
                taken_text(&said, &Amount::one())?
            )
        }
        _ => return None,
    })
}

/// "Tap an untapped creature you control", "Tap another untapped Rogue you control": untapped permanents the payer
/// controls, other than the source where the filter says so.
fn tap_text(filter: &Selector, count: &Amount) -> Option<String> {
    let mut said = filter.clone();
    let other = said.0.iter().position(|pred| *pred == Pred::Other);
    if let Some(at) = other {
        said.0.remove(at);
    }
    said.0
        .extend([Pred::Untapped, Pred::ControlledBy(Ref::You)]);
    let text = taken_text(&said, count)?;
    if other.is_none() {
        return Some(format!("Tap {text}"));
    }
    let rest = text
        .strip_prefix("an ")
        .or_else(|| text.strip_prefix("a "))?;
    Some(format!("Tap another {rest}"))
}

/// "a creature you control", "two untapped creatures you control": what a cost takes, in the words `phrase` reads.
fn taken_text(said: &Selector, count: &Amount) -> Option<String> {
    if *count == Amount::X {
        return Some(format!("X {}", phrase_text(said, Form::Targets)?));
    }
    let Amount::Fixed(number) = count else {
        return None;
    };
    if *number == 1 {
        let text = phrase_text(said, Form::Subject)?;
        // "another creature you control" takes no article of its own.
        if text.starts_with("another ") {
            return Some(text);
        }
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
    if filter.0.is_empty() || !filter.0.iter().all(is_cost_filter) {
        return None;
    }
    if *count == Amount::X {
        return Some(format!(
            "Sacrifice X {}",
            phrase_text(filter, Form::Targets)?
        ));
    }
    let Amount::Fixed(number) = count else {
        return None;
    };
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

/// "As an additional cost to cast this spell, you may sacrifice an artifact.": a cost the caster may pay as the spell is
/// announced; without "you may" ("..., discard a card.") the caster must pay it.
pub(super) fn additional(r: &mut Reader<'_>) -> Option<Ability> {
    r.words("as an additional cost to cast").then_some(())?;
    r.source().then_some(())?;
    r.punct(',').then_some(())?;
    let required = !r.words("you may");
    let paid_in_mana = if required {
        None
    } else {
        r.attempt(|r| {
            r.word("pay").then_some(())?;
            mana(r).map(Cost::mana)
        })
    };
    let cost = match paid_in_mana {
        Some(cost) => cost,
        None if required => match r.attempt(alternatives) {
            Some(choice) => choice,
            None => cost(r)?,
        },
        None => cost(r)?,
    };
    // The caster announces the X of a required cost that counts it ("Pay X life", "sacrifice X creatures"); an `{X}` of
    // mana and the X of an optional cost are left.
    let mana_x = cost
        .0
        .iter()
        .any(|item| matches!(item, CostItem::Mana(mana) if mana.has_x()));
    if mana_x || (!required && cost.has_x()) {
        return None;
    }
    Some(Ability::AdditionalCost { cost, required })
}

/// "discard a card or pay 3 life", "sacrifice a creature, discard a card, or pay 4 life", "reveal a Giant card from your
/// hand or pay {2}": the caster pays one of the costs, each of one part.
fn alternatives(r: &mut Reader<'_>) -> Option<Cost> {
    let before = std::mem::replace(&mut r.x_free, false);
    let found = alternative_list(r);
    r.x_free = before;
    found
}

fn alternative_list(r: &mut Reader<'_>) -> Option<Cost> {
    let mut costs = vec![alternative(r)?];
    let mut chosen = false;
    loop {
        let comma = r.punct(',');
        let or = r.word("or");
        if !comma && !or {
            break;
        }
        chosen |= or;
        costs.push(alternative(r)?);
    }
    chosen.then(|| Cost(vec![CostItem::OneOf(costs)]))
}

/// One cost of a choice: "pay {2}" or a part.
fn alternative(r: &mut Reader<'_>) -> Option<Cost> {
    let paid = r.attempt(|r| {
        r.word("pay").then_some(())?;
        mana(r).map(Cost::mana)
    });
    paid.or_else(|| item(r).map(|item| Cost(vec![item])))
}

/// The words of a choice between costs: "Discard a card or pay 3 life", "Sacrifice a creature, discard a card, or pay 4 life".
fn choice_text(costs: &[Cost]) -> Option<String> {
    let mut words = Vec::new();
    for (at, cost) in costs.iter().enumerate() {
        let [item] = cost.0.as_slice() else {
            return None;
        };
        let text = match item {
            CostItem::Mana(mana) => format!("pay {mana}"),
            CostItem::OneOf(_) => return None,
            other => item_text(other)?,
        };
        words.push(if at == 0 { text } else { lowered(&text) });
    }
    match words.as_slice() {
        [] | [_] => None,
        [one, two] => Some(format!("{one} or {two}")),
        [rest @ .., last] => Some(format!("{}, or {last}", rest.join(", "))),
    }
}

/// `text` with its first letter in lower case.
fn lowered(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|first| first.to_lowercase().chain(chars).collect())
        .unwrap_or_default()
}

/// The sentence an additional cost is written as; `None` for a cost the readers do not write.
pub(super) fn additional_text(cost: &Cost, required: bool) -> Option<String> {
    let body = if let [CostItem::Mana(mana)] = cost.0.as_slice() {
        // The reader takes a cost of mana alone only after "you may".
        (!required).then(|| format!("pay {mana}"))?
    } else {
        let text = text(cost)?;
        let mut chars = text.chars();
        let first = chars.next()?;
        first.to_lowercase().chain(chars).collect()
    };
    let may = if required { "" } else { "you may " };
    Some(format!("As an additional cost to cast ~, {may}{body}."))
}
