//! Trigger heads the other writers do not word: "Whenever an opponent discards a creature card", "Whenever a creature
//! attacks one of your opponents or a planeswalker an opponent controls", "When ~ becomes the target of a spell",
//! "Whenever ~ deals combat damage to a creature", "Whenever one or more time counters are removed from ~", "Whenever
//! a creature you control dies or is put into the command zone", "Whenever a source you control deals 5 or more damage
//! to a player". The ability is written with a stand-in event the checker words (the same subject, a plainer event)
//! and the event of each wording is swapped for the one derived from the trigger's data.

use serde_json::{Value, json};

use super::super::super::counters::kind_text;
use super::super::super::selectors::{Form, phrase_text, type_name};
use super::super::super::triggers::{article, pattern_text, player_verb, subject_text};
use super::w4b_stand::{self, json as to_json, swapped};
use crate::ability::Ability;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::trigger::TriggerPattern;

/// Adds the wordings of a triggered ability whose event, or event and intervening "if", only these writers word.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered { when, .. } = ability else {
        return;
    };
    let (stand, said, keep_if) = match damage_at_least(ability) {
        Some((stand, said)) => (stand, said, false),
        None if pattern_text(when).is_none() => {
            let Some((stand, said)) = event(when) else {
                return;
            };
            (stand, said, true)
        }
        None => return,
    };
    let Some(from) = pattern_text(&stand) else {
        return;
    };
    let Some(standing) = w4b_stand::stand(ability, |value| {
        let triggered = value.get_mut("Triggered")?;
        triggered["when"] = to_json(&stand);
        if !keep_if {
            triggered.as_object_mut()?.remove("if");
        }
        that_many(triggered);
        Some(())
    }) else {
        return;
    };
    let (Some(from), Some(to)) = (body(&from), body(&said)) else {
        return;
    };
    let mut found = swapped(&standing, &[(&format!("{from},"), &format!("{to},"))]);
    if by_spell(when) {
        // What triggered it is the spell that targets, not the permanent of the stand-in.
        found = found
            .iter()
            .map(|text| text.replace("that permanent's", "that spell's"))
            .collect();
    }
    texts.extend(that_object(when, to, &found));
    texts.extend(found);
}

/// Whether the event is a spell targeting the object.
fn by_spell(when: &TriggerPattern) -> bool {
    matches!(when, TriggerPattern::BecomesTarget { by, .. } if by.0.first() == Some(&Pred::Spell))
}

/// The counters an intervening "if it had one or more +1/+1 counters on it" asks for are "that many" counters in the
/// effects, which the checker words for `Amount::Triggering`.
fn that_many(triggered: &mut Value) {
    let Some(had) = triggered.pointer("/if/Compare").cloned() else {
        return;
    };
    let (Some(left), Some(1)) = (had.get("left"), had.get("right").and_then(Value::as_u64)) else {
        return;
    };
    let on_it = left.pointer("/Counters/on") == Some(&json!("Triggering"));
    if on_it
        && had.get("op") == Some(&json!("Ge"))
        && let Some(effects) = triggered.get_mut("effects")
    {
        replaced(effects, left, &json!("Triggering"));
    }
}

/// Each `from` in `value` replaced by `to`.
fn replaced(value: &mut Value, from: &Value, to: &Value) {
    if value == from {
        *value = to.clone();
        return;
    }
    match value {
        Value::Array(items) => items.iter_mut().for_each(|item| replaced(item, from, to)),
        Value::Object(fields) => fields
            .values_mut()
            .for_each(|item| replaced(item, from, to)),
        _ => {}
    }
}

/// The attacker named back as "that creature" where the stand-in's wording says "it".
fn that_object(when: &TriggerPattern, head: &str, texts: &[String]) -> Vec<String> {
    let TriggerPattern::AttacksPlayerOrPlaneswalker { what, .. } = when else {
        return Vec::new();
    };
    let [Pred::Type(kind)] = what.0.as_slice() else {
        return Vec::new();
    };
    let it = format!("{head}, it ");
    let that = format!("{head}, that {} ", type_name(*kind));
    texts
        .iter()
        .filter(|text| text.contains(&it))
        .map(|text| text.replacen(&it, &that, 1))
        .collect()
}

/// The event after "When" or "Whenever".
fn body(head: &str) -> Option<&str> {
    head.strip_prefix("Whenever ")
        .or_else(|| head.strip_prefix("When "))
}

/// The stand-in event the checker words and the head of the trigger as the card prints it.
pub(super) fn event(when: &TriggerPattern) -> Option<(TriggerPattern, String)> {
    match when {
        TriggerPattern::Discarded { what, by } if !what.is_empty() => Some((
            TriggerPattern::Discarded {
                what: Selector(Vec::new()),
                by: by.clone(),
            },
            format!(
                "Whenever {} {}",
                player_verb(by, "discard")?,
                card_text(what)?
            ),
        )),
        TriggerPattern::AttacksPlayerOrPlaneswalker {
            what,
            at: Ref::Opponents,
        } => Some((
            TriggerPattern::Attacks { what: what.clone() },
            format!(
                "Whenever {} attacks one of your opponents or a planeswalker an opponent controls",
                subject_text(what)?
            ),
        )),
        TriggerPattern::BecomesTarget { what, by } => targeted(what, by),
        TriggerPattern::DealsDamage {
            source,
            to: Some(Ref::Each(hit)),
            combat,
        } => {
            let stand = TriggerPattern::DealsDamage {
                source: source.clone(),
                to: None,
                combat: *combat,
            };
            let said = format!("{} to {}", pattern_text(&stand)?, subject_text(hit)?);
            Some((stand, said))
        }
        TriggerPattern::Batch(inner) => batch(inner),
        TriggerPattern::ZoneChange {
            what,
            from: from @ (None | Some(ZoneKind::Battlefield)),
            to: Some(ZoneKind::Command),
        } => Some((
            dies(what),
            format!(
                "Whenever {} is put into the command zone{}",
                subject_text(what)?,
                if from.is_none() { " from anywhere" } else { "" }
            ),
        )),
        TriggerPattern::ZoneChangeExcept {
            what,
            except: ZoneKind::Hand,
            to: Some(ZoneKind::Battlefield),
        } => Some((
            TriggerPattern::ZoneChange {
                what: what.clone(),
                from: None,
                to: Some(ZoneKind::Battlefield),
            },
            format!(
                "Whenever {} enters from anywhere other than your hand",
                subject_text(what)?
            ),
        )),
        TriggerPattern::AnyOf(parts) => any_of(parts),
        _ => None,
    }
}

