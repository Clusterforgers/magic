//! A token copy made more times when the spell was kicked: "Create a token that's a copy of target creature. If this
//! spell was kicked, create five of those tokens instead." (`Effect::If` of `Condition::Kicked` over two
//! `CreateToken` of the same `CopyOf`), read from the wording and written back the same way.

use super::amounts::{count_word, number_word};
use super::copies::token_copy;
use super::tokens::Reader;
use crate::effect::{Effect, TokenRef};
use crate::value::{Amount, Condition};

/// "Create a token that's a copy of target creature. If this spell was kicked, create five of those tokens instead."
pub fn kicked_copies(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let one = single_copy(token_copy(r)?)?;
    r.punct('.').then_some(())?;
    r.need("if")?;
    r.source().then_some(())?;
    r.need("was kicked")?;
    r.punct(',').then_some(())?;
    r.need("create")?;
    let many = number_word(r.take_word()?).filter(|number| *number > 1)?;
    r.need("of those tokens instead")?;
    let mut more = one.clone();
    if let Effect::CreateToken { count, .. } = &mut more {
        *count = Amount::Fixed(i32::try_from(many).ok()?);
    }
    Some(vec![Effect::If {
        condition: Condition::Kicked,
        then: vec![more],
        otherwise: vec![one],
    }])
}

/// The one token copy a sentence made, with nothing done to it afterwards.
fn single_copy(effects: Vec<Effect>) -> Option<Effect> {
    match <[Effect; 1]>::try_from(effects) {
        Ok(
            [
                copy @ Effect::CreateToken {
                    token: TokenRef::CopyOf { .. },
                    bind: None,
                    ..
                },
            ],
        ) if is_one(&copy) => Some(copy),
        _ => None,
    }
}

/// Whether the effect makes one token.
fn is_one(effect: &Effect) -> bool {
    matches!(effect, Effect::CreateToken { count, .. } if *count == Amount::one())
}

/// The effects that are one copy and the same copy made more times when the spell was kicked.
pub(super) fn is_kicked_copy(effect: &Effect) -> Option<(&Effect, &Effect, u32)> {
    let Effect::If {
        condition: Condition::Kicked,
        then,
        otherwise,
    } = effect
    else {
        return None;
    };
    let ([more], [one]) = (then.as_slice(), otherwise.as_slice()) else {
        return None;
    };
    let (
        Effect::CreateToken {
            token: made,
            count: Amount::Fixed(many),
            ..
        },
        Effect::CreateToken { token: base, .. },
    ) = (more, one)
    else {
        return None;
    };
    let same = made == base && is_one(one) && matches!(base, TokenRef::CopyOf { .. });
    (same && *many > 1).then_some((one, more, u32::try_from(*many).ok()?))
}

/// The sentence that makes the copies more times, without its first capital and period.
pub(super) fn more_text(many: u32) -> Option<String> {
    Some(format!(
        "if this spell was kicked, create {} of those tokens instead",
        count_word(many)?
    ))
}
