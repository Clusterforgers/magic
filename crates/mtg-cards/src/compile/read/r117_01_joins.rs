//! "Tap target creature an opponent controls and put a stun counter on it." (Enterprise NX-01): a tap or an untap of one
//! target and the counters put on it, joined with "and", read as the two sentences in order with "it" the target; the
//! writer prints them as two sentences and this wording joins them.

use super::effects::templates;
use super::tokens::Reader;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::Ref;

/// The target the first instruction taps or untaps, when it is one of those alone.
fn r117_tapped(first: &[Effect]) -> Option<Ref> {
    match first {
        [Effect::Tap { what, .. } | Effect::Untap { what }] if matches!(what, Ref::Target(_)) => {
            Some(what.clone())
        }
        _ => None,
    }
}

/// Reads "tap target creature and put a stun counter on it": the tap, then the counters on that target.
pub(super) fn r117_tap_and_counter(r: &mut Reader<'_>, effects: &mut Vec<Effect>) -> Option<()> {
    let found = templates().find_map(|template| {
        r.attempt(|r| {
            let mut first = template(r)?;
            let target = r117_tapped(&first)?;
            r.need("and")?;
            (r.peek_word() == Some("put")).then_some(())?;
            let outer = r.it.replace(target.clone());
            let second = templates().find_map(|template| {
                r.attempt(|r| {
                    let found = template(r)?;
                    r.sentence_end().then_some(found)
                })
            });
            r.it = outer;
            let second = second?;
            matches!(second.as_slice(), [Effect::PutCounters { on, .. }] if *on == target)
                .then_some(())?;
            first.extend(second);
            Some(first)
        })
    })?;
    effects.extend(found);
    Some(())
}

/// The verbs a sentence of the controller opens with, which a card says with "You" after a sentence of another player.
const R117_VERBS: [&str; 7] = [
    "Draw ", "Gain ", "Lose ", "Scry ", "Surveil ", "Mill ", "Create ",
];

/// The subjects of a sentence about another player.
const R117_OTHERS: [&str; 5] = [
    "Target opponent ",
    "Target player ",
    "Each opponent ",
    "Each player ",
    "That player ",
];

/// The wordings of `texts` with the sentence of the controller after a sentence of another player said with "You":
/// "Target opponent draws a card. You draw two cards." (Test of Conviction).
pub(super) fn r117_you_wordings(texts: &[String]) -> Vec<String> {
    texts
        .iter()
        .filter_map(|text| {
            let mut found = text.clone();
            for verb in R117_VERBS {
                let Some(at) = found.find(&format!(". {verb}")) else {
                    continue;
                };
                let other = found[..at]
                    .split(". ")
                    .filter_map(|sentence| sentence.rsplit(['\n', '\u{2014}']).next())
                    .any(|sentence| {
                        let sentence = sentence.trim_start();
                        R117_OTHERS
                            .iter()
                            .any(|subject| sentence.starts_with(subject))
                    });
                if other {
                    let lowered = verb.to_lowercase();
                    found = format!(
                        "{}. You {lowered}{}",
                        &found[..at],
                        &found[at + 2 + verb.len()..]
                    );
                }
            }
            (found != *text).then_some(found)
        })
        .collect()
}

/// The wordings of an ability whose last two instructions tap or untap its one target and put counters on it: the
/// wordings of the ability without the counters, each with " and put <counters> on it" before its period.
pub(super) fn r117_join_wordings(ability: &Ability) -> Vec<String> {
    let Ok(mut json) = serde_json::to_value(ability) else {
        return Vec::new();
    };
    // A clause of several chapters pairs with its first chapter only (roadmap sa23a), so a chapter stays unwritten.
    if json.pointer("/Triggered/when/Chapter").is_some() {
        return Vec::new();
    }
    let Some(body) = json.as_object_mut().and_then(|map| map.values_mut().next()) else {
        return Vec::new();
    };
    let one_target = body
        .get("targets")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|targets| targets.len() == 1 && targets[0].get("bounds").is_none());
    let Some(effects) = body
        .get_mut("effects")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return Vec::new();
    };
    let Some(last) = effects.pop() else {
        return Vec::new();
    };
    let pair = effects.last().cloned().into_iter().chain([last]).collect();
    let Ok(pair) = serde_json::from_value::<Vec<Effect>>(serde_json::Value::Array(pair)) else {
        return Vec::new();
    };
    let [first, Effect::PutCounters { on, counter, count }] = pair.as_slice() else {
        return Vec::new();
    };
    let tapped = r117_tapped(std::slice::from_ref(first));
    if !one_target || tapped.as_ref() != Some(on) {
        return Vec::new();
    }
    let Some(counters) = super::counters::text(counter, count) else {
        return Vec::new();
    };
    let Ok(without) = serde_json::from_value::<Ability>(json) else {
        return Vec::new();
    };
    super::render::variants(&without)
        .into_iter()
        .filter_map(|text| {
            // The counters join the sentence of the tap, before any rider after it ("Activate only as a sorcery.").
            let lowered = text.to_lowercase();
            let at = lowered.find("tap target ")?;
            let end = text[at..].find('.')? + at;
            let (head, tail) = text.split_at(end);
            // Also a sentence of its own ("... controls. Put a stun counter on it."), as the last-resort writer says it.
            Some([
                format!("{head} and put {counters} on it{tail}"),
                format!("{head}. Put {counters} on it{tail}"),
            ])
        })
        .flatten()
        .collect()
}
