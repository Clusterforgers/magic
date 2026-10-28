//! Sentences of counters the renderers have no wording for, each derived from the counters, amounts and objects of the
//! effects: "remove all counters from target permanent" (and "... from all permanents and exile all tokens"),
//! "distribute two +1/+1 counters among one or two target creatures", "put a +1/+1 counter on each of up to two
//! Soldiers you control", "Put four +1/+1 counters, a flying counter, ... and a lifelink counter on that creature" and
//! "put two +1/+1 counters on each creature you control that's a Zombie or Mutant".

use super::super::super::amounts::count_word;
use super::super::super::counters::kind_text;
use super::super::super::selectors::{Form, phrase_text, spec_text};
use super::super::super::triggers::article;
use super::super::{capitalize, variants};
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{Bounds, Pred, Ref, Selector, TargetSpec};
use crate::value::{Amount, CounterKind};
use mtg_core::card::Subtype;

/// "a +1/+1 counter", "four +1/+1 counters", "X +1/+1 counters".
pub(super) fn w4e_counter_phrase(count: &Amount, counter: &CounterKind) -> Option<String> {
    let kind = kind_text(counter)?;
    match count {
        Amount::Fixed(1) => Some(format!("{} {kind} counter", article(kind))),
        Amount::X => Some(format!("X {kind} counters")),
        other => Some(format!("{} {kind} counters", count_word(other.fixed()?)?)),
    }
}

/// "one or two", "one, two, or three": the numbers a target count from one to `most` allows.
pub(super) fn w4e_one_to(bounds: &Bounds) -> Option<String> {
    if bounds.min != Amount::Fixed(1) {
        return None;
    }
    let most = bounds.max.as_ref()?.fixed()?;
    let words: Vec<&str> = (1..=most).map(count_word).collect::<Option<_>>()?;
    match words.as_slice() {
        [one, two] => Some(format!("{one} or {two}")),
        [first @ .., last] if first.len() >= 2 => Some(format!("{}, or {last}", first.join(", "))),
        _ => None,
    }
}

/// The sentences of the counter steps, without their period, each opening in lower case.
pub(super) fn sentences(targets: &[TargetSpec], effects: &[Effect]) -> Vec<String> {
    let mut found = Vec::new();
    found.extend(removed_all(targets, effects));
    found.extend(divided(targets, effects));
    found.extend(each_of_up_to(targets, effects));
    found.extend(listed(targets, effects));
    found
}

/// "remove all counters from target permanent", "remove all counters from all permanents and exile all tokens".
fn removed_all(targets: &[TargetSpec], effects: &[Effect]) -> Option<String> {
    let [
        Effect::RemoveCounters {
            from,
            counter: None,
            count: Amount::AnyCounters(counted),
            up_to: false,
        },
        rest @ ..,
    ] = effects
    else {
        return None;
    };
    if counted != from {
        return None;
    }
    let object = match from {
        Ref::Target(0) if targets.len() == 1 => spec_text(&targets[0])?,
        Ref::Each(selector) if targets.is_empty() => {
            format!("all {}", phrase_text(selector, Form::All)?)
        }
        _ => return None,
    };
    let removed = format!("remove all counters from {object}");
    if rest.is_empty() {
        return Some(removed);
    }
    let then = Ability::Spell {
        targets: Vec::new(),
        effects: rest.to_vec(),
    };
    let next = variants(&then).into_iter().find_map(|text| {
        let sentence = text.strip_suffix('.')?;
        (!sentence.contains('.')).then(|| lower_first(sentence))
    })?;
    Some(format!("{removed} and {next}"))
}

/// "distribute two +1/+1 counters among one or two target creatures".
fn divided(targets: &[TargetSpec], effects: &[Effect]) -> Option<String> {
    let [
        Effect::PutCounters {
            on: Ref::Target(0),
            counter,
            count: Amount::Divided,
        },
    ] = effects
    else {
        return None;
    };
    let [spec] = targets else { return None };
    let total = w4e_counter_phrase(spec.divide.as_ref()?, counter)?;
    let crate::refs::Targetable::Object(selector) = &spec.what else {
        return None;
    };
    let many = w4e_one_to(&spec.bounds)?;
    Some(format!(
        "distribute {total} among {many} target {}",
        phrase_text(selector, Form::Targets)?
    ))
}

