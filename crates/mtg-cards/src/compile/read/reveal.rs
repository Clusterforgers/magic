//! Looking at the top card and revealing it: "Look at the top card of your library. You may reveal that card. If an
//! instant or sorcery card is revealed this way, transform ~." is a `LookAtTop` and a `May` of the `Reveal` and an
//! `If` on the type of the card, three sentences read as one shape because the effects after the reveal happen only
//! when the card was revealed.

use super::conditional::consequence;
use super::library::{TOP, filter, filter_text};
use super::tokens::Reader;
use super::triggers::article;
use crate::effect::Effect;
use crate::refs::Ref;
use crate::value::{Amount, Condition};

/// "Look at the top card of your library. You may reveal that card. If a land card is revealed this way, ...": the
/// card is looked at, the player may reveal it, and the effects follow when it is the kind named.
pub fn look_reveal(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("look at the top card of your library")?;
    r.punct('.').then_some(())?;
    r.need("you may reveal")?;
    (r.word("it") || r.words("that card")).then_some(())?;
    r.punct('.').then_some(())?;
    r.need("if")?;
    (r.word("a") || r.word("an")).then_some(())?;
    let selector = filter(r, false)?;
    r.need("is revealed this way")?;
    r.punct(',').then_some(())?;
    let top = Ref::Bound(TOP.to_owned());
    let condition = Condition::Matches {
        subject: top.clone(),
        selector,
    };
    let then = consequence(r, &condition)?;
    Some(vec![
        Effect::LookAtTop {
            who: Ref::You,
            count: Amount::one(),
            bind: TOP.to_owned(),
        },
        Effect::May {
            who: Ref::You,
            effects: vec![
                Effect::Reveal { what: top },
                Effect::If {
                    condition,
                    then,
                    otherwise: Vec::new(),
                },
            ],
        },
    ])
}

/// The condition and the effects of a look at the top card with the reveal after it, at the front of the list, and
/// how many effects it takes.
pub fn shape(effects: &[Effect]) -> Option<(&Condition, &[Effect], usize)> {
    let top = Ref::Bound(TOP.to_owned());
    let [
        Effect::LookAtTop {
            who: Ref::You,
            count,
            bind,
        },
        Effect::May {
            who: Ref::You,
            effects: inner,
        },
        ..,
    ] = effects
    else {
        return None;
    };
    let [
        Effect::Reveal { what },
        Effect::If {
            condition:
                condition @ Condition::Matches {
                    subject,
                    selector: _,
                },
            then,
            otherwise,
        },
    ] = inner.as_slice()
    else {
        return None;
    };
    let plain = *count == Amount::one()
        && bind == TOP
        && *what == top
        && *subject == top
        && otherwise.is_empty()
        && !then.is_empty();
    plain.then_some((condition, then.as_slice(), 2))
}

/// The three sentences of the shape, with the effects that follow the condition written as `tail`.
pub fn text(condition: &Condition, tail: &str) -> Option<String> {
    let Condition::Matches { selector, .. } = condition else {
        return None;
    };
    let kind = filter_text(selector, false)?;
    Some(format!(
        "look at the top card of your library. You may reveal that card. If {} {kind} is revealed this way, {tail}",
        article(&kind)
    ))
}
