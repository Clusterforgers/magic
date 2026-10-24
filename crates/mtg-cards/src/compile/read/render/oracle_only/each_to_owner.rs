//! A return of a group to the hands of their owners, worded for each object: "return each creature your opponents
//! control with toughness X or less to its owner's hand". The renderers write "return all creatures ... to their owners'
//! hands".

use super::super::{Form, phrase_text};
use crate::ability::Ability;
use crate::effect::{Destination, Effect};
use crate::refs::{Pred, Ref, Selector};
use crate::value::Amount;

/// The selector with each amount it compares with named X, as a sentence with "where X is" words it.
fn with_x(selector: &Selector) -> Selector {
    Selector(
        selector
            .0
            .iter()
            .map(|pred| match pred {
                Pred::Compare { stat, op, .. } => Pred::Compare {
                    stat: *stat,
                    op: *op,
                    to: Amount::X,
                },
                other => other.clone(),
            })
            .collect(),
    )
}

/// Adds the wordings of a bounce of a group with "each" and "its owner's hand".
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    for effect in ability.effects() {
        let Effect::Move {
            what: Ref::Each(selector),
            to: Destination::Hand,
        } = effect
        else {
            continue;
        };
        let selector = with_x(selector);
        let (Some(all), Some(each)) = (
            phrase_text(&selector, Form::All),
            phrase_text(&selector, Form::Each),
        ) else {
            continue;
        };
        let (from, into) = (format!("return all {all}"), format!("return each {each}"));
        let said: Vec<String> = texts
            .iter()
            .filter(|text| text.contains(&from) && text.contains(" to their owners' hands"))
            .map(|text| {
                text.replacen(&from, &into, 1).replacen(
                    " to their owners' hands",
                    " to its owner's hand",
                    1,
                )
            })
            .collect();
        texts.extend(said);
    }
}
