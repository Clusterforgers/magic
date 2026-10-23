//! Divided damage: "~ deals 5 damage divided as you choose among any number of targets" (a target specification with
//! a `divide` amount and a `Divided` amount of damage).

use super::{Form, Writer, count_word, phrase_text, points_text};
use crate::effect::Effect;
use crate::refs::{Bounds, Ref, TargetSpec, Targetable};
use crate::value::Amount;

impl Writer<'_> {
    /// "~ deals 3 damage divided as you choose among one, two, or three targets".
    pub(super) fn divided(&mut self, effect: &Effect) -> Option<String> {
        let Effect::DealDamage {
            source: None,
            to: Ref::Target(0),
            amount: Amount::Divided,
        } = effect
        else {
            return None;
        };
        if self.named {
            return None;
        }
        let spec = self.targets.first()?;
        let total = points_text(spec.divide.as_ref()?)?;
        let among = among_text(spec)?;
        self.named = true;
        Some(format!(
            "~ deals {total} damage divided as you choose among {among}"
        ))
    }
}

/// The targets the damage is divided among: "one or two targets", "any number of target creatures and/or planeswalkers
/// your opponents control", "up to X target creatures".
fn among_text(spec: &TargetSpec) -> Option<String> {
    let targets = match &spec.what {
        Targetable::AnyTarget => "targets".to_owned(),
        Targetable::Object(selector) => format!(
            "target {}",
            phrase_text(selector, Form::Targets)?.replacen(" or ", " and/or ", 1)
        ),
        _ => return None,
    };
    let Bounds { min, max } = &spec.bounds;
    let how_many = match (min, max) {
        (0 | 1, None) => "any number of".to_owned(),
        (1, Some(Amount::Fixed(last))) if *last >= 2 => listed(u32::try_from(*last).ok()?)?,
        (0, Some(limit)) => format!("up to {}", limit_word(limit)?),
        _ => return None,
    };
    Some(format!("{how_many} {targets}"))
}

/// "one or two", "one, two, or three": every number from one to the last.
fn listed(last: u32) -> Option<String> {
    let words: Vec<&str> = (1..=last).map(count_word).collect::<Option<_>>()?;
    Some(match words.as_slice() {
        [first, second] => format!("{first} or {second}"),
        [rest @ .., last] => format!("{}, or {last}", rest.join(", ")),
        [] => return None,
    })
}

/// The most targets as a word or X.
fn limit_word(limit: &Amount) -> Option<String> {
    match limit {
        Amount::Fixed(count) => count_word(u32::try_from(*count).ok()?).map(str::to_owned),
        Amount::X => Some("X".to_owned()),
        _ => None,
    }
}