/// "put a +1/+1 counter on each of up to two Soldiers you control".
fn each_of_up_to(targets: &[TargetSpec], effects: &[Effect]) -> Option<String> {
    let [
        Effect::Choose {
            who: Ref::You,
            from: Ref::Each(chosen),
            count:
                Bounds {
                    min: Amount::Fixed(0),
                    max: Some(Amount::Fixed(most)),
                },
            bind,
            rest_as: None,
        },
        Effect::PutCounters {
            on: Ref::Bound(on),
            counter,
            count,
        },
    ] = effects
    else {
        return None;
    };
    if on != bind || !targets.is_empty() {
        return None;
    }
    let most = count_word(u32::try_from(*most).ok()?)?;
    Some(format!(
        "put {} on each of up to {most} {}",
        w4e_counter_phrase(count, counter)?,
        phrase_text(chosen, Form::Targets)?
    ))
}

/// "Put four +1/+1 counters, a flying counter, and a lifelink counter on target creature" and "choose target
/// creature. Put ... on that creature": several kinds of counters on one target.
fn listed(targets: &[TargetSpec], effects: &[Effect]) -> Vec<String> {
    let phrases: Option<Vec<String>> = effects
        .iter()
        .map(|effect| match effect {
            Effect::PutCounters {
                on: Ref::Target(0),
                counter,
                count,
            } => w4e_counter_phrase(count, counter),
            _ => None,
        })
        .collect();
    let (Some(phrases), [spec]) = (phrases, targets) else {
        return Vec::new();
    };
    let Some((final_one, first)) = phrases.split_last() else {
        return Vec::new();
    };
    let (Some(target), true) = (spec_text(spec), first.len() >= 2) else {
        return Vec::new();
    };
    let list = format!("{}, and {final_one}", first.join(", "));
    let mut found = vec![format!("put {list} on {target}")];
    if let Some(noun) = target.strip_prefix("target ") {
        found.push(format!(
            "choose {target}. {} on that {noun}",
            capitalize(&format!("put {list}"))
        ));
    }
    found
}

/// The wordings of a put of counters on each of a group with "that's a Zombie or Mutant" for a choice of subtypes.
pub(super) fn subtype_choice(ability: &Ability) -> Vec<String> {
    let mut plain = ability.clone();
    let (Ability::Spell { effects, .. }
    | Ability::Activated { effects, .. }
    | Ability::Triggered { effects, .. }) = &mut plain
    else {
        return Vec::new();
    };
    let [
        Effect::PutCounters {
            on: Ref::Each(Selector(preds)),
            ..
        },
    ] = effects.as_mut_slice()
    else {
        return Vec::new();
    };
    let Some(at) = preds.iter().position(|pred| matches!(pred, Pred::AnyOf(_))) else {
        return Vec::new();
    };
    let Pred::AnyOf(kinds) = preds.remove(at) else {
        return Vec::new();
    };
    let names: Option<Vec<&str>> = kinds
        .iter()
        .map(|pred| match pred {
            Pred::Subtype(Subtype(name)) => Some(name.as_str()),
            _ => None,
        })
        .collect();
    let (Some([one, two]), Some(group)) = (
        names.as_deref(),
        phrase_text(&Selector(preds.clone()), Form::Each),
    ) else {
        return Vec::new();
    };
    let said = format!("on each {group}");
    let into = format!("{said} that's {} {one} or {two}", article(one));
    variants(&plain)
        .iter()
        .filter(|text| text.contains(&said))
        .map(|text| text.replacen(&said, &into, 1))
        .collect()
}

/// The sentence with its first letter in lower case.
pub(super) fn lower_first(sentence: &str) -> String {
    let mut chars = sentence.chars();
    chars
        .next()
        .map(|first| first.to_lowercase().chain(chars).collect())
        .unwrap_or_default()
}
