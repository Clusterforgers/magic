//! r47-07 wordings of every permanent of a kind sacrificed by its own controller: "{1}, {T}: Each nontoken permanent
//! with a name originally printed in the Antiquities expansion is sacrificed by its controller." (Golgothian Sylex),
//! written from the twin that destroys each of them, and the state trigger "Whenever one or more other nontoken
//! permanents with a name originally printed in the Arabian Nights expansion are on the battlefield, their controllers
//! sacrifice them." (City in a Bottle).

use super::super::super::selectors::{Form, phrase_text};
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{Pred, Ref, Selector};
use crate::trigger::TriggerPattern;
use crate::value::{Amount, Cmp, Condition};

/// Adds the r47-07 wordings of the ability.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    r4707_activated(ability, texts);
    r4707_state_trigger(ability, texts);
    r4707_one_or_more_energy(ability, texts);
}

/// "you may pay one or more {E}. If you do, create an X/X ... token ..., where X is the amount of {E} paid this way.": a
/// payment of X energy with X at least 1 whose one paid step makes X/X tokens, written from the twin that pays X life.
fn r4707_one_or_more_energy(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return;
    };
    let Some(steps) = value
        .as_object_mut()
        .and_then(|kinds| kinds.values_mut().next())
        .and_then(|body| body.get_mut("effects"))
        .and_then(serde_json::Value::as_array_mut)
    else {
        return;
    };
    let [only] = steps.as_mut_slice() else {
        return;
    };
    let Some(pay) = only
        .get_mut("Pay")
        .and_then(serde_json::Value::as_object_mut)
    else {
        return;
    };
    if pay.get("cost") != Some(&serde_json::json!([{"PayEnergy": "X"}, {"XAtLeast": 1}]))
        || pay.contains_key("unpaid")
        || pay.contains_key("who")
        || pay.contains_key("times")
    {
        return;
    }
    let sized = pay
        .get("paid")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|paid| {
            matches!(paid.as_slice(), [step] if step.pointer("/CreateToken/token/Sized/size") == Some(&serde_json::json!("X")))
        });
    if !sized {
        return;
    }
    pay.insert("cost".to_owned(), serde_json::json!([{"PayLife": "X"}]));
    let Ok(twin) = serde_json::from_value::<Ability>(value) else {
        return;
    };
    let found: Vec<String> = crate::compile::written_variants(&twin)
        .into_iter()
        .filter_map(|text| {
            let (head, tail) = text.split_once("you may pay X life. If you do, create an X/X ")?;
            let body = tail.strip_suffix('.')?;
            (!body.contains('.') && !body.contains(" X")).then(|| {
                format!(
                    "{head}you may pay one or more {{E}}. If you do, create an X/X {body}, where X is the amount of \
                     {{E}} paid this way."
                )
            })
        })
        .filter(|text| !texts.contains(text))
        .collect();
    texts.extend(found);
}

/// The permanents of `effects` when they are exactly "each player sacrifices every permanent they control that the
/// selector picks", none otherwise.
fn r4707_each_sacrificed(effects: &[Effect]) -> Option<&Selector> {
    let [
        Effect::RepeatEach {
            over: Ref::EachPlayer,
            bind,
            effects,
        },
    ] = effects
    else {
        return None;
    };
    let [
        Effect::Sacrifice {
            who: Ref::Bound(who),
            filter,
            count: Amount::Count(counted),
            bind: None,
        },
    ] = effects.as_slice()
    else {
        return None;
    };
    let mut theirs = filter.0.clone();
    theirs.push(Pred::ControlledBy(Ref::Bound(bind.clone())));
    (who == bind && counted.0 == theirs && !filter.0.is_empty()).then_some(filter)
}

/// "{1}, {T}: Each <permanent> is sacrificed by its controller.", from the twin that destroys each of them.
fn r4707_activated(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Activated { effects, .. } = ability else {
        return;
    };
    let Some(filter) = r4707_each_sacrificed(effects) else {
        return;
    };
    let Some(each) = phrase_text(filter, Form::Each) else {
        return;
    };
    let mut twin = ability.clone();
    if let Ability::Activated { effects, .. } = &mut twin {
        *effects = vec![Effect::Destroy {
            what: Ref::Each(filter.clone()),
            regenerate: true,
            bind: None,
        }];
    }
    let destroyed = format!("Destroy each {each}.");
    let sacrificed = format!("Each {each} is sacrificed by its controller.");
    let found: Vec<String> = crate::compile::written_variants(&twin)
        .into_iter()
        .filter(|text| text.matches(&destroyed).count() == 1)
        .map(|text| text.replace(&destroyed, &sacrificed))
        .filter(|text| !texts.contains(text))
        .collect();
    texts.extend(found);
}

/// "Whenever one or more <permanents> are on the battlefield, their controllers sacrifice them.": a state trigger on
/// one or more of them, which each of their controllers sacrifices.
fn r4707_state_trigger(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when:
            TriggerPattern::State {
                condition:
                    Condition::Compare {
                        left: Amount::Count(present),
                        op: Cmp::Ge,
                        right: Amount::Fixed(1),
                    },
            },
        intervening_if: None,
        limit: None,
        targets,
        effects,
        ..
    } = ability
    else {
        return;
    };
    let Some(filter) = r4707_each_sacrificed(effects) else {
        return;
    };
    if present != filter || !targets.is_empty() {
        return;
    }
    let Some(all) = phrase_text(filter, Form::All) else {
        return;
    };
    let text = format!(
        "Whenever one or more {all} are on the battlefield, their controllers sacrifice them."
    );
    if !texts.contains(&text) {
        texts.push(text);
    }
}