/// The card dying, the stand-in of another move from the battlefield.
fn dies(what: &Selector) -> TriggerPattern {
    TriggerPattern::ZoneChange {
        what: what.clone(),
        from: Some(ZoneKind::Battlefield),
        to: Some(ZoneKind::Graveyard),
    }
}

/// "When ~ becomes the target of a spell", "... of a spell an opponent controls".
fn targeted(what: &Selector, by: &Selector) -> Option<(TriggerPattern, String)> {
    let (kind, rest) = by.0.split_first()?;
    let kind = match kind {
        Pred::Spell => "a spell",
        _ => return None,
    };
    let whose = match rest {
        [] => "",
        [Pred::ControlledBy(Ref::You)] => " you control",
        [Pred::ControlledBy(Ref::Opponents)] => " an opponent controls",
        _ => return None,
    };
    Some((
        TriggerPattern::BecomesTarget {
            what: what.clone(),
            by: Selector(Vec::new()),
        },
        format!(
            "Whenever {} becomes the target of {kind}{whose}",
            subject_text(what)?
        ),
    ))
}

/// A batch of events: the attackers of one combat and the counters removed at once.
fn batch(inner: &TriggerPattern) -> Option<(TriggerPattern, String)> {
    match inner {
        TriggerPattern::Attacks { what } => {
            let stand = TriggerPattern::GroupAttacks {
                what: what.clone(),
                at_least: 1,
            };
            let group = phrase_text(what, Form::All)?;
            let plain = group.strip_suffix(" you control")?;
            Some((
                stand,
                format!("Whenever you attack with one or more {plain}"),
            ))
        }
        TriggerPattern::CountersRemoved {
            what,
            counter: Some(counter),
        } => Some((
            inner.clone(),
            format!(
                "Whenever one or more {} counters are removed from {}",
                kind_text(counter)?,
                subject_text(what)?
            ),
        )),
        _ => None,
    }
}

/// Several events in one head, the words they open with said once: "a creature you control dies or is put into the
/// command zone", "~ deals combat damage to a player or planeswalker".
fn any_of(parts: &[TriggerPattern]) -> Option<(TriggerPattern, String)> {
    let heads: Vec<(TriggerPattern, String)> = parts
        .iter()
        .map(|part| match pattern_text(part) {
            Some(said) => Some((part.clone(), said)),
            None => event(part),
        })
        .collect::<Option<_>>()?;
    let (stand, _) = heads.first()?.clone();
    let words: Vec<Vec<&str>> = heads
        .iter()
        .map(|(_, said)| body(said).map(|said| said.split(' ').collect()))
        .collect::<Option<_>>()?;
    let shortest = words.iter().map(Vec::len).min()?;
    let shared = (0..shortest.saturating_sub(1))
        .take_while(|at| words.iter().all(|other| other[*at] == words[0][*at]))
        .count();
    let rests: Vec<String> = words.iter().map(|said| said[shared..].join(" ")).collect();
    let lead = words[0][..shared].join(" ");
    let joined = rests.join(" or ");
    let said = if lead.is_empty() {
        format!("Whenever {joined}")
    } else {
        format!("Whenever {lead} {joined}")
    };
    Some((stand, said))
}

/// "a creature card", "a noncreature, nonland card": a discarded card by its types.
fn card_text(what: &Selector) -> Option<String> {
    let words: Vec<String> = what
        .0
        .iter()
        .map(|pred| match pred {
            Pred::Type(kind) => Some(type_name(*kind).to_owned()),
            Pred::Not(inner) => match inner.as_ref() {
                Pred::Type(kind) => Some(format!("non{}", type_name(*kind))),
                _ => None,
            },
            _ => None,
        })
        .collect::<Option<_>>()?;
    let types = words.join(", ");
    Some(format!("{} {types} card", article(&types)))
}

/// "Whenever a source you control deals 5 or more damage to a player": a damage event whose intervening "if" asks
/// for an amount, as one head.
fn damage_at_least(ability: &Ability) -> Option<(TriggerPattern, String)> {
    let Ability::Triggered {
        when: when @ TriggerPattern::DealsDamage { source, .. },
        intervening_if: Some(condition),
        ..
    } = ability
    else {
        return None;
    };
    let condition = to_json(condition);
    let least = condition.pointer("/Compare/right")?.as_u64()?;
    let asked = json!({"Compare": {"left": "Triggering", "op": "Ge", "right": least}});
    (condition == asked).then_some(())?;
    let said = pattern_text(when)?;
    let (head, rest) = said.split_once(" deals damage")?;
    // A source of any kind is "a source", not "a permanent".
    let head = if source.0 == [Pred::ControlledBy(Ref::You)] {
        head.replacen("a permanent you control", "a source you control", 1)
    } else {
        head.to_owned()
    };
    Some((
        when.clone(),
        format!("{head} deals {least} or more damage{rest}"),
    ))
}
