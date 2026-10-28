//! The change an activated ability prints of its own cost after its effects: "This ability costs {1} less to activate for
//! each instant and sorcery card in your graveyard.", "This ability costs {4} less to activate if an opponent controls
//! four or more nonland permanents.", "This ability costs {X} less to activate, where X is the total power of creatures
//! you control." It is a `CostItem::Changed` at the end of the ability's cost, read with the counts and
//! conditions of a spell's cost reduction and written back the same way.

use super::cost_conditions;
use super::render;
use super::selectors::{Form, card_type, phrase, phrase_text, type_name};
use super::statics::{cost_mana, cost_sentences, cost_times};
use super::tokens::Reader;
use crate::ability::{Ability, CostChange};
use crate::cost::{Cost, CostItem};
use crate::refs::{Pred, Selector};
use crate::value::Amount;

/// Whether the reader stands at the sentence of the reduction.
pub(super) fn c11a_at_less(r: &mut Reader<'_>) -> bool {
    r.lookahead(|r| r.words("this ability costs").then_some(()))
        .is_some()
}

/// "This ability costs {1} less to activate ..." (or "more"), with its closing period.
pub(super) fn c11a_less(r: &mut Reader<'_>) -> Option<CostItem> {
    r.words("this ability costs").then_some(())?;
    let (mana, x) = cost_mana(r)?;
    let change = if r.word("less") {
        CostChange::Reduce
    } else if r.word("more") {
        CostChange::Increase
    } else {
        return None;
    };
    r.need("to activate")?;
    let per = match r.attempt(c11a_two_types) {
        Some(per) if !x => per,
        _ => cost_times(r, false, x)?,
    };
    let condition = if per.is_one() {
        r.attempt(cost_conditions::trailing)
    } else {
        None
    };
    r.sentence_end().then_some(CostItem::Changed {
        change,
        mana,
        per,
        condition,
    })
}

/// "for each instant and sorcery card in your graveyard": a card of either type, counted where the phrase places it.
fn c11a_two_types(r: &mut Reader<'_>) -> Option<Amount> {
    r.need("for each")?;
    let first = card_type(r.take_word()?)?;
    r.word("and").then_some(())?;
    let second = card_type(r.take_word()?)?;
    let placed = phrase(r, Form::Count)?;
    (!placed.0.is_empty()
        && placed
            .0
            .iter()
            .all(|pred| matches!(pred, Pred::In(_) | Pred::OwnedBy(_))))
    .then_some(())?;
    let mut counted = vec![Pred::AnyOf(vec![Pred::Type(first), Pred::Type(second)])];
    counted.extend(placed.0);
    Some(Amount::Count(Selector(counted)))
}

/// The words after "for each" of a count [`c11a_two_types`] reads; `None` for any other count.
fn c11a_two_types_text(per: &Amount) -> Option<String> {
    let Amount::Count(counted) = per else {
        return None;
    };
    let (Pred::AnyOf(kinds), placed) = counted.0.split_first()? else {
        return None;
    };
    let [Pred::Type(first), Pred::Type(second)] = kinds.as_slice() else {
        return None;
    };
    let place = phrase_text(&Selector(placed.to_vec()), Form::Count)?;
    Some(format!(
        "{} and {} {place}",
        type_name(*first),
        type_name(*second)
    ))
}

/// The wordings of the change, with its closing period.
fn c11a_less_texts(item: &CostItem) -> Vec<String> {
    let CostItem::Changed {
        change,
        mana,
        per,
        condition,
    } = item
    else {
        return Vec::new();
    };
    if let Some(counted) = c11a_two_types_text(per) {
        let more = match change {
            CostChange::Increase => "more",
            CostChange::Reduce => "less",
        };
        return vec![format!(
            "This ability costs {mana} {more} to activate for each {counted}."
        )];
    }
    cost_sentences(
        "This ability",
        &Selector::this(),
        *change,
        mana,
        per,
        condition.as_ref(),
    )
    .into_iter()
    .filter(|sentence| sentence.starts_with("This ability"))
    .map(|sentence| format!("{}.", sentence.replacen(" to cast", " to activate", 1)))
    .collect()
}

/// The wordings of an activated ability with a reduction of its own: the ability without it, the reduction after the
/// effects and before "Activate only ..."; `None` for any other ability.
pub(super) fn c11a_variants(ability: &Ability) -> Option<Vec<String>> {
    let Ability::Activated { cost, .. } = ability else {
        return None;
    };
    let (less, paid): (Vec<&CostItem>, Vec<&CostItem>) = cost
        .0
        .iter()
        .partition(|item| matches!(item, CostItem::Changed { .. }));
    let [less] = less.as_slice() else {
        // Without a reduction the other writers word it; two have no wording.
        return (!less.is_empty()).then(Vec::new);
    };
    let mut bare = ability.clone();
    if let Ability::Activated { cost, .. } = &mut bare {
        *cost = Cost(paid.into_iter().cloned().collect());
    }
    let sentences = c11a_less_texts(less);
    let texts = render::variants(&bare)
        .into_iter()
        .flat_map(|text| {
            let at = text.find(" Activate only ").unwrap_or(text.len());
            let (body, tail) = text.split_at(at);
            sentences
                .iter()
                .map(|sentence| format!("{body} {sentence}{tail}"))
                .collect::<Vec<_>>()
        })
        .collect();
    Some(texts)
}
