//! Steps of a conditional effect that the renderers write only in another context: "untap that land" (the card an
//! earlier step found), "put a +1/+1 counter on a creature you control" (one chosen among a group), "you may cast a
//! Hero spell from your hand without paying its mana cost" and "exile ~ and each artifact and creature your opponents
//! control".

use super::super::super::counters::kind_text;
use super::super::super::selectors::{Form, phrase_text};
use super::super::super::triggers::article;
use crate::effect::Effect;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::value::Amount;
use serde_json::Value;

/// The wordings of `steps` after `before`, opening in lower case without a period.
pub(super) fn w4a_step_texts(before: &[Effect], steps: &[Effect]) -> Vec<String> {
    match steps {
        [
            Effect::Untap {
                what: Ref::Bound(name),
            },
        ] => w4a_bound_noun(before, name)
            .map(|noun| format!("untap that {noun}"))
            .into_iter()
            .collect(),
        [
            Effect::Choose {
                who: Ref::You,
                from: Ref::Each(group),
                bind,
                rest_as: None,
                count,
            },
            Effect::PutCounters {
                on,
                counter,
                count: one,
            },
        ] if *on == Ref::Bound(bind.clone())
            && one.is_one()
            && count.max.as_ref().is_some_and(Amount::is_one)
            && count.min.is_one() =>
        {
            let (Some(kind), Some(counter)) =
                (phrase_text(group, Form::Subject), kind_text(counter))
            else {
                return Vec::new();
            };
            vec![format!(
                "put {} {counter} counter on {} {kind}",
                article(counter),
                article(&kind)
            )]
        }
        [
            Effect::CastFromZone {
                who: Ref::You,
                what: Ref::Each(group),
                free: true,
                play: false,
                way: None,
                bind: None,
                otherwise,
            },
        ] if otherwise.is_empty() => w4a_free_from_hand(group).into_iter().collect(),
        [
            Effect::Exile {
                what: Ref::This,
                bind: None,
                duration: None,
            },
            Effect::Exile {
                what: Ref::Each(group),
                bind: None,
                duration: None,
            },
        ] => w4a_each_texts(group)
            .iter()
            .map(|kind| format!("exile ~ and each {kind}"))
            .collect(),
        _ => Vec::new(),
    }
}

/// "you may cast a Hero spell from your hand without paying its mana cost": a cast of a kind of card in the hand.
fn w4a_free_from_hand(group: &Selector) -> Option<String> {
    let in_hand = [Pred::In(ZoneKind::Hand), Pred::OwnedBy(Ref::You)];
    let kind: Vec<Pred> = group
        .0
        .iter()
        .filter(|pred| !in_hand.contains(pred))
        .cloned()
        .collect();
    if kind.len() + in_hand.len() != group.0.len() || kind.is_empty() {
        return None;
    }
    let kind = phrase_text(&Selector(kind), Form::Subject)?;
    if kind.contains(' ') {
        return None;
    }
    Some(format!(
        "you may cast {} {kind} spell from your hand without paying its mana cost",
        article(&kind)
    ))
}

/// The noun of the card an earlier step bound as `name`: "land" for a search for a basic land card.
fn w4a_bound_noun(before: &[Effect], name: &str) -> Option<String> {
    before.iter().find_map(|effect| {
        let value = serde_json::to_value(effect).ok()?;
        let (_, body) = value.as_object()?.iter().next()?;
        if body.get("as").and_then(Value::as_str) != Some(name) {
            return None;
        }
        let types: Vec<&str> = body
            .get("filter")?
            .as_array()?
            .iter()
            .filter_map(|pred| pred.get("Type").and_then(Value::as_str))
            .collect();
        match types.as_slice() {
            [one] => Some(one.to_lowercase()),
            _ => None,
        }
    })
}

/// The group after "each": "artifact or creature your opponents control", and for one of several kinds also "artifact
/// and creature your opponents control", as the oracle joins them after "each".
fn w4a_each_texts(group: &Selector) -> Vec<String> {
    let Some(kind) = phrase_text(group, Form::Each) else {
        return Vec::new();
    };
    match group.0.first() {
        Some(Pred::AnyOf(_)) => vec![kind.replacen(" or ", " and ", 1), kind],
        _ => vec![kind],
    }
}
