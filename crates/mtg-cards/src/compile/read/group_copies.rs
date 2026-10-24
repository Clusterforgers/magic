//! A group that becomes a copy of one target: "Each other creature becomes a copy of target nonlegendary creature
//! until end of turn." and "Choose target creature you control. Each other creature becomes a copy of that creature
//! until end of turn, except it isn't legendary." (`Effect::BecomeCopy` of an `Each` group without the target), read
//! from the wording and written back the same way.

use super::copies::{self, Subject};
use super::copy_spells::{chosen_noun, permanent_target};
use super::selectors::{Form, phrase, phrase_text, target};
use super::tokens::Reader;
use crate::effect::Effect;
use crate::modify::Duration;
use crate::refs::{Pred, Ref, Selector, Targetable};

/// The predicate that leaves the target out of "each other creature".
fn not_the_target() -> Pred {
    Pred::Not(Box::new(Pred::Is(Ref::Target(0))))
}

/// "Each other creature becomes a copy of target nonlegendary creature until end of turn.", also after a sentence of
/// its own that chooses the target ("Choose target creature you control.") and with the exceptions of the copy.
pub fn group_becomes_copy(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let chosen = r
        .attempt(|r| {
            r.need("choose")?;
            target(r)?;
            r.punct('.').then_some(())
        })
        .is_some();
    r.need("each other")?;
    let mut group = phrase(r, Form::Each)?;
    group.0.push(not_the_target());
    r.need("becomes a copy of")?;
    let of = if chosen {
        let Targetable::Object(selector) = &r.targets.first()?.what else {
            return None;
        };
        let noun = chosen_noun(selector)?;
        r.need("that")?;
        r.need(noun)?;
        Ref::Target(0)
    } else {
        permanent_target(r)?
    };
    r.need("until end of turn")?;
    let except = r
        .attempt(|r| {
            r.punct(',').then_some(())?;
            copies::exceptions(r, Subject::It)
        })
        .unwrap_or_default();
    Some(vec![Effect::BecomeCopy {
        what: Ref::Each(group),
        of,
        except,
        duration: Duration::UntilEndOfTurn,
    }])
}

/// Whether the effect is a group that becomes a copy of the target, which the card words in one sentence or two.
pub(super) fn is_group_copy(effect: &Effect) -> bool {
    matches!(
        effect,
        Effect::BecomeCopy {
            what: Ref::Each(group),
            of: Ref::Target(0),
            ..
        } if group.0.last() == Some(&not_the_target())
    )
}

/// The sentences of a group that becomes a copy, without their first capitals and periods: one that names the target
/// in place, or (`chosen`) one that chooses it and one that calls it "that creature". `target` is the text of the
/// clause's target and `kind` what it may be.
pub(super) fn texts(
    effect: &Effect,
    target: &str,
    kind: &Targetable,
    chosen: bool,
) -> Option<Vec<String>> {
    let Effect::BecomeCopy {
        what: Ref::Each(group),
        of: Ref::Target(0),
        except,
        duration: Duration::UntilEndOfTurn,
    } = effect
    else {
        return None;
    };
    let (last, rest) = group.0.split_last()?;
    (*last == not_the_target()).then_some(())?;
    let each = phrase_text(&Selector(rest.to_vec()), Form::Each)?;
    let tail = if except.is_empty() {
        String::new()
    } else {
        format!(", {}", copies::exceptions_text(except, Subject::It)?)
    };
    if !chosen {
        return Some(vec![format!(
            "each other {each} becomes a copy of {target} until end of turn{tail}"
        )]);
    }
    let Targetable::Object(selector) = kind else {
        return None;
    };
    Some(vec![
        format!("choose {target}"),
        format!(
            "each other {each} becomes a copy of that {} until end of turn{tail}",
            chosen_noun(selector)?
        ),
    ])
}
