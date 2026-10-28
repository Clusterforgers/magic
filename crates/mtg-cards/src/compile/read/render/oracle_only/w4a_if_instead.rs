//! A bigger number of the same tokens on a condition: "Create five 1/1 white Human creature tokens. If this spell was
//! cast from a graveyard, create ten of those tokens instead." An `If` whose two branches create the same token, each
//! a fixed number.

use super::super::super::facts::quantity_texts;
use crate::effect::Effect;
use crate::value::Amount;

/// "Z. If X, create N of those tokens instead": `others` are the wordings of the other branch.
pub(super) fn w4a_instead_texts(
    conditions: &[String],
    then: &[Effect],
    otherwise: &[Effect],
    others: &[String],
) -> Vec<String> {
    let (
        [
            Effect::CreateToken {
                count: Amount::Fixed(more),
                ..
            },
        ],
        [
            Effect::CreateToken {
                count: Amount::Fixed(fewer),
                ..
            },
        ],
    ) = (then, otherwise)
    else {
        return Vec::new();
    };
    if w4a_without_count(then) != w4a_without_count(otherwise) {
        return Vec::new();
    }
    let Ok(number) = u32::try_from(*more) else {
        return Vec::new();
    };
    if more == fewer {
        return Vec::new();
    }
    let mut out = Vec::new();
    for other in others {
        for condition in conditions {
            for words in quantity_texts(number) {
                out.push(format!(
                    "{other}. If {condition}, create {words} of those tokens instead"
                ));
            }
        }
    }
    out
}

/// The creations with the count left out.
fn w4a_without_count(effects: &[Effect]) -> Vec<Effect> {
    effects
        .iter()
        .map(|effect| {
            let mut plain = effect.clone();
            if let Effect::CreateToken { count, .. } = &mut plain {
                *count = Amount::one();
            }
            plain
        })
        .collect()
}
