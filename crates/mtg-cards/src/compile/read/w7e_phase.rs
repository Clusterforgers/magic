//! Phase-out sentences (`Effect::PhaseOut`): "target creature phases out", "any number of target creatures you control
//! phase out", "all permanents you control phase out", "~ phases out". The writer's side is an arm of the renderer.

use super::effects::{object, subject};
use super::tokens::Reader;
use crate::effect::Effect;
use crate::keyword::Keyword;
use crate::modify::{Duration, GameRule};
use crate::refs::{Ref, Selector};

/// "<one permanent> phases out." or "<several permanents> phase out.": what the subject names phases out.
pub fn w7e_phase_out(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let what = match r.attempt(subject) {
        Some((what, _)) => what,
        None => object(r)?,
    };
    let verb = if w7e_one(r, &what) {
        "phases out"
    } else {
        "phase out"
    };
    r.need(verb)?;
    Some(vec![Effect::PhaseOut { what }])
}

/// The sentence of Teferi's Protection, as the reader takes it and the writer gives it back.
const W7E_PROTECTED: &str =
    "until your next turn, your life total can't change and you gain protection from everything";

/// "Until your next turn, your life total can't change and you gain protection from everything.": two rules of the
/// controller until their next turn (CR 119.7, 702.16).
pub fn w7e_protected(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("until your next turn")?;
    r.punct(',').then_some(())?;
    r.need("your life total can't change and you gain protection from everything")?;
    Some(w7e_protected_effects())
}

/// "All permanents you control phase out, and until your next turn, your life total can't change and you gain
/// protection from everything." (Perch Protection): a phase-out and the two rules in one sentence.
pub fn w7e_phased_and_protected(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let mut effects = w7e_phase_out(r)?;
    r.punct(',').then_some(())?;
    r.need("and")?;
    effects.extend(w7e_protected(r)?);
    Some(effects)
}

/// The writer of a group phase-out joined to the two rules: "all permanents you control phase out, and until ...".
pub fn w7e_phased_and_protected_text(effects: &[Effect]) -> Option<(String, usize)> {
    let [
        Effect::PhaseOut {
            what: Ref::Each(selector),
        },
        rest @ ..,
    ] = effects
    else {
        return None;
    };
    let (protected, _) = w7e_protected_text(rest)?;
    let group = super::selectors::phrase_text(selector, super::selectors::Form::All)?;
    Some((format!("all {group} phase out, and {protected}"), 3))
}

/// The two rules of "until your next turn, your life total can't change and you gain protection from everything".
fn w7e_protected_effects() -> Vec<Effect> {
    let duration = Duration::UntilYourNextTurn;
    vec![
        Effect::Rule {
            rule: GameRule::LifeTotalCantChange(Ref::You),
            duration,
        },
        Effect::Rule {
            rule: GameRule::PlayersHave {
                who: Ref::You,
                keyword: Keyword::Protection(Selector::default()),
            },
            duration,
        },
    ]
}

/// The writer of the two rules: the sentence and the two effects it takes, or nothing when the effects begin otherwise.
pub fn w7e_protected_text(effects: &[Effect]) -> Option<(String, usize)> {
    let pair = effects.get(..2)?;
    (pair == w7e_protected_effects().as_slice()).then(|| (W7E_PROTECTED.to_owned(), 2))
}

/// Whether the subject is one permanent: the card itself or a single target.
fn w7e_one(r: &Reader<'_>, what: &Ref) -> bool {
    match what {
        Ref::This => true,
        Ref::Target(_) => r.targets.first().is_some_and(|spec| spec.bounds.is_one()),
        _ => false,
    }
}
