//! Costs to attack or block beyond mana and life (CR 508.1g-h, 509.1d): "~ can't attack unless you sacrifice two
//! Islands", "Green creatures can't attack unless their controller sacrifices a land of their choice for each green
//! creature they control that's attacking", "~ can't attack or block unless you tap an untapped creature you control not
//! declared as an attacking or blocking creature this combat".
//!
//! The taxes in force give each taxed creature their costs (`AttackTax::costs`, `BlockTax::costs`); the costs of a
//! declaration are added up, a part of each kind and filter taking all the permanents together. The player picks them
//! one part at a time with `ChooseCards`, as for an ability's cost, after the declaration and before its mana; a
//! creature being declared never pays a tap part. The parts are paid with the declaration's events, after it.

use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::value::Amount;
use mtg_core::ids::{ObjectId, PlayerId};

use super::{Done, Next, Phase};
use crate::choose::{CardsOptions, Choices};
use crate::decision::{Decision, DecisionKind};
use crate::events::Event;
use crate::parts::{Part, Payer};
use crate::price::Price;
use crate::state::GameState;

/// The costs of a tax paid `times` times for one creature: each item's fixed count multiplied, none for 0 times.
pub(crate) fn r6400_repeated(cost: &[CostItem], times: usize) -> Vec<CostItem> {
    let times = i32::try_from(times).unwrap_or(i32::MAX);
    if times == 0 {
        return Vec::new();
    }
    cost.iter()
        .map(|item| with_count(item, |count| count.saturating_mul(times)))
        .collect()
}

/// `item` with its fixed count changed by `change`; any other item as it is.
fn with_count(item: &CostItem, change: impl Fn(i32) -> i32) -> CostItem {
    let changed = |count: &Amount| match count {
        Amount::Fixed(n) => Amount::Fixed(change(*n)),
        other => other.clone(),
    };
    match item {
        CostItem::Sacrifice { filter, count } => CostItem::Sacrifice {
            filter: filter.clone(),
            count: changed(count),
        },
        CostItem::ReturnToHand { filter, count } => CostItem::ReturnToHand {
            filter: filter.clone(),
            count: changed(count),
        },
        CostItem::TapPermanents { filter, count } => CostItem::TapPermanents {
            filter: filter.clone(),
            count: changed(count),
        },
        other => other.clone(),
    }
}

/// The fixed count of an item that takes permanents, with the item at a count of one; `None` for any other item.
fn unit(item: &CostItem) -> Option<(CostItem, i32)> {
    let (CostItem::Sacrifice { count, .. }
    | CostItem::ReturnToHand { count, .. }
    | CostItem::TapPermanents { count, .. }) = item
    else {
        return None;
    };
    let Amount::Fixed(n) = count else { return None };
    Some((with_count(item, |_| 1), *n))
}

/// The parts that pay `items` together: the items of one kind and filter as one part taking all their permanents, in
/// the order they first come.
pub(crate) fn r6400_parts(items: &[CostItem]) -> Vec<Part> {
    let mut merged: Vec<(CostItem, i32)> = Vec::new();
    for item in items {
        let Some((one, n)) = unit(item) else { continue };
        match merged.iter_mut().find(|(seen, _)| *seen == one) {
            Some((_, total)) => *total = total.saturating_add(n),
            None => merged.push((one, n)),
        }
    }
    let cost = Cost(
        merged
            .iter()
            .filter(|(_, n)| *n > 0)
            .map(|(one, n)| with_count(one, |_| *n))
            .collect(),
    );
    Price::announced(&cost)
        .map(|price| price.parts)
        .unwrap_or_default()
}

/// What the pick of `part` cannot take: what earlier parts took and, for a part that taps, the creatures being declared
/// ("not declared as an attacking or blocking creature this combat").
fn barred(part: &Part, declared: &[ObjectId], picked: &[Vec<ObjectId>]) -> Vec<ObjectId> {
    let mut barred: Vec<ObjectId> = picked.iter().flatten().copied().collect();
    if part.taps() {
        barred.extend_from_slice(declared);
    }
    barred
}

/// Who pays a part: `player`, for the declaration of `source` (its first creature).
fn payer<'a>(
    state: &'a GameState,
    player: PlayerId,
    source: ObjectId,
    barred: &'a [ObjectId],
) -> Payer<'a> {
    Payer {
        state,
        player,
        source,
        barred,
        granter: None,
        pays: crate::r4707_pay_costs::PaysFor::Cost,
    }
}

/// Whether `player` can pay every part now: each part takes from what the parts before it would leave.
pub(crate) fn r6400_payable(
    state: &GameState,
    player: PlayerId,
    declared: &[ObjectId],
    parts: &[Part],
) -> bool {
    let Some(source) = declared.first().copied() else {
        return parts.is_empty();
    };
    let mut taken: Vec<Vec<ObjectId>> = Vec::new();
    for part in parts {
        let barred = barred(part, declared, &taken);
        match part.payable(&payer(state, player, source, &barred), &[]) {
            Some(objects) => taken.push(objects),
            None => return false,
        }
    }
    true
}

/// The next pick the parts need, `None` once every part is picked for; `Next::Drop` when one can no longer be paid.
pub(crate) fn r6400_next(
    state: &GameState,
    player: PlayerId,
    declared: &[ObjectId],
    parts: &[Part],
    (choices, done): (&mut Choices, &mut Done),
) -> Option<Next> {
    let source = declared.first().copied()?;
    let mut settled = 0;
    for part in parts {
        let Part::Pick(pick) = part else { continue };
        if settled < choices.cost_picks().len() {
            settled += 1;
            continue;
        }
        let barred = barred(part, declared, choices.cost_picks());
        let candidates = pick.candidates(&payer(state, player, source, &barred));
        if pick.short(state, &candidates) {
            return Some(Next::Drop);
        }
        if let Some(taken) = pick.unasked(state, &candidates) {
            choices.pick_for_cost(taken);
            settled += 1;
            continue;
        }
        let (min, max) = pick.bounds(candidates.len());
        done.asked = Some(Phase::Parts);
        return Some(Next::Ask(Decision {
            player,
            kind: DecisionKind::ChooseCards(CardsOptions {
                one_per_name: false,
                source,
                from: candidates,
                min,
                max,
                text: pick.text(),
                can_cancel: true,
                total: None,
                groups: Vec::new(),
                targeted: Box::default(),
            }),
        }));
    }
    None
}

/// The events that pay the parts with the objects picked for them, in order; `None` when a pick is missing.
pub(crate) fn r6400_events(
    state: &GameState,
    player: PlayerId,
    declared: &[ObjectId],
    parts: &[Part],
    choices: &Choices,
) -> Option<Vec<Event>> {
    let Some(source) = declared.first().copied() else {
        return parts.is_empty().then(Vec::new);
    };
    let mut picks = choices.cost_picks().iter();
    let mut events = Vec::new();
    for part in parts {
        let picked = if matches!(part, Part::Pick(_)) {
            picks.next()?.as_slice()
        } else {
            &[]
        };
        events.extend(part.events(state, player, source, picked));
    }
    Some(events)
}
