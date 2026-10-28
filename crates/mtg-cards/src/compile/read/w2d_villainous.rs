//! Villainous choices (CR 701.56): "that player faces a villainous choice — A, or B.", "target opponent faces ..." and
//! "each opponent faces ..." are a `VillainousChoice` (in a `RepeatEach` over the opponents for "each opponent"). In the
//! options "that player" and "they" name the player who faces the choice.

use serde_json::Value;

use super::conditional::{body, steps};
use super::tokens::Reader;
use crate::effect::Effect;
use crate::refs::{Bounds, Ref, TargetSpec, Targetable};
use crate::value::Condition;

/// The name the opponent of each pass is bound to in "each opponent faces a villainous choice".
pub const W2D_FACING: &str = "opponent";

/// The effects with `into` in the place of each `from`; `None` when they do not serialize back.
pub fn w2d_swap(effects: &[Effect], from: &Ref, into: &Ref) -> Option<Vec<Effect>> {
    let mut json = serde_json::to_value(effects).ok()?;
    let from = serde_json::to_value(from).ok()?;
    let into = serde_json::to_value(into).ok()?;
    w2d_replace(&mut json, &from, &into);
    serde_json::from_value(json).ok()
}

fn w2d_replace(value: &mut Value, from: &Value, into: &Value) {
    if value == from {
        value.clone_from(into);
        return;
    }
    match value {
        Value::Array(items) => items.iter_mut().for_each(|v| w2d_replace(v, from, into)),
        Value::Object(map) => map.values_mut().for_each(|v| w2d_replace(v, from, into)),
        _ => {}
    }
}

/// Who faces the choice: "that player" of the event, "target opponent" or "each opponent" (`Opponents`, one pass each).
fn w2d_subject(r: &mut Reader<'_>) -> Option<Ref> {
    if r.that_player && r.words("that player") {
        return Some(Ref::Triggering);
    }
    if r.words("target opponent") {
        let index = u8::try_from(r.targets.len()).ok()?;
        r.targets.push(TargetSpec {
            what: Targetable::Opponent,
            bounds: Bounds::default(),
            divide: None,
        });
        return Some(Ref::Target(index));
    }
    r.words("each opponent").then_some(Ref::Opponents)
}

/// One option, read with "that player" and "they" naming the player of the event, then put on `who`.
/// The first option ends at ", or" and the second at the end of the sentence.
fn w2d_option(r: &mut Reader<'_>, who: &Ref, last: bool) -> Option<Vec<Effect>> {
    let event = std::mem::replace(&mut r.that_player, true);
    let read = if last {
        body(r)
    } else {
        let before_or = |r: &mut Reader<'_>| {
            r.lookahead(|r| {
                r.punct(',').then_some(())?;
                r.need("or")
            })
        };
        r.attempt(|r| {
            let found = steps(r, &Condition::All(Vec::new()))?;
            before_or(r).map(|()| found)
        })
        .or_else(|| {
            r.attempt(|r| {
                let found = super::effects::one_sentence(r)?;
                before_or(r).map(|()| found)
            })
        })
    };
    r.that_player = event;
    let read = read?;
    if *who == Ref::Triggering {
        return Some(read);
    }
    // Where the event has a player of its own, "that player" would name two players.
    (!event).then_some(())?;
    w2d_swap(&read, &Ref::Triggering, who)
}

/// "S faces a villainous choice — A, or B."
pub fn w2d_villainous(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let subject = w2d_subject(r)?;
    r.need("faces a villainous choice")?;
    r.punct('\u{2014}').then_some(())?;
    let each = subject == Ref::Opponents;
    let who = if each {
        Ref::Bound(W2D_FACING.to_owned())
    } else {
        subject
    };
    let they = r.lookahead(|r| r.word("they").then_some(())).is_some();
    let first = w2d_option(r, &who, false)?;
    r.punct(',').then_some(())?;
    r.need("or")?;
    let second = w2d_option(r, &who, true)?;
    let choice = Effect::VillainousChoice {
        who,
        first,
        second,
        they,
    };
    Some(vec![if each {
        Effect::RepeatEach {
            over: Ref::Opponents,
            bind: W2D_FACING.to_owned(),
            effects: vec![choice],
        }
    } else {
        choice
    }])
}
